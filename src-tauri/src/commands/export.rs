//! Export commands: start a background export job and cancel it. Progress goes to
//! the UI as `export-progress` events.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::export::{self, ExportRequest};
use crate::state::{AppState, Session};

use super::{confirm_safe_mode, Cmd};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    job_id: String,
    rows: u64,
    done: bool,
    error: Option<AppError>,
    path: String,
}

#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    job_id: String,
    request: ExportRequest,
    confirmed: Option<bool>,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    if request.path.trim().is_empty() {
        return Err(AppError::validation("Choose where to save the file."));
    }
    // Exporting a query result runs the query again: safe mode applies (SAFE-2).
    if let export::Source::Query { sql } = &request.source {
        let stmts: Vec<String> = crate::engines::split(s.connection.driver, sql)
            .into_iter()
            .map(|st| st.text)
            .collect();
        confirm_safe_mode(&state, &s, &stmts, confirmed.unwrap_or(false))?;
    }
    let token = CancellationToken::new();
    state
        .exports
        .lock()
        .unwrap()
        .insert(job_id.clone(), token.clone());
    tauri::async_runtime::spawn(run_job(app, s, job_id, request, token));
    Ok(())
}

/// Runs one export job and reports its progress, then forgets the job.
async fn run_job(
    app: AppHandle,
    s: Arc<Session>,
    job_id: String,
    request: ExportRequest,
    token: CancellationToken,
) {
    let emit = |rows: u64, done: bool, error: Option<AppError>| {
        let progress = ExportProgress {
            job_id: job_id.clone(),
            rows,
            done,
            error,
            path: request.path.clone(),
        };
        let _ = app.emit("export-progress", progress);
    };
    let result = export::run(&s, &request, &token, |rows| emit(rows, false, None)).await;
    match result {
        Ok(rows) => emit(rows, true, None),
        Err(e) => emit(0, true, Some(e)),
    }
    if let Some(state) = app.try_state::<AppState>() {
        state.exports.lock().unwrap().remove(&job_id);
    }
}

#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>, job_id: String) -> bool {
    match state.exports.lock().unwrap().get(&job_id) {
        Some(t) => {
            t.cancel();
            true
        }
        None => false,
    }
}
