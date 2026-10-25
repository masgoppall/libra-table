//! The query editor: commands run in order on the editor's own connection, so
//! `SELECT n`, `MULTI`/`EXEC`, `WATCH` and `CLIENT SETNAME` carry over between runs.
//! Cancelling unblocks a blocking command (BLPOP, XREAD BLOCK, WAIT…) with
//! `CLIENT UNBLOCK`; other commands finish quickly and can't be stopped midway.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Instant;

use redis::aio::MultiplexedConnection;
use redis::{Client, Value as Reply};
use tokio::sync::Mutex;

use crate::db::engine::is_cancelled;
use crate::db::exec::StatementResult;
use crate::error::{AppError, AppResult};

use super::conn::{self, command_error};
use super::{args, commands, results};

/// The editor connection, opened on first use, and its `CLIENT ID` for cancelling.
#[derive(Default)]
pub struct Editor {
    conn: Mutex<Option<MultiplexedConnection>>,
    client_id: AtomicI64,
}

impl Editor {
    pub fn client_id(&self) -> Option<i64> {
        Some(self.client_id.load(Ordering::SeqCst)).filter(|id| *id > 0)
    }

    pub async fn close(&self) {
        *self.conn.lock().await = None;
        self.client_id.store(0, Ordering::SeqCst);
    }

    async fn open(
        &self,
        client: &Client,
        tunnel: bool,
        startup: &str,
    ) -> AppResult<MultiplexedConnection> {
        let mut c = conn::dedicated(client, tunnel).await?;
        let id: Reply = redis::cmd("CLIENT")
            .arg("ID")
            .query_async(&mut c)
            .await
            .map_err(command_error)?;
        self.client_id
            .store(super::values::int(&id).unwrap_or(0), Ordering::SeqCst);
        for st in args::split(startup) {
            let cmd = args::parse(&st.text)?;
            command(&cmd)
                .query_async::<Reply>(&mut c)
                .await
                .map_err(|e| {
                    AppError::validation(format!(
                        "The startup command \"{}\" failed: {}",
                        st.text,
                        conn::server_message(&e)
                    ))
                })?;
        }
        Ok(c)
    }

    /// Runs `statements`, stopping at the first error or when cancelled.
    pub async fn run(
        &self,
        target: (&Client, bool, &str),
        statements: &[String],
        limit: usize,
        cancelled: &AtomicBool,
    ) -> Vec<StatementResult> {
        let mut out = Vec::new();
        let mut slot = self.conn.lock().await;
        for sql in statements {
            if is_cancelled(cancelled) {
                break;
            }
            let mut r = StatementResult::new(sql);
            let started = Instant::now();
            let failed = match parse(sql) {
                Err(e) => Some(e),
                Ok(cmd) => {
                    if slot.is_none() {
                        match self.open(target.0, target.1, target.2).await {
                            Ok(c) => *slot = Some(c),
                            Err(e) => {
                                r.error = Some(e);
                                out.push(r);
                                break;
                            }
                        }
                    }
                    let c = slot.as_mut().expect("editor connection");
                    let sent = command(&cmd).query_async::<Reply>(c).await;
                    let quit = cmd[0].eq_ignore_ascii_case(b"QUIT");
                    match sent {
                        Ok(reply) => {
                            results::table(&mut r, &cmd, reply, limit);
                            if quit {
                                *slot = None;
                            }
                            None
                        }
                        Err(e) => {
                            if e.is_io_error() || e.is_connection_dropped() || quit {
                                *slot = None;
                            }
                            Some(failure(e, is_cancelled(cancelled)))
                        }
                    }
                }
            };
            r.duration_ms = started.elapsed().as_millis() as u64;
            let stop = failed.is_some();
            r.error = failed;
            out.push(r);
            if stop {
                break;
            }
        }
        out
    }
}

fn parse(sql: &str) -> AppResult<Vec<Vec<u8>>> {
    let cmd = args::parse(sql)?;
    if cmd.is_empty() {
        return Err(AppError::validation("There is no command to run."));
    }
    if let Some(why) = commands::refused(&cmd) {
        return Err(AppError::unsupported(why));
    }
    Ok(cmd)
}

fn failure(e: redis::RedisError, cancelled: bool) -> AppError {
    if cancelled && conn::server_message(&e).contains("UNBLOCKED") {
        return AppError::cancelled();
    }
    command_error(e)
}

/// A command from its arguments.
pub fn command(parts: &[Vec<u8>]) -> redis::Cmd {
    let mut c = redis::Cmd::new();
    for a in parts {
        c.arg(a.as_slice());
    }
    c
}
