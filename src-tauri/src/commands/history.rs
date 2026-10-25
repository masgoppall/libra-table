//! Query history and saved query commands.

use tauri::State;

use crate::state::{now_ms, AppState};
use crate::storage::history::{HistoryEntry, HistoryQuery};
use crate::storage::saved::{SavedQueries, SavedQuery};

use super::Cmd;

const DAY_MS: i64 = 86_400_000;

#[tauri::command]
pub async fn search_history(
    state: State<'_, AppState>,
    query: HistoryQuery,
) -> Cmd<Vec<HistoryEntry>> {
    state.history.search(&query).await
}

#[tauri::command]
pub async fn delete_history(state: State<'_, AppState>, ids: Vec<i64>) -> Cmd<u64> {
    state.history.delete(&ids).await
}

#[tauri::command]
pub async fn purge_history(state: State<'_, AppState>, older_than_days: u32) -> Cmd<u64> {
    let cutoff = now_ms() - older_than_days as i64 * DAY_MS;
    state.history.purge_older_than(cutoff).await
}

fn saved(state: &AppState) -> SavedQueries {
    SavedQueries::new(state.storage.dir())
}

#[tauri::command]
pub fn list_saved_queries(state: State<'_, AppState>, id: String) -> Cmd<Vec<SavedQuery>> {
    saved(&state).list(&id)
}

#[tauri::command]
pub fn read_saved_query(state: State<'_, AppState>, id: String, name: String) -> Cmd<String> {
    saved(&state).read(&id, &name)
}

#[tauri::command]
pub fn write_saved_query(
    state: State<'_, AppState>,
    id: String,
    name: String,
    sql: String,
) -> Cmd<()> {
    saved(&state).write(&id, &name, &sql)
}

#[tauri::command]
pub fn rename_saved_query(
    state: State<'_, AppState>,
    id: String,
    from: String,
    to: String,
) -> Cmd<()> {
    saved(&state).rename(&id, &from, &to)
}

#[tauri::command]
pub fn delete_saved_query(state: State<'_, AppState>, id: String, name: String) -> Cmd<()> {
    saved(&state).delete(&id, &name)
}
