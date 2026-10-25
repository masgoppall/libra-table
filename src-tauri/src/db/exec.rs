//! Runs editor scripts on the session's dedicated connection (QRY-3, QRY-4).
//! Statements run one by one; each gets its own result. Rows past the limit are
//! counted but not kept. A run stops at the first error. Cancelling stops the query
//! on the server (pg_cancel_backend / KILL QUERY / sqlite3_interrupt).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use futures::TryStreamExt;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Column, Either, Executor, TypeInfo};

use crate::db::value::{self, ColumnMeta};
use crate::db::DbPool;
use crate::error::{AppError, AppResult, ErrorKind, Layer};
use crate::state::{CancelTarget, EditorConn, Session};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementResult {
    pub sql: String,
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Value>>,
    /// Rows the statement returned in total (may exceed `rows.len()`).
    pub total_rows: u64,
    pub truncated: bool,
    pub rows_affected: Option<u64>,
    pub duration_ms: u64,
    pub error: Option<AppError>,
}

impl StatementResult {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            columns: Vec::new(),
            rows: Vec::new(),
            total_rows: 0,
            truncated: false,
            rows_affected: None,
            duration_ms: 0,
            error: None,
        }
    }
}

/// Statements that return rows even when empty, so we describe their columns.
fn returns_rows(sql: &str) -> bool {
    let first = sql
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(
        first.as_str(),
        "SELECT"
            | "WITH"
            | "SHOW"
            | "VALUES"
            | "TABLE"
            | "EXPLAIN"
            | "PRAGMA"
            | "DESCRIBE"
            | "DESC"
    )
}

/// Makes sure the editor connection exists and returns how to cancel work on it.
async fn ensure_editor(
    session: &Session,
    slot: &mut Option<EditorConn>,
) -> AppResult<CancelTarget> {
    if slot.is_none() {
        *slot = Some(match &session.pool {
            DbPool::Postgres(p) => {
                let mut conn = p.acquire().await?;
                let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                    .fetch_one(&mut *conn)
                    .await?;
                EditorConn::Postgres { conn, pid }
            }
            DbPool::Mysql(p) => {
                let mut conn = p.acquire().await?;
                let id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
                    .fetch_one(&mut *conn)
                    .await?;
                EditorConn::Mysql { conn, id }
            }
            DbPool::Sqlite(p) => {
                let mut conn = p.acquire().await?;
                let handle = {
                    let mut locked = conn.lock_handle().await?;
                    locked.as_raw_handle().as_ptr() as usize
                };
                EditorConn::Sqlite { conn, handle }
            }
            // Engines run statements themselves (`Engine::run`).
            DbPool::Engine(_) => return Err(crate::db::engine::sqlx_only()),
        });
    }
    Ok(match (slot.as_ref().unwrap(), &session.pool) {
        (EditorConn::Postgres { pid, .. }, DbPool::Postgres(p)) => CancelTarget::Postgres {
            pool: p.clone(),
            pid: *pid,
        },
        (EditorConn::Mysql { id, .. }, DbPool::Mysql(p)) => CancelTarget::Mysql {
            pool: p.clone(),
            id: *id,
        },
        (EditorConn::Sqlite { handle, .. }, _) => CancelTarget::Sqlite { handle: *handle },
        _ => {
            return Err(AppError::internal(
                "Editor connection does not match the driver.",
            ))
        }
    })
}

/// Runs one statement on a connection (`&mut XxxConnection`) and fills `r`.
/// A macro rather than a generic fn: the three connection types share no trait that
/// allows both streaming and `describe` on the same `&mut` borrow.
macro_rules! run_one {
    ($conn:expr, $sql:expr, $limit:expr, $r:expr, $decode:path) => {{
        let conn = $conn;
        let sql: &str = $sql;
        let r: &mut StatementResult = $r;
        let res: AppResult<()> = async {
            let mut affected: Option<u64> = None;
            {
                let mut stream = sqlx::raw_sql(sql).fetch_many(&mut *conn);
                while let Some(item) = stream.try_next().await? {
                    match item {
                        Either::Left(done) => {
                            affected = Some(affected.unwrap_or(0) + done.rows_affected())
                        }
                        Either::Right(row) => {
                            if r.columns.is_empty() {
                                r.columns = value::columns(&row);
                            }
                            r.total_rows += 1;
                            if r.rows.len() < $limit {
                                r.rows.push($decode(&row, value::PREVIEW));
                            } else {
                                r.truncated = true;
                            }
                        }
                    }
                }
            }
            r.rows_affected = if r.total_rows > 0 { None } else { affected };
            if r.columns.is_empty() && returns_rows(sql) {
                if let Ok(desc) = (&mut *conn).describe(sql).await {
                    r.columns = desc
                        .columns()
                        .iter()
                        .map(|c| ColumnMeta {
                            name: c.name().to_string(),
                            type_name: c.type_info().name().to_string(),
                        })
                        .collect();
                    r.rows_affected = None;
                }
            }
            Ok(())
        }
        .await;
        res
    }};
}

/// Opens the editor connection and returns its cancel handle. The caller registers
/// the handle before calling `run_locked`.
pub async fn prepare(session: &Session, slot: &mut Option<EditorConn>) -> AppResult<CancelTarget> {
    ensure_editor(session, slot).await
}

pub async fn run_locked(
    slot: &mut Option<EditorConn>,
    statements: &[String],
    limit: usize,
    cancelled: &Arc<AtomicBool>,
) -> Vec<StatementResult> {
    let mut results = Vec::new();
    let mut broken = false;
    for sql in statements {
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        let started = Instant::now();
        let mut r = StatementResult::new(sql);
        let outcome = match slot.as_mut() {
            Some(EditorConn::Postgres { conn, .. }) => {
                run_one!(&mut **conn, sql, limit, &mut r, value::pg::row)
            }
            Some(EditorConn::Mysql { conn, .. }) => {
                run_one!(&mut **conn, sql, limit, &mut r, value::mysql::row)
            }
            Some(EditorConn::Sqlite { conn, .. }) => {
                run_one!(&mut **conn, sql, limit, &mut r, value::sqlite::row)
            }
            None => Err(AppError::not_connected()),
        };
        r.duration_ms = started.elapsed().as_millis() as u64;
        if let Err(mut e) = outcome {
            if cancelled.load(Ordering::SeqCst) {
                e = AppError::new(
                    ErrorKind::Cancelled,
                    Layer::Database,
                    "The query was cancelled.",
                );
            }
            if e.kind == ErrorKind::Query && is_connection_error(&e) {
                broken = true;
            }
            r.error = Some(e);
            results.push(r);
            break;
        }
        results.push(r);
    }
    if broken {
        // Reconnect on the next run.
        *slot = None;
    }
    results
}

fn is_connection_error(e: &AppError) -> bool {
    let m = e.message.to_ascii_lowercase();
    m.contains("connection")
        && (m.contains("closed")
            || m.contains("reset")
            || m.contains("broken")
            || m.contains("io error"))
}

/// Interrupts a running SQLite statement. Call it while the query is still registered
/// as running (the registry lock keeps the connection, and so the pointer, alive).
pub fn interrupt_sqlite(handle: usize) {
    // SAFETY: the pointer belongs to the editor connection, which stays open while the
    // query is registered; sqlite3_interrupt may be called from any thread.
    unsafe { libsqlite3_sys::sqlite3_interrupt(handle as *mut libsqlite3_sys::sqlite3) };
}

/// Stops the running query on the server (QRY-4). The cancel needs a second connection;
/// once it has one it re-checks `still_running` so a late cancel can't hit the next query
/// on the same editor connection.
pub async fn cancel(target: &CancelTarget, still_running: impl Fn() -> bool) -> AppResult<()> {
    match target {
        CancelTarget::Postgres { pool, pid } => {
            let mut conn = pool.acquire().await?;
            if still_running() {
                sqlx::query("SELECT pg_cancel_backend($1)")
                    .bind(pid)
                    .execute(&mut *conn)
                    .await?;
            }
        }
        CancelTarget::Mysql { pool, id } => {
            let mut conn = pool.acquire().await?;
            if still_running() {
                sqlx::query(&format!("KILL QUERY {id}"))
                    .execute(&mut *conn)
                    .await?;
            }
        }
        CancelTarget::Sqlite { handle } => {
            if still_running() {
                interrupt_sqlite(*handle);
            }
        }
        CancelTarget::Engine(e) => {
            if still_running() {
                e.cancel().await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_row_returning_statements() {
        assert!(returns_rows("  select 1"));
        assert!(returns_rows("WITH x AS (select 1) select * from x"));
        assert!(!returns_rows("(select 1)"));
        assert!(!returns_rows("update t set a=1"));
        assert!(returns_rows("PRAGMA table_info(t)"));
    }
}
