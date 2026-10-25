//! Exporting several tables to a folder, one file each. Progress goes to the UI as
//! `export-tables-progress` events; `cancel_export` cancels the job.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::export::tables::{self, TablesProgress, TablesRequest};
use crate::state::{AppState, Session};

use super::import::{forget_job, register_job};
use super::Cmd;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TablesEvent {
    job_id: String,
    progress: TablesProgress,
    done: bool,
    error: Option<AppError>,
}

#[tauri::command]
pub async fn start_export_tables(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    job_id: String,
    request: TablesRequest,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    if request.tables.is_empty() {
        return Err(AppError::validation("Choose at least one table to export."));
    }
    let token = register_job(&state, &job_id);
    tauri::async_runtime::spawn(run_job(app, s, job_id, request, token));
    Ok(())
}

async fn run_job(
    app: AppHandle,
    s: Arc<Session>,
    job_id: String,
    request: TablesRequest,
    token: CancellationToken,
) {
    let emit = |progress: &TablesProgress, done: bool, error: Option<AppError>| {
        let event = TablesEvent {
            job_id: job_id.clone(),
            progress: progress.clone(),
            done,
            error,
        };
        let _ = app.emit("export-tables-progress", event);
    };
    match tables::run(&s, &request, &token, |p| emit(p, false, None)).await {
        Ok(p) => emit(&p, true, None),
        Err(e) => emit(&TablesProgress::default(), true, Some(e)),
    }
    forget_job(&app, &job_id);
}
