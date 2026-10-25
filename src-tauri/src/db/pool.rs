//! Connection pools per driver, built from the saved connection settings.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlSslMode};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgSslMode};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;

use crate::db::split;
use crate::error::{AppError, AppResult, ErrorKind, Layer};
use crate::models::{Connection, Driver, TlsMode};

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(15);
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// SQLite allows one writer; more connections only add lock contention.
const SQLITE_MAX_CONNECTIONS: u32 = 4;
/// Statements slower than this are marked in the console log.
const SLOW_STATEMENT: Duration = Duration::from_secs(1);

/// An `after_connect` hook running the setup statements (one per driver: sqlx's executor
/// lifetimes don't allow a generic function here).
macro_rules! setup_hook {
    ($setup:ident) => {
        move |c, _| {
            let setup = $setup.clone();
            Box::pin(async move {
                for sql in setup.iter() {
                    sqlx::Executor::execute(&mut *c, sqlx::raw_sql(sql)).await?;
                }
                Ok(())
            })
        }
    };
}

#[derive(Clone)]
pub enum DbPool {
    Postgres(PgPool),
    Mysql(MySqlPool),
    Sqlite(SqlitePool),
}

impl DbPool {
    pub fn driver(&self) -> Driver {
        match self {
            DbPool::Postgres(_) => Driver::Postgres,
            DbPool::Mysql(_) => Driver::Mysql,
            DbPool::Sqlite(_) => Driver::Sqlite,
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            DbPool::Postgres(p) => p.is_closed(),
            DbPool::Mysql(p) => p.is_closed(),
            DbPool::Sqlite(p) => p.is_closed(),
        }
    }

    pub async fn close(&self) {
        match self {
            DbPool::Postgres(p) => p.close().await,
            DbPool::Mysql(p) => p.close().await,
            DbPool::Sqlite(p) => p.close().await,
        }
    }

    pub async fn server_version(&self) -> AppResult<String> {
        use sqlx::query_scalar as scalar;
        Ok(match self {
            DbPool::Postgres(p) => scalar("SHOW server_version").fetch_one(p).await?,
            DbPool::Mysql(p) => scalar("SELECT VERSION()").fetch_one(p).await?,
            DbPool::Sqlite(p) => scalar("SELECT sqlite_version()").fetch_one(p).await?,
        })
    }
}

/// Where to reach the database: directly, or the local end of an SSH tunnel.
pub struct Target {
    pub host: String,
    pub port: u16,
    pub through_tunnel: bool,
}

/// What every driver needs to open a pool.
struct PoolArgs<'a> {
    conn: &'a Connection,
    target: Option<&'a Target>,
    password: Option<&'a str>,
    database: String,
    max_connections: u32,
}

impl PoolArgs<'_> {
    fn through_tunnel(&self) -> bool {
        self.target.map(|t| t.through_tunnel).unwrap_or(false)
    }

    /// A Unix socket is used only for a direct connection.
    fn socket(&self) -> Option<&str> {
        let db = &self.conn.db;
        (db.use_socket && !db.socket.is_empty() && self.target.is_none())
            .then_some(db.socket.as_str())
    }

    fn host_port(&self) -> (String, u16) {
        match self.target {
            Some(t) => (t.host.clone(), t.port),
            None => (self.conn.db_host(), self.conn.db_port()),
        }
    }
}

/// Opens a pool and one connection right away so errors surface at connect time.
pub async fn open_pool(
    conn: &Connection,
    target: Option<&Target>,
    password: Option<&str>,
    database: Option<&str>,
    max_connections: u32,
) -> AppResult<DbPool> {
    let args = PoolArgs {
        conn,
        target,
        password,
        database: database
            .map(str::to_string)
            .unwrap_or_else(|| conn.db.database.clone()),
        max_connections,
    };
    match conn.driver {
        Driver::Postgres => open_postgres(&args).await,
        Driver::Mysql => open_mysql(&args).await,
        Driver::Sqlite => open_sqlite(&args).await,
    }
}

fn pg_ssl_mode(mode: TlsMode) -> PgSslMode {
    match mode {
        TlsMode::Disable => PgSslMode::Disable,
        TlsMode::Allow => PgSslMode::Allow,
        TlsMode::Prefer => PgSslMode::Prefer,
        TlsMode::Require => PgSslMode::Require,
        TlsMode::VerifyCa => PgSslMode::VerifyCa,
        TlsMode::VerifyFull => PgSslMode::VerifyFull,
    }
}

fn mysql_ssl_mode(mode: TlsMode) -> MySqlSslMode {
    match mode {
        TlsMode::Disable => MySqlSslMode::Disabled,
        TlsMode::Allow | TlsMode::Prefer => MySqlSslMode::Preferred,
        TlsMode::Require => MySqlSslMode::Required,
        TlsMode::VerifyCa => MySqlSslMode::VerifyCa,
        TlsMode::VerifyFull => MySqlSslMode::VerifyIdentity,
    }
}

async fn open_postgres(a: &PoolArgs<'_>) -> AppResult<DbPool> {
    let (db, tls) = (&a.conn.db, &a.conn.tls);
    let mut o = PgConnectOptions::new().application_name("LibraTable");
    o = match a.socket() {
        Some(socket) => o.socket(socket),
        None => {
            let (h, p) = a.host_port();
            o.host(&h).port(p)
        }
    };
    if !db.user.is_empty() {
        o = o.username(&db.user);
    }
    if let Some(pw) = a.password {
        o = o.password(pw);
    }
    if !a.database.is_empty() {
        o = o.database(&a.database);
    }
    o = o.ssl_mode(pg_ssl_mode(tls.mode));
    if !tls.ca_path.is_empty() {
        o = o.ssl_root_cert(&tls.ca_path);
    }
    if !tls.cert_path.is_empty() {
        o = o.ssl_client_cert(&tls.cert_path);
    }
    if !tls.key_path.is_empty() {
        o = o.ssl_client_key(&tls.key_path);
    }
    let setup = session_setup(a.conn);
    let pool = PgPoolOptions::new()
        .max_connections(a.max_connections)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .after_connect(setup_hook!(setup))
        .connect_with(with_console_log(o))
        .await
        .map_err(|e| AppError::from_connect(e, a.through_tunnel()))?;
    Ok(DbPool::Postgres(pool))
}

async fn open_mysql(a: &PoolArgs<'_>) -> AppResult<DbPool> {
    let (db, tls) = (&a.conn.db, &a.conn.tls);
    let mut o = MySqlConnectOptions::new().charset("utf8mb4");
    o = match a.socket() {
        Some(socket) => o.socket(socket),
        None => {
            let (h, p) = a.host_port();
            o.host(&h).port(p)
        }
    };
    if !db.user.is_empty() {
        o = o.username(&db.user);
    }
    if let Some(pw) = a.password {
        o = o.password(pw);
    }
    if !a.database.is_empty() {
        o = o.database(&a.database);
    }
    o = o.ssl_mode(mysql_ssl_mode(tls.mode));
    if !tls.ca_path.is_empty() {
        o = o.ssl_ca(&tls.ca_path);
    }
    if !tls.cert_path.is_empty() {
        o = o.ssl_client_cert(&tls.cert_path);
    }
    if !tls.key_path.is_empty() {
        o = o.ssl_client_key(&tls.key_path);
    }
    let setup = session_setup(a.conn);
    let pool = MySqlPoolOptions::new()
        .max_connections(a.max_connections)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .after_connect(setup_hook!(setup))
        .connect_with(with_console_log(o))
        .await
        .map_err(|e| AppError::from_connect(e, a.through_tunnel()))?;
    Ok(DbPool::Mysql(pool))
}

async fn open_sqlite(a: &PoolArgs<'_>) -> AppResult<DbPool> {
    let path = a.conn.db.file_path.trim();
    if path.is_empty() {
        return Err(AppError::validation("Choose the SQLite database file."));
    }
    if !Path::new(path).exists() {
        return Err(AppError::new(
            ErrorKind::DbNotFound,
            Layer::Database,
            format!("The database file does not exist: {path}"),
        ));
    }
    let o = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .busy_timeout(SQLITE_BUSY_TIMEOUT)
        .read_only(a.conn.options.read_only);
    let o = with_console_log(o);
    let setup = session_setup(a.conn);
    let pool = SqlitePoolOptions::new()
        .max_connections(a.max_connections.min(SQLITE_MAX_CONNECTIONS))
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .after_connect(setup_hook!(setup))
        .connect_with(o)
        .await
        .map_err(|e| AppError::from_connect(e, false))?;
    Ok(DbPool::Sqlite(pool))
}

/// What every new connection of a pool runs first: the read-only guard (enforced by the
/// server, not only by the app), then the user's startup query, split into statements.
fn session_setup(conn: &Connection) -> Arc<Vec<String>> {
    let mut out = Vec::new();
    if conn.options.read_only {
        match conn.driver {
            Driver::Postgres => {
                out.push("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY".to_string())
            }
            Driver::Mysql => out.push("SET SESSION TRANSACTION READ ONLY".to_string()),
            // SQLite connections are opened read-only instead.
            Driver::Sqlite => {}
        }
    }
    let startup = split::split(conn.driver, &conn.options.startup_query);
    out.extend(startup.into_iter().map(|s| s.text));
    Arc::new(out)
}

/// Statements go to the console log (`crate::console`); slow ones are flagged.
fn with_console_log<O: ConnectOptions>(o: O) -> O {
    o.log_statements(log::LevelFilter::Debug)
        .log_slow_statements(log::LevelFilter::Warn, SLOW_STATEMENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(driver: &str) -> Connection {
        serde_json::from_value(serde_json::json!({ "id": "x", "name": "n", "driver": driver }))
            .unwrap()
    }

    #[test]
    fn setup_runs_read_only_guard_then_startup_query() {
        let mut c = conn("postgres");
        assert!(session_setup(&c).is_empty());
        c.options.read_only = true;
        c.options.startup_query = "SET search_path TO app; SET TIME ZONE 'UTC';".into();
        let setup = session_setup(&c);
        assert_eq!(setup.len(), 3);
        assert!(setup[0].contains("READ ONLY"));
        assert!(setup[2].starts_with("SET TIME ZONE"));
    }

    #[test]
    fn sqlite_read_only_is_a_connect_option() {
        let mut c = conn("sqlite");
        c.options.read_only = true;
        assert!(session_setup(&c).is_empty());
    }
}
