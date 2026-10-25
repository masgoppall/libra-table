//! Connection lifecycle: save / delete / duplicate (CON-*), opening sessions with an
//! optional SSH tunnel (SSH-*), test, key import, new SQLite files and connection URLs.

mod keys;
mod secrets;
mod session;
mod sqlite_file;
mod store;
pub mod transfer;
pub mod url;

pub use keys::import_key;
pub use session::{connect, disconnect, test, trust_host_key};
pub use sqlite_file::create_sqlite_file;
pub use store::{delete, delete_group, duplicate, save, save_group};

use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::{Connection, ConnectionView, Driver, SecretsInput};
use crate::state::AppState;
use crate::storage::secrets::SecretKind;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    pub connection: Connection,
    #[serde(default)]
    pub secrets: SecretsInput,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectInfo {
    pub connection_id: String,
    pub driver: Driver,
    pub database: String,
    pub server_version: String,
    pub through_tunnel: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub server_version: String,
    pub duration_ms: u64,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedKeyInfo {
    pub token: String,
    pub name: String,
    pub encrypted: bool,
}

/// What the connection list shows: the connection and which secrets it has.
pub fn view(state: &AppState, c: &Connection) -> ConnectionView {
    let has = |kind| state.secrets.has(&c.id, kind);
    ConnectionView {
        summary: c.summary(),
        has_db_password: has(SecretKind::DbPassword),
        has_ssh_password: has(SecretKind::SshPassword),
        has_ssh_key: has(SecretKind::SshKey),
        has_ssh_key_passphrase: has(SecretKind::SshKeyPassphrase),
        connection: c.clone(),
    }
}

fn validate(c: &Connection) -> AppResult<()> {
    if c.name.trim().is_empty() {
        return Err(AppError::validation("Give the connection a name."));
    }
    if c.driver == Driver::Sqlite && c.db.file_path.trim().is_empty() {
        return Err(AppError::validation("Choose the SQLite database file."));
    }
    if c.driver != Driver::Sqlite && c.db.use_socket && c.db.socket.trim().is_empty() {
        return Err(AppError::validation(
            "Enter the socket path, or switch back to host and port.",
        ));
    }
    if c.ssh.enabled {
        if c.driver == Driver::Sqlite {
            return Err(AppError::validation(
                "SSH tunnels are not available for SQLite files.",
            ));
        }
        if c.ssh.host.trim().is_empty() {
            return Err(AppError::validation("Enter the SSH server address."));
        }
    }
    if c.options.page_size == 0 || c.options.page_size > db::data::MAX_PAGE_SIZE {
        return Err(AppError::validation(format!(
            "Page size must be between 1 and {}.",
            db::data::MAX_PAGE_SIZE
        )));
    }
    Ok(())
}
