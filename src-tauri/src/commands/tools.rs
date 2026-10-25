//! Backup / restore with the native tools and "Open in command line". Jobs run in the
//! background; their progress goes to the UI as `tool-progress` events.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::PasswordMode;
use crate::server::tools::args::{BackupRequest, RestoreRequest};
use crate::server::tools::run::Progress;
use crate::server::tools::{self, cli, endpoint::Endpoint, ToolChoice, ToolPlan};
use crate::state::{AppState, Session};

use super::{confirm_production, Cmd};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolProgress {
    job_id: String,
    /// A stderr line of the tool, when this event carries one.
    line: Option<String>,
    bytes: u64,
    total: Option<u64>,
    done: bool,
    error: Option<AppError>,
    path: String,
}

/// `tool` for the session's server, honoring the path the user chose for it.
async fn choose(state: &AppState, s: &Session, tool: &str) -> AppResult<ToolChoice> {
    let saved = state.storage.load_tool_paths()?;
    let custom = saved.paths.get(tool).map(String::as_str);
    Ok(tools::resolve(tool, s.dialect(), &s.server_version, custom).await)
}

async fn backup_choice(state: &AppState, s: &Session) -> AppResult<Option<ToolChoice>> {
    match tools::backup_tool(s.dialect()) {
        Some(tool) => Ok(Some(choose(state, s, tool).await?)),
        None => Ok(None),
    }
}

/// The restore tool for the chosen file (psql or pg_restore by its format).
async fn restore_choice(state: &AppState, s: &Session, path: &str) -> AppResult<ToolChoice> {
    let tool = if path.trim().is_empty() {
        tools::restore_tool_default(s.dialect())?
    } else {
        tools::restore_tool(s.dialect(), path)?
    };
    choose(state, s, tool).await
}

#[tauri::command]
pub async fn plan_backup(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: BackupRequest,
) -> Cmd<ToolPlan> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let choice = backup_choice(&state, &s).await?;
    Ok(tools::backup_plan(&s, &request, choice))
}

#[tauri::command]
pub async fn plan_restore(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: RestoreRequest,
) -> Cmd<ToolPlan> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let choice = restore_choice(&state, &s, &request.path).await?;
    Ok(tools::restore_plan(&s, &request, choice))
}

/// Remembers the program to use for `tool`; `None` goes back to finding it.
#[tauri::command]
pub async fn set_tool_path(
    state: State<'_, AppState>,
    tool: String,
    path: Option<String>,
) -> Cmd<()> {
    if !tools::TOOLS.contains(&tool.as_str()) {
        return Err(AppError::validation(format!("Unknown tool \"{tool}\".")));
    }
    let path = path.filter(|p| !p.trim().is_empty());
    if let Some(p) = &path {
        if tools::locate::probe(std::path::Path::new(p))
            .await
            .is_none()
        {
            return Err(AppError::validation(format!(
                "{p} doesn't look like {tool}: it doesn't answer --version."
            )));
        }
    }
    state.storage.set_tool_path(&tool, path.as_deref())
}

#[tauri::command]
pub async fn start_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    job_id: String,
    request: BackupRequest,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let choice = backup_choice(&state, &s).await?;
    let token = register(&state, &job_id);
    tauri::async_runtime::spawn(async move {
        let path = request.path.clone();
        let mut emit = emitter(&app, &job_id, &path);
        let r = tools::backup(&s, &request, choice.as_ref(), &token, &mut emit).await;
        finish(&app, &job_id, &path, r);
    });
    Ok(())
}

#[tauri::command]
pub async fn start_restore(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    job_id: String,
    request: RestoreRequest,
    confirmed: bool,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let choice = restore_choice(&state, &s, &request.path).await?;
    let plan = tools::restore_plan(&s, &request, choice.clone());
    // A restore changes data: on production it needs the confirmation.
    confirm_production(&state, &s, &[plan.command], confirmed)?;
    let token = register(&state, &job_id);
    tauri::async_runtime::spawn(async move {
        let path = request.path.clone();
        let mut emit = emitter(&app, &job_id, &path);
        let r = tools::restore(&s, &request, &choice, &token, &mut emit).await;
        s.clear_all_caches().await;
        finish(&app, &job_id, &path, r);
    });
    Ok(())
}

fn register(state: &AppState, job_id: &str) -> CancellationToken {
    let token = CancellationToken::new();
    let jobs = &state.tool_jobs;
    jobs.lock()
        .unwrap()
        .insert(job_id.to_string(), token.clone());
    token
}

fn emitter<'a>(app: &'a AppHandle, job_id: &'a str, path: &'a str) -> impl FnMut(Progress) + 'a {
    let mut bytes = 0;
    let mut total = None;
    move |p| {
        let line = match p {
            Progress::Line(l) => Some(l),
            Progress::Bytes { done, total: t } => {
                (bytes, total) = (done, t);
                None
            }
        };
        let event = progress(job_id, path, line, bytes, total);
        let _ = app.emit("tool-progress", event);
    }
}

fn progress(
    job_id: &str,
    path: &str,
    line: Option<String>,
    bytes: u64,
    total: Option<u64>,
) -> ToolProgress {
    ToolProgress {
        job_id: job_id.to_string(),
        line,
        bytes,
        total,
        done: false,
        error: None,
        path: path.to_string(),
    }
}

/// Reports the end of a job and forgets it.
fn finish(app: &AppHandle, job_id: &str, path: &str, result: AppResult<()>) {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let event = ToolProgress {
        done: true,
        error: result.err(),
        ..progress(job_id, path, None, size, None)
    };
    let _ = app.emit("tool-progress", event);
    if let Some(state) = app.try_state::<AppState>() {
        state.tool_jobs.lock().unwrap().remove(job_id);
    }
}

#[tauri::command]
pub fn cancel_tool_job(state: State<'_, AppState>, job_id: String) -> bool {
    match state.tool_jobs.lock().unwrap().get(&job_id) {
        Some(t) => {
            t.cancel();
            true
        }
        None => false,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLine {
    command: String,
    tool: ToolChoice,
    /// The command uses the SSH tunnel's local port: it works while LibraTable is connected.
    through_tunnel: bool,
}

async fn command_for(state: &AppState, s: &Session) -> AppResult<CommandLine> {
    let tool = choose(state, s, tools::client_tool(s.dialect())).await?;
    let program = tool.path.clone().unwrap_or_else(|| tool.tool.clone());
    let asks = s.connection.db.password_mode != PasswordMode::None;
    let e = Endpoint::of(s);
    Ok(CommandLine {
        command: cli::command_line(&e, &program, tool.mariadb, asks),
        through_tunnel: e.through_tunnel,
        tool,
    })
}

#[tauri::command]
pub async fn command_line(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
) -> Cmd<CommandLine> {
    let s = state.session_in(&id, database.as_deref()).await?;
    command_for(&state, &s).await
}

#[tauri::command]
pub async fn open_in_terminal(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let line = command_for(&state, &s).await?;
    cli::open_in_terminal(&line.command, &std::env::temp_dir())?;
    Ok(())
}
