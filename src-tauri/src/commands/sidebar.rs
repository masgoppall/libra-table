//! The sidebar's recent and pinned items, kept per connection.

use tauri::State;

use crate::state::AppState;
use crate::storage::SidebarState;

use super::Cmd;

#[tauri::command]
pub fn get_sidebar_state(state: State<'_, AppState>, id: String) -> Cmd<SidebarState> {
    state.storage.load_sidebar(&id)
}

#[tauri::command]
pub fn save_sidebar_state(
    state: State<'_, AppState>,
    id: String,
    sidebar: SidebarState,
) -> Cmd<()> {
    state.storage.save_sidebar(&id, sidebar)
}
