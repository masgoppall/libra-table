//! Schema objects besides tables and views: functions, procedures, sequences,
//! materialized views, triggers, types and events. Each driver lists what it has with
//! one round of catalog queries per schema, and writes an object's definition (the DDL)
//! plus a script to change it from a query tab.

mod mysql;
mod postgres;
mod sqlite;

use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Function,
    Procedure,
    Sequence,
    MaterializedView,
    Trigger,
    Type,
    Event,
}

impl ObjectKind {
    /// "materialized view", for messages.
    pub fn label(self) -> &'static str {
        match self {
            ObjectKind::Function => "function",
            ObjectKind::Procedure => "procedure",
            ObjectKind::Sequence => "sequence",
            ObjectKind::MaterializedView => "materialized view",
            ObjectKind::Trigger => "trigger",
            ObjectKind::Type => "type",
            ObjectKind::Event => "event",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaObject {
    pub kind: ObjectKind,
    pub name: String,
    /// Postgres functions and procedures: the identity arguments (`a integer, b text`),
    /// which tell overloads apart. MySQL routines: the parameters, for display.
    pub arguments: Option<String>,
    /// Triggers: the table they belong to.
    pub table: Option<String>,
    /// A short description: result type, trigger timing and events, type category…
    pub detail: Option<String>,
    /// Postgres enum types: their labels in order.
    pub labels: Vec<String>,
}

impl SchemaObject {
    fn new(kind: ObjectKind, name: String) -> Self {
        Self {
            kind,
            name,
            arguments: None,
            table: None,
            detail: None,
            labels: Vec::new(),
        }
    }
}

/// Which object a definition or a change is about, as the UI sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRef {
    pub kind: ObjectKind,
    pub name: String,
    #[serde(default)]
    pub arguments: Option<String>,
    #[serde(default)]
    pub table: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObjectDefinition {
    /// The object as the database describes it.
    pub ddl: String,
    /// What "Edit in query tab" opens: CREATE OR REPLACE where the driver has it,
    /// otherwise DROP followed by CREATE (ALTER SEQUENCE for sequences).
    pub edit_script: String,
}

/// Numbers go to the UI as text: a bigint MAXVALUE doesn't fit a JavaScript number.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SequenceInfo {
    pub data_type: String,
    #[serde(serialize_with = "as_text")]
    pub start_value: i64,
    #[serde(serialize_with = "as_text")]
    pub min_value: i64,
    #[serde(serialize_with = "as_text")]
    pub max_value: i64,
    #[serde(serialize_with = "as_text")]
    pub increment: i64,
    pub cycle: bool,
    #[serde(serialize_with = "as_text")]
    pub cache: i64,
    /// None until the sequence is first used.
    #[serde(serialize_with = "opt_as_text")]
    pub last_value: Option<i64>,
}

fn as_text<S: serde::Serializer>(v: &i64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&v.to_string())
}

fn opt_as_text<S: serde::Serializer>(v: &Option<i64>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(n) => s.serialize_str(&n.to_string()),
        None => s.serialize_none(),
    }
}

/// Every object of `schema` that has its own sidebar section.
pub async fn list(pool: &DbPool, schema: &str) -> AppResult<Vec<SchemaObject>> {
    match pool {
        DbPool::Postgres(p) => postgres::list(p, schema).await,
        DbPool::Mysql(p) => mysql::list(p, schema).await,
        DbPool::Sqlite(p) => sqlite::list(p, schema).await,
        DbPool::Engine(_) => Ok(Vec::new()),
    }
}

pub async fn definition(
    pool: &DbPool,
    schema: &str,
    object: &ObjectRef,
) -> AppResult<ObjectDefinition> {
    match pool {
        DbPool::Postgres(p) => postgres::definition(p, schema, object).await,
        DbPool::Mysql(p) => mysql::definition(p, schema, object).await,
        DbPool::Sqlite(p) => sqlite::definition(p, schema, object).await,
        DbPool::Engine(_) => Err(crate::db::engine::sqlx_only()),
    }
}

pub async fn sequence(pool: &DbPool, schema: &str, name: &str) -> AppResult<SequenceInfo> {
    match pool {
        DbPool::Postgres(p) => postgres::sequence(p, schema, name).await,
        _ => Err(unsupported(ObjectKind::Sequence)),
    }
}

fn unsupported(kind: ObjectKind) -> AppError {
    AppError::validation(format!("This database has no {}s.", kind.label()))
}

fn missing(kind: ObjectKind, schema: &str, name: &str) -> AppError {
    AppError::not_found(format!(
        "The {} \"{schema}.{name}\" was not found.",
        kind.label()
    ))
}

/// Ends a statement with exactly one `;`.
fn terminated(sql: &str) -> String {
    let t = sql.trim_end();
    if t.ends_with(';') {
        t.to_string()
    } else {
        format!("{t};")
    }
}

#[cfg(test)]
mod tests;
