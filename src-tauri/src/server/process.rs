//! The server's process list (Postgres `pg_stat_activity`, MySQL `SHOW FULL PROCESSLIST`)
//! and the two ways to stop one: cancel its query or close its connection.
//!
//! Rows that belong to LibraTable itself are flagged `own`, so the UI can warn before
//! the user stops the very connection they are working on.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;

use crate::db::{value, DbPool};
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    /// Backend pid (Postgres) or connection id (MySQL).
    pub id: i64,
    pub user: String,
    pub database: String,
    /// `host:port` (or "local socket") of the client.
    pub client: String,
    pub application: String,
    /// "active", "idle in transaction"… (Postgres) or "Query · executing" (MySQL).
    pub state: String,
    /// Time in the current state (Postgres) or command (MySQL).
    pub duration_ms: Option<i64>,
    pub query: String,
    /// The connection is LibraTable's: the one listing, the editor, or another
    /// connection named "LibraTable" (Postgres application_name).
    pub own: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessAction {
    /// pg_cancel_backend / KILL QUERY: stop the running statement, keep the connection.
    Cancel,
    /// pg_terminate_backend / KILL: close the connection.
    Kill,
}

/// The statement that runs for `action` on process `id`. It is shown as is in the
/// confirmation; `id` is a number, so there is nothing to quote.
pub fn action_sql(driver: Dialect, action: ProcessAction, id: i64) -> AppResult<String> {
    if id <= 0 {
        return Err(AppError::validation("Invalid process id."));
    }
    Ok(match (driver, action) {
        (Dialect::Postgres, ProcessAction::Cancel) => format!("SELECT pg_cancel_backend({id})"),
        (Dialect::Postgres, ProcessAction::Kill) => format!("SELECT pg_terminate_backend({id})"),
        (Dialect::Mysql, ProcessAction::Cancel) => format!("KILL QUERY {id}"),
        (Dialect::Mysql, ProcessAction::Kill) => format!("KILL {id}"),
        (Dialect::Sqlite, _) => return Err(no_process_list()),
    })
}

fn no_process_list() -> AppError {
    AppError::validation("SQLite has no server, so there is no process list.")
}

const PG_LIST: &str = "SELECT pid, coalesce(usename::text, '') AS usr, \
    coalesce(datname::text, '') AS db, \
    CASE WHEN client_addr IS NULL THEN 'local socket' \
         ELSE host(client_addr) || ':' || client_port END AS client, \
    coalesce(application_name, '') AS app, coalesce(state, '') AS st, \
    (extract(epoch FROM clock_timestamp() - coalesce( \
        CASE WHEN state = 'active' THEN query_start ELSE state_change END, \
        backend_start)) * 1000)::bigint AS ms, \
    coalesce(query, '') AS q, pid = pg_backend_pid() AS me \
    FROM pg_stat_activity WHERE backend_type = 'client backend' ORDER BY pid";

/// The server's client connections. `own_ids` are LibraTable connections the caller
/// knows of (the editor connections); the listing connection is always added.
pub async fn list(pool: &DbPool, own_ids: &[i64]) -> AppResult<Vec<ProcessInfo>> {
    match pool {
        DbPool::Postgres(p) => {
            let rows = sqlx::query(PG_LIST).fetch_all(p).await?;
            rows.iter().map(|r| pg_row(r, own_ids)).collect()
        }
        DbPool::Mysql(p) => {
            let mut conn = p.acquire().await?;
            let me: i64 = sqlx::query_scalar("SELECT CAST(CONNECTION_ID() AS SIGNED)")
                .fetch_one(&mut *conn)
                .await?;
            let rows = sqlx::query("SHOW FULL PROCESSLIST")
                .fetch_all(&mut *conn)
                .await?;
            Ok(rows.iter().map(|r| mysql_row(r, me, own_ids)).collect())
        }
        DbPool::Sqlite(_) | DbPool::Engine(_) => Err(no_process_list()),
    }
}

fn pg_row(r: &sqlx::postgres::PgRow, own_ids: &[i64]) -> AppResult<ProcessInfo> {
    let id = r.try_get::<i32, _>("pid")? as i64;
    let application: String = r.try_get("app")?;
    let me: bool = r.try_get("me")?;
    Ok(ProcessInfo {
        id,
        user: r.try_get("usr")?,
        database: r.try_get("db")?,
        client: r.try_get("client")?,
        own: me || application == "LibraTable" || own_ids.contains(&id),
        application,
        state: r.try_get("st")?,
        duration_ms: r.try_get("ms")?,
        query: r.try_get("q")?,
    })
}

/// One `SHOW FULL PROCESSLIST` row, read by column name: MySQL and MariaDB differ in
/// the column types and MariaDB adds a few columns.
fn mysql_row(r: &sqlx::mysql::MySqlRow, me: i64, own_ids: &[i64]) -> ProcessInfo {
    use sqlx::Column;
    let cells = value::mysql::row(r, value::PREVIEW);
    let get = |name: &str| -> Value {
        r.columns()
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(name))
            .and_then(|i| cells.get(i).cloned())
            .unwrap_or(Value::Null)
    };
    let id = number(&get("Id")).unwrap_or(0);
    ProcessInfo {
        id,
        user: text(&get("User")),
        database: text(&get("db")),
        client: text(&get("Host")),
        application: String::new(),
        state: mysql_state(&text(&get("Command")), &text(&get("State"))),
        duration_ms: number(&get("Time")).map(|s| s * 1000),
        query: text(&get("Info")),
        own: id == me || own_ids.contains(&id),
    }
}

/// "Query · executing", or just "Sleep" when there is no state.
fn mysql_state(command: &str, state: &str) -> String {
    if state.is_empty() {
        command.to_string()
    } else {
        format!("{command} · {state}")
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn number(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Runs a cancel or kill. Postgres answers `false` when the process is gone or not
/// ours to stop; that becomes an error so the UI doesn't report success.
pub async fn run_action(pool: &DbPool, action: ProcessAction, id: i64) -> AppResult<()> {
    let dialect = pool.dialect().ok_or_else(crate::db::engine::sqlx_only)?;
    let sql = action_sql(dialect, action, id)?;
    match pool {
        DbPool::Postgres(p) => {
            let done: bool = sqlx::query_scalar(&sql).fetch_one(p).await?;
            if !done {
                return Err(AppError::query(format!(
                    "Process {id} was not stopped: it has ended, or you may not stop it."
                )));
            }
        }
        DbPool::Mysql(p) => {
            sqlx::query(&sql).execute(p).await?;
        }
        DbPool::Sqlite(_) | DbPool::Engine(_) => return Err(no_process_list()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_statements_per_driver() {
        let pg = |a| action_sql(Dialect::Postgres, a, 42).unwrap();
        assert_eq!(pg(ProcessAction::Cancel), "SELECT pg_cancel_backend(42)");
        assert_eq!(pg(ProcessAction::Kill), "SELECT pg_terminate_backend(42)");
        let my = |a| action_sql(Dialect::Mysql, a, 7).unwrap();
        assert_eq!(my(ProcessAction::Cancel), "KILL QUERY 7");
        assert_eq!(my(ProcessAction::Kill), "KILL 7");
        assert!(action_sql(Dialect::Sqlite, ProcessAction::Kill, 1).is_err());
        assert!(action_sql(Dialect::Mysql, ProcessAction::Kill, 0).is_err());
    }

    #[test]
    fn mysql_state_and_cells() {
        assert_eq!(mysql_state("Query", "executing"), "Query · executing");
        assert_eq!(mysql_state("Sleep", ""), "Sleep");
        assert_eq!(number(&Value::String("12".into())), Some(12));
        assert_eq!(number(&serde_json::json!(5)), Some(5));
        assert_eq!(text(&Value::Null), "");
    }
}
