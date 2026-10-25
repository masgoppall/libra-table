//! Import commands: preview a CSV/JSON file, import it into a table, run a SQL file.
//! Jobs run in the background and report `import-progress` / `sql-file-progress`
//! events; they share the cancellation registry with exports (`cancel_import`).

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::import::sqlrun::{self, SqlFileProgress, SqlFileRequest, SqlFileSummary};
use crate::import::{self, FilePreview, ImportProgress, ImportRequest, ImportSummary, NewColumn};
use crate::import::{SourceSpec, Target};
use crate::models::Dialect;
use crate::state::{AppState, Session};

use super::{confirm_production, record_history, Cmd, Outcome, Timer};

/// Statements of a SQL file shown in the production confirmation.
const PREVIEW_STATEMENTS: usize = 10;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobEvent<P: Serialize, S: Serialize> {
    job_id: String,
    progress: P,
    done: bool,
    summary: Option<S>,
    error: Option<AppError>,
}

fn emit<P: Serialize + Clone, S: Serialize + Clone>(
    app: &AppHandle,
    event: &str,
    job_id: &str,
    progress: &P,
    end: Option<Result<S, AppError>>,
) {
    let done = end.is_some();
    let (summary, error) = match end {
        Some(Ok(s)) => (Some(s), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    let payload = JobEvent {
        job_id: job_id.to_string(),
        progress: progress.clone(),
        done,
        summary,
        error,
    };
    let _ = app.emit(event, payload);
}

/// Registers a cancellable job under `job_id` (the export registry).
pub(super) fn register_job(state: &AppState, job_id: &str) -> CancellationToken {
    let token = CancellationToken::new();
    state
        .exports
        .lock()
        .unwrap()
        .insert(job_id.to_string(), token.clone());
    token
}

pub(super) fn forget_job(app: &AppHandle, job_id: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.exports.lock().unwrap().remove(job_id);
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> Cmd<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn preview_import_file(driver: Dialect, source: SourceSpec) -> Cmd<FilePreview> {
    blocking(move || import::preview_file(&source, driver)).await
}

/// The statements an import will run. `new_columns` describes a table that is created
/// first (so it doesn't exist yet).
#[tauri::command]
pub async fn preview_import(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: ImportRequest,
    new_columns: Option<Vec<NewColumn>>,
) -> Cmd<Vec<String>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let target = import::target(&s, &request, new_columns.as_deref()).await?;
    Ok(import::statements(&target, &request))
}

#[tauri::command]
pub async fn start_import(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    job_id: String,
    request: ImportRequest,
    confirmed: bool,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let target: Target = import::target(&s, &request, None).await?;
    let statements = import::statements(&target, &request);
    confirm_production(&state, &s, &statements, confirmed)?;
    let token = register_job(&state, &job_id);
    tauri::async_runtime::spawn(import_job(app, s, job_id, request, statements, token));
    Ok(())
}

async fn import_job(
    app: AppHandle,
    s: Arc<Session>,
    job_id: String,
    request: ImportRequest,
    statements: Vec<String>,
    token: CancellationToken,
) {
    const EVENT: &str = "import-progress";
    let timer = Timer::start();
    let progress = |p: &ImportProgress| emit::<_, ImportSummary>(&app, EVENT, &job_id, p, None);
    let result = import::run(&s, &request, &token, progress).await;
    let sql = format!(
        "-- Import {}\n{}",
        request.source.path,
        statements.join("\n")
    );
    history(&app, &s, &sql, &timer, result.as_ref().map(|r| r.inserted)).await;
    emit(
        &app,
        EVENT,
        &job_id,
        &ImportProgress::default(),
        Some(result),
    );
    forget_job(&app, &job_id);
}

async fn history(app: &AppHandle, s: &Session, sql: &str, t: &Timer, r: Result<u64, &AppError>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let outcome = Outcome {
        status: if r.is_ok() { "ok" } else { "error" },
        row_count: r.as_ref().ok().map(|n| *n as i64),
        error: r.as_ref().err().map(|e| e.message.as_str()),
    };
    record_history(&state, s, sql, t, outcome).await;
}

#[tauri::command]
pub async fn start_sql_file(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    job_id: String,
    request: SqlFileRequest,
    confirmed: bool,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    if state.live_connection(&s).is_production() && !confirmed {
        let statements = sql_file_preview(&request.path, s.dialect()).await?;
        confirm_production(&state, &s, &statements, confirmed)?;
    }
    let token = register_job(&state, &job_id);
    tauri::async_runtime::spawn(sql_file_job(app, s, job_id, request, token));
    Ok(())
}

/// The first statements of the file and a line standing for the rest.
async fn sql_file_preview(path: &str, driver: Dialect) -> Cmd<Vec<String>> {
    let p = path.to_string();
    let mut out =
        blocking(move || sqlrun::first_statements(&p, driver, PREVIEW_STATEMENTS)).await?;
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    out.push(format!(
        "-- … then every other statement of {name} ({:.1} MB)",
        size as f64 / 1e6
    ));
    Ok(out)
}

async fn sql_file_job(
    app: AppHandle,
    s: Arc<Session>,
    job_id: String,
    request: SqlFileRequest,
    token: CancellationToken,
) {
    const EVENT: &str = "sql-file-progress";
    let timer = Timer::start();
    let progress = |p: &SqlFileProgress| emit::<_, SqlFileSummary>(&app, EVENT, &job_id, p, None);
    let result = sqlrun::run(&s, &request, &token, progress).await;
    let sql = format!("-- Run SQL file {}", request.path);
    history(
        &app,
        &s,
        &sql,
        &timer,
        result.as_ref().map(|r| r.statements),
    )
    .await;
    emit(
        &app,
        EVENT,
        &job_id,
        &SqlFileProgress::default(),
        Some(result),
    );
    forget_job(&app, &job_id);
}

#[tauri::command]
pub fn cancel_import(state: State<'_, AppState>, job_id: String) -> bool {
    match state.exports.lock().unwrap().get(&job_id) {
        Some(t) => {
            t.cancel();
            true
        }
        None => false,
    }
}
