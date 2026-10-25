//! Query editor commands: splitting scripts, running them and cancelling a run.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::db::exec::{self, StatementResult};
use crate::db::split::Statement;
use crate::db::DbPool;
use crate::engines;
use crate::error::AppError;
use crate::models::Driver;
use crate::state::{AppState, CancelTarget, RunningQuery, Session};

use super::{confirm_safe_mode, record_history, Cmd, Outcome, Timer};

/// Bounds for the per-connection "rows kept per result" setting.
const MAX_ROW_LIMIT: u32 = 100_000;

#[tauri::command]
pub fn split_statements(driver: Driver, sql: String) -> Vec<Statement> {
    engines::split(driver, &sql)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    results: Vec<StatementResult>,
    cancelled: bool,
}

#[tauri::command]
pub async fn run_query(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    sql: String,
    query_id: String,
    confirmed: bool,
) -> Cmd<RunResponse> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let stmts: Vec<String> = engines::split(s.connection.driver, &sql)
        .into_iter()
        .map(|st| st.text)
        .collect();
    if stmts.is_empty() {
        return Err(AppError::validation("There is no SQL to run."));
    }
    confirm_safe_mode(&state, &s, &stmts, confirmed)?;
    let timer = Timer::start();
    let cancelled = Arc::new(AtomicBool::new(false));
    let results = run_registered(&state, &s, &stmts, &query_id, &cancelled).await?;
    let was_cancelled = cancelled.load(Ordering::SeqCst);
    let error = results
        .iter()
        .find_map(|r| r.error.as_ref().map(|e| e.message.clone()));
    let outcome = Outcome {
        status: run_status(was_cancelled, error.is_some()),
        row_count: Some(row_count(&results)),
        error: error.as_deref(),
    };
    record_history(&state, &s, sql.trim(), &timer, outcome).await;
    Ok(RunResponse {
        results,
        cancelled: was_cancelled,
    })
}

/// Runs on the editor connection while the run is registered under `query_id`, so
/// `cancel_query` can find it.
async fn run_registered(
    state: &AppState,
    s: &Session,
    stmts: &[String],
    query_id: &str,
    cancelled: &Arc<AtomicBool>,
) -> Cmd<Vec<StatementResult>> {
    let limit = s.connection.options.query_row_limit.clamp(1, MAX_ROW_LIMIT) as usize;
    if let DbPool::Engine(e) = &s.pool {
        let running = RunningQuery {
            target: CancelTarget::Engine(e.clone()),
            cancelled: cancelled.clone(),
        };
        state
            .running
            .lock()
            .unwrap()
            .insert(query_id.to_string(), running);
        let results = e.run(stmts, limit, cancelled).await;
        state.running.lock().unwrap().remove(query_id);
        return Ok(results);
    }
    let mut slot = s.editor.lock().await;
    let target = exec::prepare(s, &mut slot).await?;
    let running = RunningQuery {
        target,
        cancelled: cancelled.clone(),
    };
    state
        .running
        .lock()
        .unwrap()
        .insert(query_id.to_string(), running);
    let results = exec::run_locked(&mut slot, stmts, limit, cancelled).await;
    state.running.lock().unwrap().remove(query_id);
    Ok(results)
}

fn run_status(cancelled: bool, failed: bool) -> &'static str {
    if cancelled {
        "cancelled"
    } else if failed {
        "error"
    } else {
        "ok"
    }
}

/// Rows returned or changed by the whole run.
fn row_count(results: &[StatementResult]) -> i64 {
    results
        .iter()
        .map(|r| r.rows_affected.unwrap_or(r.total_rows) as i64)
        .sum()
}

#[tauri::command]
pub async fn cancel_query(state: State<'_, AppState>, query_id: String) -> Cmd<bool> {
    let target = {
        let running = state.running.lock().unwrap();
        let Some(r) = running.get(&query_id) else {
            return Ok(false);
        };
        r.cancelled.store(true, Ordering::SeqCst);
        if let CancelTarget::Sqlite { handle } = r.target {
            // Interrupt while the registry lock guarantees the connection is alive.
            exec::interrupt_sqlite(handle);
            return Ok(true);
        }
        r.target.clone()
    };
    let still_running = || state.running.lock().unwrap().contains_key(&query_id);
    exec::cancel(&target, still_running).await?;
    Ok(true)
}
