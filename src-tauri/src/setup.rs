//! Startup: loads the saved state into `AppState` and forwards deep links to the UI.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::sync::RwLock;

use crate::state::{now_ms, AppState, Config};
use crate::storage::history::History;
use crate::storage::secrets::SecretStore;
use crate::storage::Storage;

const DAY_MS: i64 = 86_400_000;

pub fn build_state(app: &tauri::App) -> Result<AppState, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    let storage = Storage::new(&dir)?;
    let connections = storage.load_connections()?;
    let groups = storage.load_groups()?;
    let settings = storage.load_settings()?;
    let table_states = storage.load_table_states()?;
    let history = tauri::async_runtime::block_on(History::open(&dir.join("history.db")))?;

    // Retention (QRY-6): drop history older than the configured number of days.
    let cutoff = now_ms() - settings.history_retention_days as i64 * DAY_MS;
    let _ = tauri::async_runtime::block_on(history.purge_older_than(cutoff));

    Ok(AppState {
        storage,
        secrets: SecretStore::keychain(),
        config: Mutex::new(Config {
            connections,
            groups,
        }),
        settings: Mutex::new(settings),
        table_states: Mutex::new(table_states),
        sessions: RwLock::new(HashMap::new()),
        pending_host_keys: Mutex::new(HashMap::new()),
        imported_keys: Mutex::new(HashMap::new()),
        running: Mutex::new(HashMap::new()),
        exports: Mutex::new(HashMap::new()),
        tool_jobs: Mutex::new(HashMap::new()),
        history,
    })
}

/// CON-8: connection URLs opened from the OS are forwarded to the UI, which shows
/// them in the connection form. Nothing connects automatically.
pub fn forward_deep_links(app: &tauri::App) {
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();
        let _ = handle.emit("deep-link", urls);
    });
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        let urls: Vec<String> = urls.iter().map(|u| u.to_string()).collect();
        let handle = app.handle().clone();
        // Give the webview time to subscribe before replaying the launch URL.
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = handle.emit("deep-link", urls);
        });
    }
}
