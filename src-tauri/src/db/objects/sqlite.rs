//! SQLite triggers from `sqlite_master` (views are listed with the tables). The stored
//! CREATE TRIGGER text is the definition.

use sqlx::SqlitePool;

use crate::db::sql::{qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Driver;

use super::{
    missing, terminated, unsupported, ObjectDefinition, ObjectKind, ObjectRef, SchemaObject,
};

const D: Driver = Driver::Sqlite;

pub(super) async fn list(p: &SqlitePool, schema: &str) -> AppResult<Vec<SchemaObject>> {
    let sql = format!(
        "SELECT name, tbl_name, sql FROM {}.sqlite_master WHERE type = 'trigger' ORDER BY name",
        quote_ident(D, schema)
    );
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(&sql).fetch_all(p).await?;
    Ok(rows.into_iter().map(trigger_object).collect())
}

pub(super) fn trigger_object((name, table, sql): (String, String, Option<String>)) -> SchemaObject {
    let mut o = SchemaObject::new(ObjectKind::Trigger, name);
    o.detail = sql.as_deref().map(trigger_detail);
    o.table = Some(table);
    o
}

/// "BEFORE INSERT" from the CREATE TRIGGER text (AFTER is SQLite's default timing).
pub(super) fn trigger_detail(sql: &str) -> String {
    let words: Vec<String> = sql
        .split(|c: char| c.is_whitespace() || c == '(')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_uppercase())
        .take_while(|w| w != "ON")
        .collect();
    let has = |w: &str| words.iter().any(|x| x == w);
    let timing = if has("INSTEAD") {
        "INSTEAD OF"
    } else if has("BEFORE") {
        "BEFORE"
    } else {
        "AFTER"
    };
    let event = ["INSERT", "UPDATE", "DELETE"]
        .into_iter()
        .find(|e| has(e))
        .unwrap_or("");
    format!("{timing} {event} · row").replace("  ", " ")
}

pub(super) async fn definition(
    p: &SqlitePool,
    schema: &str,
    object: &ObjectRef,
) -> AppResult<ObjectDefinition> {
    if object.kind != ObjectKind::Trigger {
        return Err(unsupported(object.kind));
    }
    let sql = format!(
        "SELECT sql FROM {}.sqlite_master WHERE type = 'trigger' AND name = ?1",
        quote_ident(D, schema)
    );
    let defs: Vec<Option<String>> = sqlx::query_scalar(&sql)
        .bind(&object.name)
        .fetch_all(p)
        .await?;
    let def = defs.into_iter().flatten().next();
    let ddl = terminated(&def.ok_or_else(|| missing(object.kind, schema, &object.name))?);
    let drop = format!("DROP TRIGGER {};", qualified(D, schema, &object.name));
    Ok(ObjectDefinition {
        edit_script: format!("{drop}\n{ddl}"),
        ddl,
    })
}
