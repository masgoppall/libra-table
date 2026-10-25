//! One error type for every command. It serializes to `{kind, layer, message, hint, data}`
//! so the UI can say which layer failed (SSH, database, app) in one sentence.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Ssh,
    Database,
    App,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    SshUnreachable,
    SshAuthFailed,
    SshHostKeyUnknown,
    SshHostKeyChanged,
    SshPassphraseRequired,
    SshPassphraseWrong,
    SshKeyInvalid,
    TunnelFailed,
    DbConnect,
    DbAuth,
    DbNotFound,
    Tls,
    Query,
    NotConnected,
    ConfirmationRequired,
    /// A write on a connection marked read-only.
    ReadOnly,
    PasswordRequired,
    Validation,
    NotFound,
    Io,
    Keychain,
    Cancelled,
    /// The driver can't do this (the UI normally hides it).
    Unsupported,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub kind: ErrorKind,
    pub layer: Layer,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(kind: ErrorKind, layer: Layer, message: impl Into<String>) -> Self {
        Self {
            kind,
            layer,
            message: message.into(),
            hint: None,
            data: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, Layer::App, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, Layer::App, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, Layer::App, message)
    }

    pub fn not_connected() -> Self {
        Self::new(
            ErrorKind::NotConnected,
            Layer::App,
            "This connection is not open.",
        )
        .hint("Connect first, then try again.")
    }

    pub fn query(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Query, Layer::Database, message)
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, Layer::App, "Cancelled.")
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unsupported, Layer::App, message)
    }

    /// The server could not be reached (engines without sqlx errors use this).
    pub fn unreachable(detail: impl std::fmt::Display, through_tunnel: bool) -> Self {
        let via = if through_tunnel {
            " through the SSH tunnel"
        } else {
            ""
        };
        Self::new(
            ErrorKind::DbConnect,
            Layer::Database,
            format!("Can't reach the database server{via}: {detail}"),
        )
        .hint(if through_tunnel {
            UNREACHABLE_THROUGH_TUNNEL
        } else {
            UNREACHABLE_DIRECT
        })
    }

    /// The server refused the login or token.
    pub fn auth_failed(detail: impl std::fmt::Display) -> Self {
        Self::new(
            ErrorKind::DbAuth,
            Layer::Database,
            format!("Database login failed: {detail}"),
        )
        .hint("Check the user name and password (or token).")
    }

    /// Maps an error that happened while opening a database connection
    /// (directly or through a tunnel) to a kind the UI can explain.
    pub fn from_connect(err: sqlx::Error, through_tunnel: bool) -> Self {
        let via = if through_tunnel {
            " through the SSH tunnel"
        } else {
            ""
        };
        let db = |kind, message: String| Self::new(kind, Layer::Database, message);
        match &err {
            sqlx::Error::Database(e) => connect_db_error(e.as_ref(), via),
            sqlx::Error::Tls(e) => db(ErrorKind::Tls, format!("Invalid SSL configuration: {e}"))
                .hint("Try SSL mode \"prefer\", or check the CA, certificate and key files."),
            sqlx::Error::Io(e) => db(
                ErrorKind::DbConnect,
                format!("Can't reach the database server{via}: {e}"),
            )
            .hint(if through_tunnel {
                UNREACHABLE_THROUGH_TUNNEL
            } else {
                UNREACHABLE_DIRECT
            }),
            sqlx::Error::PoolTimedOut => db(
                ErrorKind::DbConnect,
                format!("Timed out while connecting to the database{via}."),
            )
            .hint("Check the host and port, and any firewall in between."),
            _ => db(
                ErrorKind::DbConnect,
                format!("Can't open the database connection{via}: {err}"),
            ),
        }
    }
}

const UNREACHABLE_THROUGH_TUNNEL: &str = "The tunnel is up, but the database host/port is \
    not reachable from the SSH server. When the database runs on the SSH server itself, \
    use host 127.0.0.1.";

const UNREACHABLE_DIRECT: &str =
    "Check the host and port, and that the server accepts connections from this machine.";

/// A connect error the server answered with: bad login, unknown database, or other.
fn connect_db_error(e: &dyn sqlx::error::DatabaseError, via: &str) -> AppError {
    let code = e.code().map(|c| c.to_string()).unwrap_or_default();
    let msg = e.message();
    // Postgres: 28P01 / 28000 auth, 3D000 unknown db. MySQL: 1045 auth, 1049 unknown db.
    match code.as_str() {
        "28P01" | "28000" | "1045" => AppError::new(
            ErrorKind::DbAuth,
            Layer::Database,
            format!("Database login failed: {msg}"),
        )
        .hint("Check the database user and password."),
        "3D000" | "1049" => AppError::new(
            ErrorKind::DbNotFound,
            Layer::Database,
            format!("The database does not exist: {msg}"),
        )
        .hint("Check the database name, or leave it empty to use the server default."),
        _ => AppError::new(
            ErrorKind::DbConnect,
            Layer::Database,
            format!("Can't open the database connection{via}: {msg}"),
        ),
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(db) => {
                Self::query(db.message().to_string()).data(serde_json::json!({
                    "code": db.code().map(|c| c.to_string()),
                }))
            }
            other => Self::query(other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorKind::Io, Layer::App, err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(
            ErrorKind::Io,
            Layer::App,
            format!("Invalid data file: {err}"),
        )
    }
}

impl From<keyring::Error> for AppError {
    fn from(err: keyring::Error) -> Self {
        Self::new(
            ErrorKind::Keychain,
            Layer::App,
            format!("Keychain error: {err}"),
        )
    }
}
