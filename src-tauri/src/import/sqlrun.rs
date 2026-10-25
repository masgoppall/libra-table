//! Running the pieces of a SQL file in order on one connection, with progress, stop on
//! error or continue, and cancel. The connection is closed afterwards instead of going
//! back to the pool: a dump changes session settings (`search_path`,
//! `FOREIGN_KEY_CHECKS`) that other work on the pool must not inherit.

use std::fs::File;
use std::io::BufReader;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::Dialect;
use crate::state::Session;

use super::conn::Conn;
use super::source;
use super::sqlfile::{Piece, StatementReader};

const MAX_REPORTED: usize = 100;
const SQL_SHOWN: usize = 300;
const PROGRESS_EVERY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlFileRequest {
    pub path: String,
    #[serde(default)]
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlFileProgress {
    pub statements: u64,
    pub failed: u64,
    pub bytes_read: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementError {
    pub index: u64,
    pub line: u64,
    /// The start of the statement.
    pub sql: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlFileSummary {
    /// Statements run (including the failed ones).
    pub statements: u64,
    pub failed: u64,
    pub rows_affected: u64,
    pub errors: Vec<StatementError>,
    /// psql meta-commands (`\connect`, `\restrict`) that were left out.
    pub skipped_commands: u64,
    pub cancelled: bool,
    pub stopped: bool,
    pub fatal: Option<AppError>,
}

enum Msg {
    Piece(Piece),
    End { skipped_commands: u64 },
    Fatal(AppError),
}

/// The first statements of a file, for the confirmation on production connections.
/// Blocking: call it off the async runtime.
pub fn first_statements(path: &str, driver: Dialect, n: usize) -> AppResult<Vec<String>> {
    let file = BufReader::new(File::open(path)?);
    let mut out = Vec::new();
    for piece in StatementReader::new(file, driver) {
        match piece? {
            Piece::Statement { sql, .. } | Piece::CopyStart { sql, .. } => out.push(sql),
            _ => continue,
        }
        if out.len() >= n {
            break;
        }
    }
    Ok(out)
}

fn produce(path: String, driver: Dialect, tx: mpsc::Sender<Msg>, read: Arc<AtomicU64>) {
    let reader = match source::open(&path, read) {
        Ok((r, _)) => r,
        Err(e) => {
            let _ = tx.blocking_send(Msg::Fatal(e));
            return;
        }
    };
    let mut pieces = StatementReader::new(BufReader::new(reader), driver);
    for piece in pieces.by_ref() {
        let msg = match piece {
            Ok(p) => Msg::Piece(p),
            Err(e) => Msg::Fatal(e),
        };
        if tx.blocking_send(msg).is_err() {
            return;
        }
    }
    let skipped_commands = pieces.skipped_commands;
    let _ = tx.blocking_send(Msg::End { skipped_commands });
}

/// Runs the file. `progress` is called every ~150 ms.
pub async fn run(
    session: &Session,
    req: &SqlFileRequest,
    cancel: &CancellationToken,
    mut progress: impl FnMut(&SqlFileProgress),
) -> AppResult<SqlFileSummary> {
    let total_bytes = std::fs::metadata(&req.path)?.len();
    let read = Arc::new(AtomicU64::new(0));
    let (tx, mut rx) = mpsc::channel(64);
    let (path, driver, counter) = (req.path.clone(), session.dialect(), read.clone());
    let producer = tokio::task::spawn_blocking(move || produce(path, driver, tx, counter));
    let mut runner = Runner {
        conn: Conn::acquire(&session.pool).await?,
        stop_on_error: req.stop_on_error,
        s: SqlFileSummary::default(),
    };
    let mut last = Instant::now();
    let mut report = |s: &SqlFileSummary, force: bool| {
        if force || last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            progress(&SqlFileProgress {
                statements: s.statements,
                failed: s.failed,
                bytes_read: read.load(Ordering::Relaxed),
                total_bytes,
            });
        }
    };
    runner.consume(&mut rx, cancel, &mut report).await;
    drop(rx);
    let _ = producer.await;
    runner.conn.discard();
    report(&runner.s, true);
    Ok(runner.s)
}

struct Runner {
    conn: Conn,
    stop_on_error: bool,
    s: SqlFileSummary,
}

impl Runner {
    async fn consume(
        &mut self,
        rx: &mut mpsc::Receiver<Msg>,
        cancel: &CancellationToken,
        report: &mut impl FnMut(&SqlFileSummary, bool),
    ) {
        while !self.s.stopped && !self.s.cancelled {
            let msg = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    self.s.cancelled = true;
                    return;
                }
                msg = rx.recv() => msg,
            };
            match msg {
                None => return,
                Some(Msg::End { skipped_commands }) => {
                    self.s.skipped_commands = skipped_commands;
                    return;
                }
                Some(Msg::Fatal(e)) => {
                    self.s.fatal = Some(e);
                    return;
                }
                Some(Msg::Piece(Piece::Statement { index, line, sql })) => {
                    self.statement(index, line, &sql, cancel).await
                }
                Some(Msg::Piece(Piece::CopyStart { index, line, sql })) => {
                    self.copy(index, line, &sql, rx).await
                }
                // Data outside a COPY can't happen; ignore it.
                Some(Msg::Piece(_)) => {}
            }
            report(&self.s, false);
        }
    }

    async fn statement(&mut self, index: u64, line: u64, sql: &str, cancel: &CancellationToken) {
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                // The statement was interrupted mid-way: the connection is not reusable.
                self.conn.discard();
                self.s.cancelled = true;
                return;
            }
            r = self.conn.raw(sql) => r,
        };
        self.s.statements += 1;
        match result {
            Ok(n) => self.s.rows_affected += n,
            Err(e) => self.failed(index, line, sql, e),
        }
    }

    fn failed(&mut self, index: u64, line: u64, sql: &str, e: AppError) {
        self.s.failed += 1;
        if self.s.errors.len() < MAX_REPORTED {
            let sql: String = sql.chars().take(SQL_SHOWN).collect();
            let message = e.message;
            self.s.errors.push(StatementError {
                index,
                line,
                sql,
                message,
            });
        }
        self.s.stopped = self.stop_on_error;
    }

    /// `COPY … FROM stdin`: streams the data pieces that follow into the server.
    async fn copy(&mut self, index: u64, line: u64, sql: &str, rx: &mut mpsc::Receiver<Msg>) {
        self.s.statements += 1;
        let result = match &mut self.conn {
            Conn::Postgres(conn) => copy_in(conn, sql, rx).await,
            _ => Err(AppError::validation("COPY … FROM stdin needs Postgres.")),
        };
        match result {
            Ok(n) => self.s.rows_affected += n,
            Err(e) => self.failed(index, line, sql, e),
        }
    }
}

async fn copy_in(
    conn: &mut sqlx::PgConnection,
    sql: &str,
    rx: &mut mpsc::Receiver<Msg>,
) -> AppResult<u64> {
    let mut copy = conn.copy_in_raw(sql).await.map_err(AppError::from);
    loop {
        match rx.recv().await {
            Some(Msg::Piece(Piece::CopyData(data))) => {
                // A failed send is an I/O error: the connection is gone anyway.
                let err = match copy.as_mut() {
                    Ok(c) => c.send(data).await.err(),
                    Err(_) => None,
                };
                if let Some(e) = err {
                    copy = Err(e.into());
                }
            }
            Some(Msg::Piece(Piece::CopyEnd)) => break,
            // The file ended or failed inside the COPY block.
            _ => {
                copy = Err(AppError::validation(
                    "The COPY data has no end marker (\\.).",
                ));
                break;
            }
        }
    }
    Ok(copy?.finish().await?)
}
