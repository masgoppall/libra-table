//! The query editor's own Snowflake session (logged in on first use, startup query
//! run once), statement by statement, and cancelling the statement in flight.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use super::client::{new_request_id, Conn};
use super::engine::{hint, Snowflake};
use super::values;
use crate::db::engine::is_cancelled;
use crate::db::exec::StatementResult;
use crate::db::value;
use crate::engines::cloud_common::cancelled;
use crate::error::AppResult;

/// The editor statement in flight, for `cancel`.
pub struct Running {
    conn: Arc<Conn>,
    request_id: String,
    sql: String,
}

impl Snowflake {
    async fn editor_conn(&self) -> AppResult<Arc<Conn>> {
        let mut slot = self.editor.lock().await;
        if let Some(c) = slot.as_ref() {
            return Ok(c.clone());
        }
        let conn = Arc::new(Conn::login(self.http.clone(), &self.login).await?);
        self.startup(&conn).await?;
        *slot = Some(conn.clone());
        Ok(conn)
    }

    async fn run_one(
        &self,
        conn: &Arc<Conn>,
        sql: &str,
        limit: usize,
        flag: &AtomicBool,
    ) -> StatementResult {
        let started = Instant::now();
        let mut r = StatementResult::new(sql);
        let rid = new_request_id();
        *self.running.lock().unwrap() = Some(Running {
            conn: conn.clone(),
            request_id: rid.clone(),
            sql: sql.to_string(),
        });
        let res = conn.query(&rid, sql, &[], limit, Some(flag)).await;
        *self.running.lock().unwrap() = None;
        r.duration_ms = started.elapsed().as_millis() as u64;
        match res {
            Ok(out) if out.affected.is_some() => r.rows_affected = out.affected,
            Ok(out) => {
                r.columns = values::columns(&out.columns);
                r.rows = out
                    .rows
                    .iter()
                    .map(|row| values::row(&out.columns, row, value::PREVIEW))
                    .collect();
                r.total_rows = out.total.max(r.rows.len() as u64);
                r.truncated = r.total_rows > r.rows.len() as u64;
            }
            Err(_) if is_cancelled(flag) => r.error = Some(cancelled()),
            Err(e) => r.error = Some(hint(e)),
        }
        r
    }

    /// Runs statements in order on the editor session, stopping at the first error.
    pub(super) async fn run_editor(
        &self,
        statements: &[String],
        limit: usize,
        flag: &AtomicBool,
    ) -> Vec<StatementResult> {
        let conn = match self.editor_conn().await {
            Ok(c) => c,
            Err(e) => {
                let mut r = StatementResult::new(statements.first().map_or("", |s| s));
                r.error = Some(e);
                return vec![r];
            }
        };
        let mut results = Vec::new();
        for sql in statements {
            if is_cancelled(flag) {
                break;
            }
            let r = self.run_one(&conn, sql, limit, flag).await;
            let failed = r.error.is_some();
            results.push(r);
            if failed {
                break;
            }
        }
        results
    }

    /// Sends abort-request for the editor statement in flight, if any.
    pub(super) async fn cancel_editor(&self) -> AppResult<()> {
        let running = self
            .running
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| (r.conn.clone(), r.request_id.clone(), r.sql.clone()));
        match running {
            Some((conn, rid, sql)) => conn.abort(&rid, &sql).await,
            None => Ok(()),
        }
    }
}
