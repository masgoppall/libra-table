//! Redis key view: databases with key counts, key pages, one key's value and edits to
//! it. Edits are writes like grid commits: read-only connections refuse them, safe mode
//! may ask first, and they are recorded in the history as the commands sent.

use tauri::State;

use crate::engines::redis::{
    DatabaseInfo, KeyDetail, KeyEdit, ReadRequest, RedisEngine, ScanPage, ScanRequest,
};
use crate::error::AppError;
use crate::state::{AppState, Session};

use super::{confirm_production, record_history, Cmd, Outcome, Timer};

/// The session's Redis engine.
fn engine(s: &Session) -> Cmd<&RedisEngine> {
    s.pool
        .engine()
        .and_then(|e| e.as_any())
        .and_then(|a| a.downcast_ref::<RedisEngine>())
        .ok_or_else(|| AppError::validation("This connection is not a Redis connection."))
}

#[tauri::command]
pub async fn redis_databases(state: State<'_, AppState>, id: String) -> Cmd<Vec<DatabaseInfo>> {
    let s = state.session(&id).await?;
    engine(&s)?.database_list().await
}

#[tauri::command]
pub async fn redis_scan(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: ScanRequest,
) -> Cmd<ScanPage> {
    let s = state.session_in(&id, database.as_deref()).await?;
    engine(&s)?.scan(&request).await
}

#[tauri::command]
pub async fn redis_read_key(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: ReadRequest,
) -> Cmd<KeyDetail> {
    let s = state.session_in(&id, database.as_deref()).await?;
    engine(&s)?.read_key(&request).await
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub affected: u64,
    pub statements: Vec<String>,
}

#[tauri::command]
pub async fn redis_edit(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    edit: KeyEdit,
    confirmed: bool,
) -> Cmd<EditResult> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let e = engine(&s)?;
    let plan = RedisEngine::edit_plan(&edit)?;
    let statements = plan.previews();
    if statements.is_empty() {
        return Ok(EditResult {
            affected: 0,
            statements,
        });
    }
    confirm_production(&state, &s, &statements, confirmed)?;
    let timer = Timer::start();
    let result = e.run_plan(&plan).await;
    let outcome = match &result {
        Ok(_) => Outcome {
            status: "ok",
            row_count: Some(edit.affected() as i64),
            error: None,
        },
        Err(err) => Outcome {
            status: "error",
            row_count: None,
            error: Some(&err.message),
        },
    };
    record_history(&state, &s, &statements.join("\n"), &timer, outcome).await;
    result?;
    Ok(EditResult {
        affected: edit.affected(),
        statements,
    })
}
