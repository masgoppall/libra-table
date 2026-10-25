//! Data model shared with the UI. Nothing here holds a secret: passwords and keys
//! live in the OS keychain (see `secrets.rs`) and move only as `SecretChange`.

use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Driver {
    Postgres,
    Mysql,
    Sqlite,
}

impl Driver {
    pub fn default_port(self) -> Option<u16> {
        match self {
            Driver::Postgres => Some(5432),
            Driver::Mysql => Some(3306),
            Driver::Sqlite => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Driver::Postgres => "PostgreSQL",
            Driver::Mysql => "MySQL",
            Driver::Sqlite => "SQLite",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    #[default]
    Local,
    Testing,
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PasswordMode {
    /// Stored in the OS keychain.
    #[default]
    Keychain,
    /// Asked every time the connection opens; never stored.
    Ask,
    /// No password.
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    Disable,
    Allow,
    #[default]
    Prefer,
    Require,
    VerifyCa,
    /// Postgres verify-full / MySQL verify-identity.
    VerifyFull,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct DbSettings {
    /// Empty means 127.0.0.1 (resolved at connect time, never stored).
    pub host: String,
    /// Empty means the driver default port.
    pub port: Option<u16>,
    pub user: String,
    pub database: String,
    pub use_socket: bool,
    pub socket: String,
    /// SQLite only: path to the database file.
    pub file_path: String,
    pub password_mode: PasswordMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SshSettings {
    pub enabled: bool,
    pub host: String,
    /// Empty means 22.
    pub port: Option<u16>,
    pub user: String,
    pub password_mode: PasswordMode,
    pub use_key: bool,
    /// Display name of the imported key file; its content is in the keychain.
    pub key_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TlsSettings {
    pub mode: TlsMode,
    pub ca_path: String,
    pub cert_path: String,
    pub key_path: String,
}

/// When statements need a confirmation (TablePlus "safe mode"). `Auto` follows the tag:
/// production connections confirm writes. "Safe" levels ask to type the connection name.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SafeMode {
    #[default]
    Auto,
    Silent,
    AlertWrites,
    AlertAll,
    SafeWrites,
    SafeAll,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ConnectionOptions {
    pub show_system_schemas: bool,
    pub page_size: u32,
    pub query_row_limit: u32,
    pub safe_mode: SafeMode,
    /// Refuse writes; also set on the server session (READ ONLY transactions).
    pub read_only: bool,
    /// SQL run on every new connection of the pool (SET search_path, time zone…).
    pub startup_query: String,
    /// Ping interval that keeps idle connections and SSH tunnels open; 0 = off.
    pub keep_alive_secs: u32,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            show_system_schemas: false,
            page_size: 300,
            query_row_limit: 1000,
            safe_mode: SafeMode::Auto,
            read_only: false,
            startup_query: String::new(),
            keep_alive_secs: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group_id: Option<String>,
    pub driver: Driver,
    #[serde(default)]
    pub tag: Tag,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub db: DbSettings,
    #[serde(default)]
    pub ssh: SshSettings,
    #[serde(default)]
    pub tls: TlsSettings,
    #[serde(default)]
    pub options: ConnectionOptions,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl Connection {
    pub fn is_production(&self) -> bool {
        self.tag == Tag::Production
    }

    pub fn db_host(&self) -> String {
        if self.db.host.trim().is_empty() {
            "127.0.0.1".into()
        } else {
            self.db.host.trim().to_string()
        }
    }

    pub fn db_port(&self) -> u16 {
        self.db.port.or(self.driver.default_port()).unwrap_or(0)
    }

    pub fn ssh_port(&self) -> u16 {
        self.ssh.port.unwrap_or(22)
    }

    /// One-line summary for lists: `host:port/db` or the SQLite file path.
    pub fn summary(&self) -> String {
        match self.driver {
            Driver::Sqlite => self.db.file_path.clone(),
            _ => {
                let mut s = if self.db.use_socket && !self.db.socket.is_empty() {
                    self.db.socket.clone()
                } else {
                    format!("{}:{}", self.db_host(), self.db_port())
                };
                if !self.db.database.is_empty() {
                    s.push('/');
                    s.push_str(&self.db.database);
                }
                s
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    /// Always null in the MVP (one level of groups).
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub collapsed: bool,
}

/// How a form changes one secret. Empty strings are valid passwords, so "keep" and
/// "clear" are explicit instead of being encoded as None / "".
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SecretChange {
    #[default]
    Keep,
    Set {
        value: String,
    },
    Clear,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SecretsInput {
    pub db_password: SecretChange,
    pub ssh_password: SecretChange,
    /// Token returned by `import_ssh_key`; the key bytes never leave Rust.
    pub ssh_key_token: Option<String>,
    pub clear_ssh_key: bool,
    pub ssh_key_passphrase: SecretChange,
}

/// Secrets typed into a prompt for one connect attempt ("Ask every time", or a
/// passphrase the keychain does not have). `save` stores them in the keychain.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PromptedSecrets {
    pub db_password: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_key_passphrase: Option<String>,
    pub save_passphrase: bool,
}

/// What the UI lists: the connection plus which secrets exist (never their values).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionView {
    #[serde(flatten)]
    pub connection: Connection,
    pub summary: String,
    pub has_db_password: bool,
    pub has_ssh_password: bool,
    pub has_ssh_key: bool,
    pub has_ssh_key_passphrase: bool,
}
