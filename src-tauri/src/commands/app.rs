//! App-level commands: version, settings, the saved workspace and per-table grid state.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
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
pub fn set_window_view(window: tauri::Window, in_workspace: bool) {
    if window.label() == crate::window::MAIN {
        crate::window::set_in_workspace(in_workspace);
    }
}

/// The compact welcome window (no connection open) or the large workspace window.
#[tauri::command]
pub fn set_window_mode(window: tauri::WebviewWindow, mode: crate::window::mode::Mode) -> Cmd<()> {
    crate::window::mode::apply(&window, mode)
        .map_err(|e| crate::error::AppError::internal(e.to_string()))
}

/// Largest image file `read_image_file` accepts.
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// An image file as a `data:` URL, for the group icon picker (the UI scales it down).
#[tauri::command]
pub fn read_image_file(path: String) -> Cmd<String> {
    let path = std::path::Path::new(&path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        _ => {
            return Err(crate::error::AppError::validation(
                "Choose a PNG, JPEG, GIF, WebP or SVG image.",
            ))
        }
    };
    let size = std::fs::metadata(path)?.len();
    if size > MAX_IMAGE_BYTES {
        return Err(crate::error::AppError::validation(
            "The image is larger than 8 MB.",
        ));
    }
    let bytes = std::fs::read(path)?;
    Ok(format!("data:{mime};base64,{}", B64.encode(bytes)))
}
