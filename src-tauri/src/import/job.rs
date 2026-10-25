//! The consuming side of an import: rows are inserted in chunks, each inside a savepoint,
//! committed per run or per batch. A failing chunk is retried row by row to find the
//! bad rows, which are skipped or stop the run.

use std::time::Instant;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::AppResult;

use super::conn::Conn;
use super::insert::Target;
use super::run::{End, ImportSummary, Item, RowError, PROGRESS_EVERY};
use super::source::Record;
use super::{ImportRequest, OnError, TxScope};

/// Row errors kept for the summary; the rest are only counted.
const MAX_REPORTED: usize = 200;
const SAVEPOINT: &str = "SAVEPOINT libratable_import";
const ROLLBACK_TO: &str = "ROLLBACK TO SAVEPOINT libratable_import";
const RELEASE: &str = "RELEASE SAVEPOINT libratable_import";

/// Rows waiting to be inserted, with where they came from.
#[derive(Default)]
struct Chunk {
    rows: Vec<Vec<Option<String>>>,
    at: Vec<(u64, Option<u64>)>,
}

pub(super) struct Job<'a> {
    conn: Conn,
    target: &'a Target,
    req: &'a ImportRequest,
    per_statement: usize,
    in_tx: bool,
    /// Rows inserted in the open transaction.
    pub(super) uncommitted: u64,
    /// Rows handled since the last commit (per-batch transactions).
    in_batch: usize,
    pub(super) s: ImportSummary,
}

impl<'a> Job<'a> {
    pub(super) fn new(conn: Conn, target: &'a Target, req: &'a ImportRequest) -> Self {
        Self {
            conn,
            target,
            req,
            per_statement: target.rows_per_statement(req.batch_size),
            in_tx: false,
            uncommitted: 0,
            in_batch: 0,
            s: ImportSummary::default(),
        }
    }

    pub(super) async fn consume(
        &mut self,
        rx: &mut mpsc::Receiver<Item>,
        cancel: &CancellationToken,
        report: &mut impl FnMut(&Job),
    ) -> End {
        let mut chunk = Chunk::default();
        let mut last = Instant::now();
        loop {
            let item = tokio::select! {
                biased;
                _ = cancel.cancelled() => return End::Cancelled,
                item = rx.recv() => item,
            };
            let step = match item {
                None => break,
                Some(Item::Fatal(e)) => return End::Failed(e),
                Some(Item::Row(r)) => self.take(r, &mut chunk),
            };
            if let Err(end) = step {
                return end;
            }
            if chunk.rows.len() >= self.per_statement {
                if let Err(end) = self.flush(&mut chunk).await {
                    return end;
                }
                if last.elapsed() >= PROGRESS_EVERY {
                    report(self);
                    last = Instant::now();
                }
            }
        }
        match self.flush(&mut chunk).await {
            Ok(()) => End::Done,
            Err(end) => end,
        }
    }

    /// Adds a record to the chunk, mapped to the target columns.
    fn take(&mut self, r: Record, chunk: &mut Chunk) -> Result<(), End> {
        self.s.rows_read += 1;
        match r.values {
            Ok(values) => {
                let mapped = self
                    .req
                    .mapping
                    .iter()
                    .map(|m| values.get(m.source).cloned().flatten())
                    .collect();
                chunk.rows.push(mapped);
                chunk.at.push((r.row, r.line));
                Ok(())
            }
            Err(message) => self.row_failed(r.row, r.line, message),
        }
    }

    fn row_failed(&mut self, row: u64, line: Option<u64>, message: String) -> Result<(), End> {
        self.s.error_count += 1;
        self.s.skipped += 1;
        if self.s.errors.len() < MAX_REPORTED {
            self.s.errors.push(RowError { row, line, message });
        }
        match self.req.on_error {
            OnError::Stop => Err(End::Stopped),
            OnError::Skip => Ok(()),
        }
    }

    /// Inserts the chunk in one statement, or row by row when that fails.
    async fn flush(&mut self, chunk: &mut Chunk) -> Result<(), End> {
        if chunk.rows.is_empty() {
            return Ok(());
        }
        if !self.in_tx {
            self.conn.raw("BEGIN").await?;
            self.in_tx = true;
        }
        let n = chunk.rows.len();
        self.conn.raw(SAVEPOINT).await?;
        let sql = self.target.sql(n);
        match self.conn.exec(&sql, &self.target.binds(&chunk.rows)).await {
            Ok(_) => {
                self.conn.raw(RELEASE).await?;
                self.uncommitted += n as u64;
            }
            Err(_) => {
                self.conn.raw(ROLLBACK_TO).await?;
                self.conn.raw(RELEASE).await?;
                self.row_by_row(chunk).await?;
            }
        }
        let rows = std::mem::take(chunk);
        self.in_batch += rows.rows.len();
        if self.req.transaction == TxScope::Batch && self.in_batch >= self.req.batch_size {
            self.commit().await?;
        }
        Ok(())
    }

    async fn row_by_row(&mut self, chunk: &Chunk) -> Result<(), End> {
        let sql = self.target.sql(1);
        for (row, &(n, line)) in chunk.rows.iter().zip(&chunk.at) {
            self.conn.raw(SAVEPOINT).await?;
            let binds = self.target.binds(std::slice::from_ref(row));
            match self.conn.exec(&sql, &binds).await {
                Ok(_) => {
                    self.conn.raw(RELEASE).await?;
                    self.uncommitted += 1;
                }
                Err(e) => {
                    self.conn.raw(ROLLBACK_TO).await?;
                    self.conn.raw(RELEASE).await?;
                    self.row_failed(n, line, e.message)?;
                }
            }
        }
        Ok(())
    }

    async fn commit(&mut self) -> AppResult<()> {
        if self.in_tx {
            self.conn.raw("COMMIT").await?;
            self.in_tx = false;
        }
        self.s.inserted += self.uncommitted;
        self.uncommitted = 0;
        self.in_batch = 0;
        Ok(())
    }

    /// Commits a finished run, rolls back an unfinished one, and returns the summary.
    pub(super) async fn finish(mut self, end: End) -> ImportSummary {
        let end = match end {
            End::Done => match self.commit().await {
                Ok(()) => End::Done,
                Err(e) => End::Failed(e),
            },
            other => other,
        };
        match end {
            End::Done => {}
            End::Stopped => self.s.stopped = true,
            End::Cancelled => self.s.cancelled = true,
            End::Failed(e) => self.s.fatal = Some(e),
        }
        if self.in_tx {
            self.s.rolled_back = self.uncommitted > 0;
            if self.conn.raw("ROLLBACK").await.is_err() {
                self.conn.discard();
            }
        }
        self.s
    }
}
