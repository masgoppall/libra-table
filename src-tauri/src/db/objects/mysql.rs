//! MySQL / MariaDB routines, triggers and events from information_schema, and their
//! definitions from SHOW CREATE. Text columns are cast to CHAR so they decode as
//! strings whatever the server's collation.
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use std::collections::HashMap;

use sqlx::mysql::MySqlRow;
use sqlx::{MySqlPool, Row};

use crate::db::sql::qualified;
use crate::error::{AppError, AppResult};
use crate::models::Driver;

use super::{
    missing, terminated, unsupported, ObjectDefinition, ObjectKind, ObjectRef, SchemaObject,
};

const D: Driver = Driver::Mysql;

const ROUTINES: &str = "SELECT CAST(ROUTINE_NAME AS CHAR), CAST(ROUTINE_TYPE AS CHAR), \
        CAST(DTD_IDENTIFIER AS CHAR) \
    FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = ? ORDER BY ROUTINE_NAME";

const PARAMETERS: &str = "SELECT CAST(SPECIFIC_NAME AS CHAR), CAST(ROUTINE_TYPE AS CHAR), \
        CAST(PARAMETER_MODE AS CHAR), CAST(PARAMETER_NAME AS CHAR), \
        CAST(DTD_IDENTIFIER AS CHAR) \
    FROM information_schema.PARAMETERS WHERE SPECIFIC_SCHEMA = ? AND ORDINAL_POSITION > 0 \
    ORDER BY SPECIFIC_NAME, ORDINAL_POSITION";

const TRIGGERS: &str = "SELECT CAST(TRIGGER_NAME AS CHAR), CAST(EVENT_OBJECT_TABLE AS CHAR), \
        CAST(ACTION_TIMING AS CHAR), CAST(EVENT_MANIPULATION AS CHAR) \
    FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA = ? \
    ORDER BY EVENT_OBJECT_TABLE, TRIGGER_NAME";

const EVENTS: &str = "SELECT CAST(EVENT_NAME AS CHAR), CAST(EVENT_TYPE AS CHAR), \
        CAST(INTERVAL_VALUE AS CHAR), CAST(INTERVAL_FIELD AS CHAR), CAST(STATUS AS CHAR) \
    FROM information_schema.EVENTS WHERE EVENT_SCHEMA = ? ORDER BY EVENT_NAME";

/// (specific name, routine type, mode, name, type)
type ParamRow = (String, String, Option<String>, Option<String>, String);

pub(super) async fn list(p: &MySqlPool, schema: &str) -> AppResult<Vec<SchemaObject>> {
    let (routines, params, triggers, events) = futures::try_join!(
        sqlx::query_as::<_, (String, String, Option<String>)>(ROUTINES)
            .bind(schema)
            .fetch_all(p),
        sqlx::query_as::<_, ParamRow>(PARAMETERS)
            .bind(schema)
            .fetch_all(p),
        sqlx::query_as::<_, (String, String, String, String)>(TRIGGERS)
            .bind(schema)
            .fetch_all(p),
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String)>(EVENTS)
            .bind(schema)
            .fetch_all(p),
    )?;
    let signatures = signatures(params);
    let mut out: Vec<SchemaObject> = routines
        .into_iter()
        .map(|r| routine_object(r, &signatures))
        .collect();
    out.extend(triggers.into_iter().map(trigger_object));
    out.extend(events.into_iter().map(event_object));
    Ok(out)
}

/// (name, routine type) → "IN a INT, OUT b TEXT". Functions have no parameter modes.
pub(super) fn signatures(rows: Vec<ParamRow>) -> HashMap<(String, String), String> {
    let mut out: HashMap<(String, String), Vec<String>> = HashMap::new();
    for (routine, kind, mode, name, ty) in rows {
        let mut p = String::new();
        if let Some(m) = mode.filter(|_| kind == "PROCEDURE") {
            p.push_str(&format!("{m} "));
        }
        if let Some(n) = name {
            p.push_str(&format!("{n} "));
        }
        p.push_str(&ty);
        out.entry((routine, kind)).or_default().push(p);
    }
    out.into_iter().map(|(k, v)| (k, v.join(", "))).collect()
}

pub(super) fn routine_object(
    (name, kind, returns): (String, String, Option<String>),
    signatures: &HashMap<(String, String), String>,
) -> SchemaObject {
    let object_kind = if kind == "PROCEDURE" {
        ObjectKind::Procedure
    } else {
        ObjectKind::Function
    };
    let args = signatures.get(&(name.clone(), kind)).cloned();
    let mut o = SchemaObject::new(object_kind, name);
    o.arguments = Some(args.unwrap_or_default());
    o.detail = returns.map(|r| format!("returns {r}"));
    o
}

pub(super) fn trigger_object(
    (name, table, timing, event): (String, String, String, String),
) -> SchemaObject {
    let mut o = SchemaObject::new(ObjectKind::Trigger, name);
    o.detail = Some(format!("{timing} {event} · row"));
    o.table = Some(table);
    o
}

pub(super) fn event_object(
    (name, kind, value, field, status): (String, String, Option<String>, Option<String>, String),
) -> SchemaObject {
    let schedule = match (kind.as_str(), value, field) {
        ("RECURRING", Some(v), Some(f)) => format!("every {v} {}", f.to_ascii_lowercase()),
        _ => "one time".to_string(),
    };
    let mut o = SchemaObject::new(ObjectKind::Event, name);
    o.detail = Some(format!("{schedule} · {}", status.to_ascii_lowercase()));
    o
}

/// The SHOW CREATE keyword and the column holding the statement.
fn show_create(kind: ObjectKind) -> Option<(&'static str, &'static str)> {
    match kind {
        ObjectKind::Function => Some(("FUNCTION", "Create Function")),
        ObjectKind::Procedure => Some(("PROCEDURE", "Create Procedure")),
        ObjectKind::Trigger => Some(("TRIGGER", "SQL Original Statement")),
        ObjectKind::Event => Some(("EVENT", "Create Event")),
        _ => None,
    }
}

pub(super) async fn definition(
    p: &MySqlPool,
    schema: &str,
    object: &ObjectRef,
) -> AppResult<ObjectDefinition> {
    let (what, column) = show_create(object.kind).ok_or_else(|| unsupported(object.kind))?;
    let target = qualified(D, schema, &object.name);
    let sql = format!("SHOW CREATE {what} {target}");
    // The text protocol: not every server prepares SHOW CREATE for routines.
    let rows = sqlx::raw_sql(&sql).fetch_all(p).await?;
    let row = rows.first();
    let text = row.and_then(|r| text_column(r, column));
    let ddl = match (row.is_some(), text) {
        (_, Some(t)) if !t.trim().is_empty() => terminated(&t),
        // The definition column is NULL without the privileges to see it.
        (true, _) => {
            return Err(AppError::query(format!(
                "MySQL didn't show the {} definition: it needs more privileges.",
                object.kind.label()
            )))
        }
        (false, _) => return Err(missing(object.kind, schema, &object.name)),
    };
    // MySQL has no CREATE OR REPLACE for routines, triggers and events.
    Ok(ObjectDefinition {
        edit_script: format!("DROP {what} IF EXISTS {target};\n{ddl}"),
        ddl,
    })
}

/// A text column of a SHOW result; MySQL may send it as bytes.
fn text_column(row: &MySqlRow, column: &str) -> Option<String> {
    row.try_get_unchecked::<Option<String>, _>(column)
        .ok()
        .flatten()
        .or_else(|| {
            row.try_get_unchecked::<Option<Vec<u8>>, _>(column)
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        })
}
