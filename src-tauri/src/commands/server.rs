//! Server commands: the process list (cancel / kill) and users & privileges. Every
//! change is previewed first and needs confirmation on production connections.

use tauri::State;

use crate::error::AppError;
use crate::server::process::{self, ProcessAction, ProcessInfo};
use crate::server::users::{self, Grantee, Stmt, UserChange, UserList};
use crate::state::{AppState, CancelTarget, EditorConn, Session};

use super::{confirm_production, record_history, Cmd, Outcome, Timer};

/// Ids of LibraTable's own connections we know of: the editor connection, and any
/// query running right now (which holds its editor connection).
fn own_ids(state: &AppState, s: &Session) -> Vec<i64> {
    let mut ids: Vec<i64> = state
        .running
        .lock()
        .unwrap()
        .values()
        .filter_map(|r| match &r.target {
            CancelTarget::Postgres { pid, .. } => Some(*pid as i64),
            CancelTarget::Mysql { id, .. } => Some(*id as i64),
            CancelTarget::Sqlite { .. } => None,
        })
        .collect();
    if let Ok(editor) = s.editor.try_lock() {
        match editor.as_ref() {
            Some(EditorConn::Postgres { pid, .. }) => ids.push(*pid as i64),
            Some(EditorConn::Mysql { id, .. }) => ids.push(*id as i64),
            _ => {}
        }
    }
    ids
}

#[tauri::command]
pub async fn list_processes(state: State<'_, AppState>, id: String) -> Cmd<Vec<ProcessInfo>> {
    let s = state.session(&id).await?;
    let own = own_ids(&state, &s);
    process::list(&s.pool, &own).await
}

#[tauri::command]
pub async fn preview_process_action(
    state: State<'_, AppState>,
    id: String,
    process_id: i64,
    action: ProcessAction,
) -> Cmd<Vec<String>> {
    let s = state.session(&id).await?;
    Ok(vec![process::action_sql(s.driver(), action, process_id)?])
}

#[tauri::command]
pub async fn run_process_action(
    state: State<'_, AppState>,
    id: String,
    process_id: i64,
    action: ProcessAction,
    confirmed: bool,
) -> Cmd<()> {
    let s = state.session(&id).await?;
    let sql = process::action_sql(s.driver(), action, process_id)?;
    confirm_production(&state, &s, std::slice::from_ref(&sql), confirmed)?;
    let timer = Timer::start();
    let result = process::run_action(&s.pool, action, process_id).await;
    record_outcome(&state, &s, &sql, &timer, &result).await;
    result
}

#[tauri::command]
pub async fn list_users(state: State<'_, AppState>, id: String) -> Cmd<UserList> {
    users::list(&state.session(&id).await?.pool).await
}

/// Postgres grants on schemas and tables are per database, hence `database`.
#[tauri::command]
pub async fn user_grants(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    grantee: Grantee,
) -> Cmd<Vec<String>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    users::grants(&s.pool, &grantee).await
}

/// The statements as shown: passwords masked.
#[tauri::command]
pub async fn preview_user_change(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    change: UserChange,
) -> Cmd<Vec<String>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    Ok(shown(&users::plan(&s.pool, &change).await?))
}

#[tauri::command]
pub async fn apply_user_change(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    change: UserChange,
    confirmed: bool,
) -> Cmd<Vec<String>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let stmts = users::plan(&s.pool, &change).await?;
    let preview = shown(&stmts);
    if stmts.is_empty() {
        return Err(AppError::validation("Nothing to change."));
    }
    confirm_production(&state, &s, &preview, confirmed)?;
    let timer = Timer::start();
    let result = users::apply(&s.pool, &stmts).await;
    // History gets the masked text: it must never hold a password.
    record_outcome(&state, &s, &preview.join(";\n"), &timer, &result).await;
    result.map(|_| preview)
}

fn shown(stmts: &[Stmt]) -> Vec<String> {
    stmts.iter().map(|s| s.shown.clone()).collect()
}

async fn record_outcome(
    state: &AppState,
    s: &Session,
    sql: &str,
    timer: &Timer,
    result: &Result<(), AppError>,
) {
    let outcome = Outcome {
        status: if result.is_ok() { "ok" } else { "error" },
        row_count: None,
        error: result.as_ref().err().map(|e| e.message.as_str()),
    };
    record_history(state, s, sql, timer, outcome).await;
}
