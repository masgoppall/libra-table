//! Connection commands: the saved list, groups, keys, connect / test / disconnect and
//! connection URLs.

use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::connections::transfer;
use crate::connections::url::{self, ConnectionDraft};
use crate::connections::{self, ConnectInfo, ConnectionInput, ImportedKeyInfo, TestResult};
use crate::models::{ConnectionView, Group, PromptedSecrets};
use crate::state::AppState;

use super::Cmd;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionList {
    connections: Vec<ConnectionView>,
    groups: Vec<Group>,
}

#[tauri::command]
pub fn list_connections(state: State<'_, AppState>) -> ConnectionList {
    let (conns, groups) = {
        let cfg = state.config.lock().unwrap();
        (cfg.connections.clone(), cfg.groups.clone())
    };
    ConnectionList {
        connections: conns.iter().map(|c| connections::view(&state, c)).collect(),
        groups,
    }
}

#[tauri::command]
pub fn save_connection(state: State<'_, AppState>, input: ConnectionInput) -> Cmd<ConnectionView> {
    connections::save(&state, input)
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: String) -> Cmd<()> {
    connections::delete(&state, &id).await
}

#[tauri::command]
pub fn duplicate_connection(
    state: State<'_, AppState>,
    id: String,
    copy_secrets: bool,
) -> Cmd<ConnectionView> {
    connections::duplicate(&state, &id, copy_secrets)
}

#[tauri::command]
pub fn save_group(state: State<'_, AppState>, group: Group) -> Cmd<Group> {
    connections::save_group(&state, group)
}

#[tauri::command]
pub fn delete_group(state: State<'_, AppState>, id: String) -> Cmd<()> {
    connections::delete_group(&state, &id)
}

#[tauri::command]
pub fn import_ssh_key(state: State<'_, AppState>, path: String) -> Cmd<ImportedKeyInfo> {
    connections::import_key(&state, &path)
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    input: ConnectionInput,
    prompted: Option<PromptedSecrets>,
) -> Cmd<TestResult> {
    connections::test(&state, input, prompted.unwrap_or_default()).await
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    id: String,
    prompted: Option<PromptedSecrets>,
) -> Cmd<ConnectInfo> {
    connections::connect(&state, &id, prompted.unwrap_or_default()).await
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>, id: String) -> Cmd<()> {
    connections::disconnect(&state, &id).await;
    Ok(())
}

#[tauri::command]
pub async fn open_sessions(state: State<'_, AppState>) -> Cmd<Vec<String>> {
    Ok(state.sessions.read().await.keys().cloned().collect())
}

#[tauri::command]
pub fn trust_host_key(state: State<'_, AppState>, host: String, port: u16) -> Cmd<()> {
    connections::trust_host_key(&state, &host, port)
}

#[tauri::command]
pub async fn create_sqlite_file(path: String) -> Cmd<()> {
    connections::create_sqlite_file(&path).await
}

/// Closes the extra session kept for `database` (e.g. when its last tab closes).
#[tauri::command]
pub async fn close_database(state: State<'_, AppState>, id: String, database: String) -> Cmd<()> {
    state.session(&id).await?.close_database(&database).await;
    Ok(())
}

#[tauri::command]
pub fn connection_url(state: State<'_, AppState>, id: String) -> Cmd<String> {
    Ok(url::to_url(&state.connection(&id)?))
}

#[tauri::command]
pub fn parse_connection_url(url: String) -> Cmd<ConnectionDraft> {
    url::parse(&url)
}

/// Writes connections (all when `ids` is empty) to a file; secrets only with a passphrase.
#[tauri::command]
pub fn export_connections(
    state: State<'_, AppState>,
    ids: Vec<String>,
    path: String,
    passphrase: Option<String>,
) -> Cmd<usize> {
    transfer::export(&state, &ids, Path::new(&path), passphrase.as_deref())
}

#[tauri::command]
pub fn inspect_connections_file(path: String) -> Cmd<transfer::FileSummary> {
    transfer::inspect(Path::new(&path))
}

#[tauri::command]
pub fn import_connections(
    state: State<'_, AppState>,
    path: String,
    passphrase: Option<String>,
) -> Cmd<transfer::ImportReport> {
    transfer::import(&state, Path::new(&path), passphrase.as_deref())
}
