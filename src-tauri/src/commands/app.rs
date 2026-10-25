//! App-level commands: version, settings, the saved workspace and per-table grid state.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use crate::state::AppState;
use crate::storage::{Settings, TableState};

use super::Cmd;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.storage.dir().display().to_string(),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Cmd<Settings> {
    settings.validate()?;
    state.storage.save_settings(&settings)?;
    let changed = state.settings.lock().unwrap().shortcuts != settings.shortcuts;
    *state.settings.lock().unwrap() = settings.clone();
    if changed {
        crate::menu::apply_shortcuts(&app, &settings.shortcuts);
    }
    Ok(settings)
}

#[tauri::command]
pub fn get_workspace(state: State<'_, AppState>) -> Cmd<Value> {
    state.storage.load_workspace()
}

#[tauri::command]
pub fn save_workspace(state: State<'_, AppState>, workspace: Value) -> Cmd<()> {
    state.storage.save_workspace(&workspace)
}

#[tauri::command]
pub fn get_table_state(state: State<'_, AppState>, key: String) -> Option<TableState> {
    state.table_states.lock().unwrap().get(&key).cloned()
}

#[tauri::command]
pub fn save_table_state(
    state: State<'_, AppState>,
    key: String,
    table_state: TableState,
) -> Cmd<()> {
    let mut ts = state.table_states.lock().unwrap();
    ts.insert(key, table_state);
    state.storage.save_table_states(&ts)
}

/// The console log so far (newest last); new entries arrive as `console-log` events.
#[tauri::command]
pub fn console_entries() -> Vec<crate::console::ConsoleEntry> {
    crate::console::entries()
}

#[tauri::command]
pub fn clear_console() {
    crate::console::clear();
}

#[tauri::command]
pub fn passcode_set(state: State<'_, AppState>) -> bool {
    crate::app_lock::is_set(&state)
}

#[tauri::command]
pub async fn verify_passcode(state: State<'_, AppState>, code: String) -> Cmd<bool> {
    crate::app_lock::verify(&state, &code).await
}

/// `next` = None removes the passcode.
#[tauri::command]
pub async fn change_passcode(
    state: State<'_, AppState>,
    current: Option<String>,
    next: Option<String>,
) -> Cmd<()> {
    crate::app_lock::change(&state, current.as_deref(), next.as_deref()).await
}

/// Which page is visible, so closing the window can go back to the connections page.
#[tauri::command]
pub fn set_window_view(in_workspace: bool) {
    crate::window::set_in_workspace(in_workspace);
}
