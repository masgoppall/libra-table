//! Schema metadata: databases, schemas, tables/views, columns, structure and DDL
//! (SCH-1, SCH-2, SCH-5) plus the name lists autocomplete uses (QRY-2).
//! Each driver has its own catalog queries; this module picks one.

mod mysql;
mod postgres;
mod sqlite;

pub use mysql::mysql_default_expr;

use std::collections::BTreeMap;

use serde::Serialize;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};

/// Autocomplete loads at most this many tables of one schema.
const COMPLETION_TABLE_LIMIT: usize = 5000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfo {
    pub name: String,
    /// "table" | "view"
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    /// Display type, and on Postgres the exact `format_type` used for casts.
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    /// 1-based position in the primary key, if part of it.
    pub pk_position: Option<i32>,
    /// Identity / serial (Postgres), AUTO_INCREMENT (MySQL), rowid alias (SQLite).
    pub auto_increment: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    /// NO ACTION | RESTRICT | CASCADE | SET NULL | SET DEFAULT
    pub on_update: String,
    pub on_delete: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub kind: &'static str,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub ddl: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionSchema {
    pub schema: String,
    pub schemas: Vec<String>,
    /// table → columns, in ordinal order.
    pub tables: BTreeMap<String, Vec<String>>,
}

fn kind_label(is_view: bool) -> &'static str {
    if is_view {
        "view"
    } else {
        "table"
    }
}

pub async fn list_databases(pool: &DbPool, show_system: bool) -> AppResult<Vec<String>> {
    match pool {
        DbPool::Postgres(p) => postgres::databases(p).await,
        DbPool::Mysql(p) => mysql::databases(p, show_system).await,
        DbPool::Sqlite(p) => sqlite::databases(p).await,
    }
}

pub async fn list_schemas(pool: &DbPool, show_system: bool) -> AppResult<Vec<String>> {
    match pool {
        DbPool::Postgres(p) => postgres::schemas(p, show_system).await,
        // A MySQL schema is a database.
        DbPool::Mysql(p) => mysql::databases(p, show_system).await,
        DbPool::Sqlite(p) => sqlite::databases(p).await,
    }
}

pub async fn list_objects(pool: &DbPool, schema: &str) -> AppResult<Vec<ObjectInfo>> {
    match pool {
        DbPool::Postgres(p) => postgres::objects(p, schema).await,
        DbPool::Mysql(p) => mysql::objects(p, schema).await,
        DbPool::Sqlite(p) => sqlite::objects(p, schema).await,
    }
}

pub async fn list_columns(pool: &DbPool, schema: &str, table: &str) -> AppResult<Vec<ColumnInfo>> {
    let cols = match pool {
        DbPool::Postgres(p) => postgres::columns(p, schema, table).await?,
        DbPool::Mysql(p) => mysql::columns(p, schema, table).await?,
        DbPool::Sqlite(p) => sqlite::columns(p, schema, table).await?,
    };
    if cols.is_empty() {
        return Err(AppError::not_found(format!(
            "Table \"{schema}.{table}\" was not found or has no columns."
        )));
    }
    Ok(cols)
}

/// Primary key columns in key order.
pub fn primary_key(columns: &[ColumnInfo]) -> Vec<String> {
    let mut pk: Vec<(i32, String)> = columns
        .iter()
        .filter_map(|c| c.pk_position.map(|p| (p, c.name.clone())))
        .collect();
    pk.sort();
    pk.into_iter().map(|(_, n)| n).collect()
}

pub async fn table_structure(
    pool: &DbPool,
    schema: &str,
    table: &str,
) -> AppResult<TableStructure> {
    let columns = list_columns(pool, schema, table).await?;
    match pool {
        DbPool::Postgres(p) => postgres::structure(p, schema, table, columns).await,
        DbPool::Mysql(p) => mysql::structure(p, schema, table, columns).await,
        DbPool::Sqlite(p) => sqlite::structure(p, schema, table, columns).await,
    }
}

/// ` ON UPDATE …` / ` ON DELETE …` for a foreign key definition (NO ACTION is left out).
fn fk_actions_suffix(on_update: &str, on_delete: &str) -> String {
    let mut s = String::new();
    if !on_update.is_empty() && on_update != "NO ACTION" {
        s.push_str(&format!(" ON UPDATE {on_update}"));
    }
    if !on_delete.is_empty() && on_delete != "NO ACTION" {
        s.push_str(&format!(" ON DELETE {on_delete}"));
    }
    s
}

/// `FOREIGN KEY (a, b) REFERENCES t (x, y) …` for display.
fn fk_definition(f: &ForeignKeyInfo) -> String {
    format!(
        "FOREIGN KEY ({}) REFERENCES {} ({}){}",
        f.columns.join(", "),
        f.ref_table,
        f.ref_columns.join(", "),
        fk_actions_suffix(&f.on_update, &f.on_delete)
    )
}

/// Tables and columns of one schema for autocomplete (capped at 5,000 tables).
pub async fn completion(
    pool: &DbPool,
    schema: &str,
    show_system: bool,
) -> AppResult<CompletionSchema> {
    let schemas = list_schemas(pool, show_system).await.unwrap_or_default();
    let mut tables = match pool {
        DbPool::Postgres(p) => postgres::completion_tables(p, schema).await?,
        DbPool::Mysql(p) => mysql::completion_tables(p, schema).await?,
        DbPool::Sqlite(p) => sqlite::completion_tables(p, schema, COMPLETION_TABLE_LIMIT).await?,
    };
    while tables.len() > COMPLETION_TABLE_LIMIT {
        let last = tables.keys().next_back().cloned().unwrap();
        tables.remove(&last);
    }
    Ok(CompletionSchema {
        schema: schema.to_string(),
        schemas,
        tables,
    })
}

/// Groups (table, column) rows into table → columns, keeping the row order.
fn group_columns(rows: Vec<(String, String)>) -> BTreeMap<String, Vec<String>> {
    let mut tables: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (t, c) in rows {
        tables.entry(t).or_default().push(c);
    }
    tables
}
