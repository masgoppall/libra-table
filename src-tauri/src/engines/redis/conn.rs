//! Connecting: address (or the SSH tunnel's local end), TLS, ACL user and password,
//! database index, and turning client errors into errors the UI can explain.

use std::time::Duration;

use redis::aio::{ConnectionManager, ConnectionManagerConfig, MultiplexedConnection};
use redis::{
    AsyncConnectionConfig, Client, ClientTlsConfig, ConnectionAddr, ConnectionInfo,
    ProtocolVersion, RedisConnectionInfo, RedisError, TlsCertificates,
};

use crate::db::engine::OpenArgs;
use crate::error::{AppError, AppResult, ErrorKind, Layer};
use crate::models::TlsMode;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Browser calls (SCAN pages, key reads) never wait longer than this.
const BROWSER_TIMEOUT: Duration = Duration::from_secs(60);
/// Reconnects of the shared browser connection after it drops.
const BROWSER_RETRIES: usize = 3;

/// The database index from the form ("", "3" or "db3").
pub fn parse_db(text: &str) -> AppResult<i64> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(0);
    }
    let digits = t
        .strip_prefix("db")
        .or_else(|| t.strip_prefix("DB"))
        .unwrap_or(t);
    digits
        .parse::<i64>()
        .ok()
        .filter(|n| *n >= 0)
        .ok_or_else(|| {
            AppError::validation(format!("\"{t}\" is not a Redis database index."))
                .hint("Use a number such as 0, or leave the field empty for database 0.")
        })
}

pub fn db_name(index: i64) -> String {
    format!("db{index}")
}

fn read_file(path: &str, what: &str) -> AppResult<Vec<u8>> {
    std::fs::read(path).map_err(|e| {
        AppError::new(
            ErrorKind::Tls,
            Layer::App,
            format!("Can't read the {what} file {path}: {e}"),
        )
    })
}

/// Redis serves TLS on its own port (`tls-port`) with no in-band upgrade, so "prefer"
/// can't try TLS and fall back cheaply (a TLS hello to a plain port just hangs): Off,
/// Allow and Prefer (the form's default) connect in plain text; Require and Verify use
/// TLS.
pub fn uses_tls(mode: TlsMode) -> bool {
    matches!(
        mode,
        TlsMode::Require | TlsMode::VerifyCa | TlsMode::VerifyFull
    )
}

/// The client for `args`, working in database `db`.
pub fn client(a: &OpenArgs<'_>, db: i64) -> AppResult<Client> {
    let (host, port) = a.host_port();
    let tls = &a.conn.tls;
    let addr = if !uses_tls(tls.mode) {
        ConnectionAddr::Tcp(host, port)
    } else {
        // Require encrypts without checking the certificate, as libpq does; Verify CA/Full
        // check it (rustls always checks the host name too).
        let verify = matches!(tls.mode, TlsMode::VerifyCa | TlsMode::VerifyFull);
        ConnectionAddr::TcpTls {
            host,
            port,
            insecure: !verify,
            tls_params: None,
        }
    };
    let user = a.conn.db.user.trim();
    let info = ConnectionInfo {
        addr,
        redis: RedisConnectionInfo {
            db,
            username: (!user.is_empty()).then(|| user.to_string()),
            password: a.secret.filter(|s| !s.is_empty()).map(str::to_string),
            protocol: ProtocolVersion::RESP2,
        },
    };
    let has_files = !tls.ca_path.trim().is_empty() || !tls.cert_path.trim().is_empty();
    let built = if uses_tls(tls.mode) && has_files {
        let root_cert = match tls.ca_path.trim() {
            "" => None,
            p => Some(read_file(p, "CA certificate")?),
        };
        let client_tls = match (tls.cert_path.trim(), tls.key_path.trim()) {
            ("", _) => None,
            (cert, key) => Some(ClientTlsConfig {
                client_cert: read_file(cert, "client certificate")?,
                client_key: read_file(if key.is_empty() { cert } else { key }, "client key")?,
            }),
        };
        Client::build_with_tls(
            info,
            TlsCertificates {
                client_tls,
                root_cert,
            },
        )
    } else {
        Client::open(info)
    };
    built.map_err(|e| connect_error(e, a.through_tunnel()))
}

/// A connection of its own (editor, edits): commands on it don't mix with others.
pub async fn dedicated(client: &Client, through_tunnel: bool) -> AppResult<MultiplexedConnection> {
    let config = AsyncConnectionConfig::new().set_connection_timeout(CONNECT_TIMEOUT);
    client
        .get_multiplexed_async_connection_with_config(&config)
        .await
        .map_err(|e| connect_error(e, through_tunnel))
}

/// The shared browser connection, reconnecting by itself after a drop.
pub async fn browser(client: &Client, through_tunnel: bool) -> AppResult<ConnectionManager> {
    let config = ConnectionManagerConfig::new()
        .set_connection_timeout(CONNECT_TIMEOUT)
        .set_response_timeout(BROWSER_TIMEOUT)
        .set_number_of_retries(BROWSER_RETRIES);
    client
        .get_connection_manager_with_config(config)
        .await
        .map_err(|e| connect_error(e, through_tunnel))
}

/// The server's words for an error: `WRONGTYPE Operation against…`.
pub fn server_message(e: &RedisError) -> String {
    match (e.code(), e.detail()) {
        (Some(code), Some(detail)) => format!("{code} {detail}"),
        (None, Some(detail)) => detail.to_string(),
        _ => e.to_string(),
    }
}

fn is_auth(e: &RedisError) -> bool {
    if e.kind() == redis::ErrorKind::AuthenticationFailed {
        return true;
    }
    let msg = server_message(e);
    [
        "WRONGPASS",
        "NOAUTH",
        "NOPERM",
        "invalid password",
        "AUTH <password>",
    ]
    .iter()
    .any(|m| msg.contains(m))
}

/// Errors while connecting: unreachable, refused login, bad TLS or database index.
pub fn connect_error(e: RedisError, through_tunnel: bool) -> AppError {
    let msg = server_message(&e);
    if is_auth(&e) {
        return AppError::auth_failed(msg)
            .hint("Check the user (ACL) and password. Leave the user empty for `default`.");
    }
    if msg.contains("DB index is out of range") {
        return AppError::validation("The server doesn't have that database index.")
            .hint("Redis has databases 0–15 unless `databases` is set higher.");
    }
    if e.kind() == redis::ErrorKind::InvalidClientConfig || msg.to_lowercase().contains("tls") {
        return AppError::new(
            ErrorKind::Tls,
            Layer::Database,
            format!("TLS failed: {msg}"),
        )
        .hint("Check the TLS mode and certificates; managed Redis usually needs TLS on.");
    }
    if e.is_io_error() || e.is_timeout() || e.is_connection_refusal() {
        return AppError::unreachable(msg, through_tunnel);
    }
    AppError::query(msg)
}

/// Errors of commands on an open connection.
pub fn command_error(e: RedisError) -> AppError {
    if e.is_io_error() || e.is_timeout() || e.is_connection_dropped() {
        return AppError::new(
            ErrorKind::DbConnect,
            Layer::Database,
            format!("Lost the connection to Redis: {}", server_message(&e)),
        )
        .hint("LibraTable reconnects by itself; try again.");
    }
    AppError::query(server_message(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_index() {
        assert_eq!(parse_db("").unwrap(), 0);
        assert_eq!(parse_db(" 3 ").unwrap(), 3);
        assert_eq!(parse_db("db12").unwrap(), 12);
        assert!(parse_db("-1").is_err());
        assert!(parse_db("cache").is_err());
        assert_eq!(db_name(4), "db4");
        assert!(!uses_tls(TlsMode::Prefer) && !uses_tls(TlsMode::Disable));
        assert!(uses_tls(TlsMode::Require) && uses_tls(TlsMode::VerifyFull));
    }
}
