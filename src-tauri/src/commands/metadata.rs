//! Metadata commands: driver capabilities, databases, schemas, objects, columns,
//! structure and autocomplete names.
//!
//! Commands that take an optional `database` serve tabs of any database of the
//! server: Rust keeps one session per database it has seen (`Session::in_database`).

use serde::Serialize;
use tauri::State;

use crate::db::filter::{self, OperatorInfo};
use crate::db::meta::{self, ColumnInfo, CompletionSchema, ObjectInfo, TableStructure};
use crate::models::Driver;
use crate::state::AppState;

use super::Cmd;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    operators: Vec<OperatorInfo>,
    has_schemas: bool,
    switch_database: bool,
}

#[tauri::command]
pub fn driver_capabilities(driver: Driver) -> Capabilities {
    Capabilities {
        operators: filter::operators(driver),
        // Postgres: databases contain schemas. MySQL: schema == database. SQLite: attached dbs.
        has_schemas: driver == Driver::Postgres,
        switch_database: driver != Driver::Sqlite,
    }
}

#[tauri::command]
pub async fn list_databases(state: State<'_, AppState>, id: String) -> Cmd<Vec<String>> {
    let s = state.session(&id).await?;
    meta::list_databases(&s.pool, s.connection.options.show_system_schemas).await
}

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
) -> Cmd<Vec<String>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    meta::list_schemas(&s.pool, s.connection.options.show_system_schemas).await
}

#[tauri::command]
pub async fn list_objects(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
) -> Cmd<Vec<ObjectInfo>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    meta::list_objects(&s.pool, &schema).await
}

#[tauri::command]
pub async fn list_columns(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    table: String,
) -> Cmd<Vec<ColumnInfo>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    Ok(s.columns(&schema, &table).await?.as_ref().clone())
}

#[tauri::command]
pub async fn table_structure(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    table: String,
) -> Cmd<TableStructure> {
    let s = state.session_in(&id, database.as_deref()).await?;
    meta::table_structure(&s.pool, &schema, &table).await
}

#[tauri::command]
pub async fn completion_metadata(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
) -> Cmd<CompletionSchema> {
    let s = state.session_in(&id, database.as_deref()).await?;
    meta::completion(&s.pool, &schema, s.connection.options.show_system_schemas).await
}

#[tauri::command]
pub async fn refresh_metadata(state: State<'_, AppState>, id: String) -> Cmd<()> {
    state.session(&id).await?.clear_all_caches().await;
    Ok(())
}
