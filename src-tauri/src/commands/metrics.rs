//! Metrics board commands: boards per connection, and running a widget's query with
//! its `{{variables}}` filled in (read-only, see `metrics::run`).

use tauri::State;

use crate::db::exec::StatementResult;
use crate::metrics::{self, Board, Variable};
use crate::state::AppState;

use super::{confirm_safe_mode, Cmd};

#[tauri::command]
pub fn list_metric_boards(state: State<'_, AppState>, id: String) -> Cmd<Vec<Board>> {
    metrics::list_boards(state.storage.dir(), &id)
}

#[tauri::command]
pub fn save_metric_board(state: State<'_, AppState>, id: String, board: Board) -> Cmd<Board> {
    metrics::save_board(state.storage.dir(), &id, board)
}

#[tauri::command]
pub fn delete_metric_board(state: State<'_, AppState>, id: String, board_id: String) -> Cmd<()> {
    metrics::delete_board(state.storage.dir(), &id, &board_id)
}

#[tauri::command]
pub async fn run_metric_query(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    sql: String,
    variables: Vec<Variable>,
) -> Cmd<StatementResult> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let sql = metrics::substitute(s.dialect(), &sql, &variables)?;
    let stmts = metrics::read_statements(s.dialect(), &sql)?;
    // Only reads get here, so safe mode has nothing to confirm; it stays the gate.
    confirm_safe_mode(&state, &s, &stmts, false)?;
    metrics::run_widget_query(&s, stmts).await
}
