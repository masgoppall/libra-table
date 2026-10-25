//! Process-wide state: config, open sessions, pending prompts and running work.

mod session;

pub use session::{EditorConn, Session};

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::{Connection, Group};
use crate::ssh::PendingHostKey;
use crate::storage::history::History;
use crate::storage::secrets::SecretStore;
use crate::storage::{Settings, Storage, TableState};

pub struct Config {
    pub connections: Vec<Connection>,
    pub groups: Vec<Group>,
}

/// A private key read from disk, waiting for the form to be saved (never sent to the UI).
pub struct ImportedKey {
    pub name: String,
    pub pem: String,
}

/// How to stop a query that is running on the editor connection (QRY-4).
#[derive(Clone)]
pub enum CancelTarget {
    Postgres {
        pool: sqlx::PgPool,
        pid: i32,
    },
    Mysql {
        pool: sqlx::MySqlPool,
        id: u64,
    },
    /// Raw `sqlite3*` of the running connection; `sqlite3_interrupt` is thread-safe.
    Sqlite {
        handle: usize,
    },
}

pub struct RunningQuery {
    pub target: CancelTarget,
    pub cancelled: Arc<AtomicBool>,
}

pub struct AppState {
    pub storage: Storage,
    pub secrets: SecretStore,
    pub config: Mutex<Config>,
    pub settings: Mutex<Settings>,
    pub table_states: Mutex<BTreeMap<String, TableState>>,
    pub sessions: RwLock<HashMap<String, Arc<Session>>>,
    pub pending_host_keys: Mutex<HashMap<String, PendingHostKey>>,
    pub imported_keys: Mutex<HashMap<String, ImportedKey>>,
    pub running: Mutex<HashMap<String, RunningQuery>>,
    pub exports: Mutex<HashMap<String, CancellationToken>>,
    /// Running backups and restores (native tools), by job id.
    pub tool_jobs: Mutex<HashMap<String, CancellationToken>>,
    pub history: History,
}

impl AppState {
    pub fn connection(&self, id: &str) -> AppResult<Connection> {
        self.config
            .lock()
            .unwrap()
            .connections
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("That connection no longer exists."))
    }

    pub async fn session(&self, id: &str) -> AppResult<Arc<Session>> {
        self.sessions
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(AppError::not_connected)
    }

    /// The session for `database` on connection `id`: the connection's own session, or
    /// one opened next to it for another database of the same server.
    pub async fn session_in(&self, id: &str, database: Option<&str>) -> AppResult<Arc<Session>> {
        self.session(id).await?.in_database(database).await
    }

    /// The connection as it is now: a tag changed to production after connecting must
    /// apply to the open session too.
    pub fn live_connection(&self, s: &Session) -> Connection {
        self.connection(&s.connection.id)
            .unwrap_or_else(|_| s.connection.clone())
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// A state for unit tests: files in `dir`, secrets in memory (never the real keychain).
#[cfg(test)]
pub async fn test_state(dir: &std::path::Path) -> AppState {
    AppState {
        storage: Storage::new(dir).unwrap(),
        secrets: SecretStore::memory(),
        config: Mutex::new(Config {
            connections: Vec::new(),
            groups: Vec::new(),
        }),
        settings: Mutex::new(Settings::default()),
        table_states: Mutex::new(BTreeMap::new()),
        sessions: RwLock::new(HashMap::new()),
        pending_host_keys: Mutex::new(HashMap::new()),
        imported_keys: Mutex::new(HashMap::new()),
        running: Mutex::new(HashMap::new()),
        exports: Mutex::new(HashMap::new()),
        tool_jobs: Mutex::new(HashMap::new()),
        history: History::open(&dir.join("history.db")).await.unwrap(),
    }
}
