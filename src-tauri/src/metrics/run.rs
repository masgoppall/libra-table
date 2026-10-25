//! Running a widget's query read-only. Every statement must classify as a read (the
//! safe-mode classifier), and the database enforces it too: a `READ ONLY` transaction
//! on Postgres and MySQL, `PRAGMA query_only` on SQLite. The statements run through the
//! query editor's executor (`exec::run_locked`) on a pooled connection of their own, so
//! a dashboard never waits on, or disturbs, the editor's session.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::db::exec::{self, StatementResult};
use crate::db::{split, DbPool};
use crate::error::{AppError, AppResult};
use crate::models::Dialect;
use crate::safe_mode::{self, Kind};
use crate::state::{EditorConn, Session};

/// Rows a widget keeps at most (charts and tables of a dashboard stay small).
const MAX_ROWS: usize = 5000;

fn read_only_guard(driver: Dialect) -> (&'static str, &'static str) {
    match driver {
        Dialect::Postgres => ("BEGIN READ ONLY", "ROLLBACK"),
        Dialect::Mysql => ("START TRANSACTION READ ONLY", "ROLLBACK"),
        Dialect::Sqlite => ("PRAGMA query_only = ON", "PRAGMA query_only = OFF"),
    }
}

/// The statements of `sql`, refusing anything that is not a read.
pub fn read_statements(driver: Dialect, sql: &str) -> AppResult<Vec<String>> {
    let stmts: Vec<String> = split::split(driver, sql)
        .into_iter()
        .map(|s| s.text)
        .collect();
    if stmts.is_empty() {
        return Err(AppError::validation("The widget has no query yet."));
    }
    for st in &stmts {
        if safe_mode::classify(driver, st).kind != Kind::Read {
            let head: String = st.chars().take(60).collect();
            return Err(AppError::validation(format!(
                "Widgets only run read-only queries; \"{head}\" is not one."
            )));
        }
    }
    Ok(stmts)
}

async fn connection(pool: &DbPool) -> AppResult<EditorConn> {
    // The ids are only used to cancel editor runs; widget queries aren't cancelled.
    Ok(match pool {
        DbPool::Postgres(p) => EditorConn::Postgres {
            conn: p.acquire().await?,
            pid: 0,
        },
        DbPool::Mysql(p) => EditorConn::Mysql {
            conn: p.acquire().await?,
            id: 0,
        },
        DbPool::Sqlite(p) => EditorConn::Sqlite {
            conn: p.acquire().await?,
            handle: 0,
        },
        DbPool::Engine(_) => return Err(crate::db::engine::sqlx_only()),
    })
}

/// Runs statements that passed `read_statements` and returns the last one's result (or
/// the first error).
pub async fn run_widget_query(session: &Session, stmts: Vec<String>) -> AppResult<StatementResult> {
    let driver = session.dialect();
    let limit = (session.connection.options.query_row_limit as usize).clamp(1, MAX_ROWS);
    let (begin, end) = read_only_guard(driver);
    let mut slot = Some(connection(&session.pool).await?);
    let never = Arc::new(AtomicBool::new(false));
    let mut all = vec![begin.to_string()];
    all.extend(stmts);
    let results = exec::run_locked(&mut slot, &all, limit, &never).await;
    let reset = exec::run_locked(&mut slot, &[end.to_string()], 1, &never).await;
    if reset.iter().any(|r| r.error.is_some()) {
        discard(slot);
    }
    if let Some(e) = results.iter().find_map(|r| r.error.clone()) {
        return Err(e);
    }
    results
        .into_iter()
        .skip(1)
        .last()
        .ok_or_else(|| AppError::internal("The query returned no result."))
}

/// A connection whose read-only guard could not be lifted must not go back to the pool.
fn discard(slot: Option<EditorConn>) {
    match slot {
        Some(EditorConn::Postgres { mut conn, .. }) => conn.close_on_drop(),
        Some(EditorConn::Mysql { mut conn, .. }) => conn.close_on_drop(),
        Some(EditorConn::Sqlite { mut conn, .. }) => conn.close_on_drop(),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_reads_pass() {
        assert_eq!(
            read_statements(Dialect::Postgres, "select 1; select 2")
                .unwrap()
                .len(),
            2
        );
        assert!(read_statements(Dialect::Postgres, "select 1; delete from t").is_err());
        assert!(read_statements(Dialect::Mysql, "set @a = 1").is_err());
        assert!(read_statements(Dialect::Sqlite, "  ").is_err());
    }
}
