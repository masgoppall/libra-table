//! Data grid commands: pages, counts, filter previews and committing cell edits.

use std::collections::BTreeMap;

use tauri::State;

use crate::db::changes::{self, Change, CommitResult, Stmt};
use crate::db::data::{self, FilterPreview, PageRequest, PageResult, RowCount};
use crate::db::{cell, choices};
use crate::state::{AppState, Session};

use super::{confirm_production, record_history, Cmd, Outcome, Timer};

#[tauri::command]
pub async fn fetch_page(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: PageRequest,
) -> Cmd<PageResult> {
    let s = state.session_in(&id, database.as_deref()).await?;
    data::fetch_page(&s, &request).await
}

#[tauri::command]
pub async fn count_rows(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: PageRequest,
) -> Cmd<RowCount> {
    let s = state.session_in(&id, database.as_deref()).await?;
    data::count_rows(&s, &request).await
}

#[tauri::command]
pub async fn preview_filter(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    request: PageRequest,
) -> Cmd<FilterPreview> {
    let s = state.session_in(&id, database.as_deref()).await?;
    data::preview_filter(&s, &request).await
}

/// The statements for `changes` and their previews (values inlined).
async fn statements(
    s: &Session,
    schema: &str,
    table: &str,
    changes: &[Change],
) -> Cmd<(Vec<Stmt>, Vec<String>)> {
    let cols = s.columns(schema, table).await?;
    let stmts = changes::Builder::new(s.driver(), schema, table, &cols).build(changes)?;
    let previews = stmts
        .iter()
        .map(|st| changes::preview(s.driver(), st))
        .collect();
    Ok((stmts, previews))
}

#[tauri::command]
pub async fn preview_changes(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    table: String,
    changes: Vec<Change>,
) -> Cmd<Vec<String>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    Ok(statements(&s, &schema, &table, &changes).await?.1)
}

#[tauri::command]
pub async fn commit_changes(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    table: String,
    changes: Vec<Change>,
    confirmed: bool,
) -> Cmd<CommitResult> {
    let s = state.session_in(&id, database.as_deref()).await?;
    let (stmts, previews) = statements(&s, &schema, &table, &changes).await?;
    if previews.is_empty() {
        return Ok(CommitResult {
            affected: 0,
            statements: previews,
        });
    }
    confirm_production(&state, &s, &previews, confirmed)?;
    let timer = Timer::start();
    let result = changes::apply(&s.pool, &stmts).await;
    let outcome = match &result {
        Ok(n) => Outcome {
            status: "ok",
            row_count: Some(*n as i64),
            error: None,
        },
        Err(e) => Outcome {
            status: "error",
            row_count: None,
            error: Some(&e.message),
        },
    };
    record_history(&state, &s, &previews.join(";\n"), &timer, outcome).await;
    Ok(CommitResult {
        affected: result?,
        statements: previews,
    })
}

/// The whole value of one cell (Quick Look), found by the row's primary key.
#[tauri::command]
pub async fn fetch_cell(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    table: String,
    pk: serde_json::Map<String, serde_json::Value>,
    column: String,
) -> Cmd<cell::CellContent> {
    let s = state.session_in(&id, database.as_deref()).await?;
    cell::fetch_cell(&s, &schema, &table, &pk, &column).await
}

/// Allowed values of the table's enum columns (dropdown editors).
#[tauri::command]
pub async fn column_choices(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    table: String,
) -> Cmd<BTreeMap<String, Vec<String>>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    choices::column_choices(&s.pool, &schema, &table).await
}
