//! LibraTable — desktop database client (Tauri 2 + React).
//!
//! All network I/O, secrets and file writes live here in Rust; the React UI calls the
//! commands in `commands/` and listens to events (`export-progress`, `tool-progress`,
//! `console-log`, `deep-link`, `menu`, `close-requested`).

pub mod app_lock;
pub mod commands;
pub mod connections;
pub mod console;
pub mod db;
pub mod diagram;
pub mod drivers;
pub mod engines;
pub mod error;
pub mod export;
pub mod import;
pub mod menu;
pub mod metrics;
pub mod models;
pub mod safe_mode;
pub mod server;
pub mod setup;
pub mod ssh;
pub mod state;
pub mod storage;
pub mod window;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .menu(menu::build)
        .on_menu_event(|app, event| {
            let _ = app.emit("menu", event.id().0.as_str());
        })
        .on_window_event(window::on_event)
        .setup(|app| {
            let state = setup::build_state(app)?;
            let shortcuts = state.settings.lock().unwrap().shortcuts.clone();
            app.manage(state);
            if !shortcuts.is_empty() {
                menu::apply_shortcuts(app.handle(), &shortcuts);
            }
            setup::forward_deep_links(app);
            console::install(app.handle());
            Ok(())
        })
        .invoke_handler(all_commands!())
        .run(tauri::generate_context!())
        .expect("error while running LibraTable");
}
