//! Snowflake catalog through SHOW / DESCRIBE (no warehouse needed) and GET_DDL.

use std::collections::BTreeMap;

use serde_json::Value;

use super::client::{new_request_id, Conn, Output};
use super::sql::{literal, quote};
use crate::db::meta::{ColumnInfo, ForeignKeyInfo, IndexInfo, ObjectInfo};
use crate::error::AppResult;

/// SHOW output is small; this only bounds a runaway answer.
const SHOW_LIMIT: usize = 100_000;

async fn show(conn: &Conn, sql: &str) -> AppResult<Output> {
    conn.query(&new_request_id(), sql, &[], SHOW_LIMIT, None)
        .await
}

/// Index of a result column, ignoring case (SHOW names its columns in lower case).
fn idx(out: &Output, name: &str) -> Option<usize> {
    out.columns
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(name))
}

fn text(row: &[Value], i: Option<usize>) -> String {
    match i.and_then(|i| row.get(i)) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// One text column of every row.
fn names(out: &Output, column: &str) -> Vec<String> {
    let i = idx(out, column);
    out.rows.iter().map(|r| text(r, i)).collect()
}

fn qualified(database: &str, schema: &str, table: &str) -> String {
    [database, schema, table]
        .iter()
        .filter(|p| !p.is_empty())
        .map(|p| quote(p))
        .collect::<Vec<_>>()
        .join(".")
}

pub async fn databases(conn: &Conn) -> AppResult<Vec<String>> {
    Ok(names(&show(conn, "SHOW DATABASES").await?, "name"))
}

pub async fn schemas(conn: &Conn, database: &str, show_system: bool) -> AppResult<Vec<String>> {
    if database.is_empty() {
        return Ok(Vec::new());
    }
    let out = show(
        conn,
        &format!("SHOW SCHEMAS IN DATABASE {}", quote(database)),
    )
    .await?;
    Ok(names(&out, "name")
        .into_iter()
        .filter(|s| show_system || s != "INFORMATION_SCHEMA")
        .collect())
}

pub async fn objects(conn: &Conn, database: &str, schema: &str) -> AppResult<Vec<ObjectInfo>> {
    if database.is_empty() {
        return Ok(Vec::new());
    }
    let place = qualified(database, schema, "");
    let mut out = Vec::new();
    for (sql, kind) in [("SHOW TABLES", "table"), ("SHOW VIEWS", "view")] {
        let res = show(conn, &format!("{sql} IN SCHEMA {place}")).await?;
        out.extend(
            names(&res, "name")
                .into_iter()
                .map(|name| ObjectInfo { name, kind }),
        );
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// DESCRIBE TABLE (or VIEW) plus SHOW PRIMARY KEYS for key positions.
pub async fn columns(
    conn: &Conn,
    database: &str,
    schema: &str,
    table: &str,
) -> AppResult<Vec<ColumnInfo>> {
    let name = qualified(database, schema, table);
    let desc = match show(conn, &format!("DESCRIBE TABLE {name}")).await {
        Ok(d) => d,
        Err(e) => show(conn, &format!("DESCRIBE VIEW {name}"))
            .await
            .map_err(|_| e)?,
    };
    let keys: BTreeMap<String, i32> =
        match show(conn, &format!("SHOW PRIMARY KEYS IN TABLE {name}")).await {
            Ok(pk) => {
                let (c, s) = (idx(&pk, "column_name"), idx(&pk, "key_sequence"));
                pk.rows
                    .iter()
                    .map(|r| (text(r, c), text(r, s).parse().unwrap_or(1)))
                    .collect()
            }
            Err(_) => BTreeMap::new(),
        };
    let i = |n| idx(&desc, n);
    let (n, ty, null, def, cmt) = (i("name"), i("type"), i("null?"), i("default"), i("comment"));
    Ok(desc
        .rows
        .iter()
        .map(|r| {
            let name = text(r, n);
            let default = Some(text(r, def)).filter(|d| !d.is_empty());
            let auto = default.as_deref().is_some_and(|d| {
                let d = d.to_ascii_uppercase();
                d.starts_with("IDENTITY") || d.starts_with("AUTOINCREMENT")
            });
            ColumnInfo {
                pk_position: keys.get(&name).copied(),
                data_type: text(r, ty),
                nullable: text(r, null) != "N",
                auto_increment: auto,
                comment: Some(text(r, cmt)).filter(|c| !c.is_empty()),
                default,
                name,
                ..Default::default()
            }
        })
        .collect())
}

/// Table → columns of a schema in one SHOW.
pub async fn completion(
    conn: &Conn,
    database: &str,
    schema: &str,
) -> AppResult<BTreeMap<String, Vec<String>>> {
    let out = show(
        conn,
        &format!("SHOW COLUMNS IN SCHEMA {}", qualified(database, schema, "")),
    )
    .await?;
    let (t, c) = (idx(&out, "table_name"), idx(&out, "column_name"));
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in &out.rows {
        map.entry(text(r, t)).or_default().push(text(r, c));
    }
    Ok(map)
}

pub fn primary_index(columns: &[ColumnInfo]) -> Option<IndexInfo> {
    let keys = crate::db::meta::primary_key(columns);
    (!keys.is_empty()).then(|| IndexInfo {
        name: "PRIMARY KEY".into(),
        definition: format!(
            "PRIMARY KEY ({})",
            keys.iter().map(|k| quote(k)).collect::<Vec<_>>().join(", ")
        ),
        columns: keys,
        unique: true,
        primary: true,
        ..Default::default()
    })
}

/// Foreign keys (informational in Snowflake) from SHOW IMPORTED KEYS.
pub async fn foreign_keys(
    conn: &Conn,
    database: &str,
    schema: &str,
    table: &str,
) -> Vec<ForeignKeyInfo> {
    let sql = format!(
        "SHOW IMPORTED KEYS IN TABLE {}",
        qualified(database, schema, table)
    );
    let Ok(out) = show(conn, &sql).await else {
        return Vec::new();
    };
    let i = |n| idx(&out, n);
    let (name, col, rt, rc) = (
        i("fk_name"),
        i("fk_column_name"),
        i("pk_table_name"),
        i("pk_column_name"),
    );
    let (upd, del) = (i("update_rule"), i("delete_rule"));
    let mut fks: Vec<ForeignKeyInfo> = Vec::new();
    for r in &out.rows {
        let n = text(r, name);
        match fks.iter_mut().find(|f| f.name == n) {
            Some(f) => {
                f.columns.push(text(r, col));
                f.ref_columns.push(text(r, rc));
            }
            None => fks.push(ForeignKeyInfo {
                name: n,
                columns: vec![text(r, col)],
                ref_table: text(r, rt),
                ref_columns: vec![text(r, rc)],
                on_update: text(r, upd),
                on_delete: text(r, del),
                definition: String::new(),
            }),
        }
    }
    for f in &mut fks {
        let list = |v: &[String]| v.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ");
        f.definition = format!(
            "FOREIGN KEY ({}) REFERENCES {} ({})",
            list(&f.columns),
            quote(&f.ref_table),
            list(&f.ref_columns)
        );
    }
    fks
}

/// GET_DDL text and whether the object is a view ("" when Snowflake won't say).
pub async fn ddl(conn: &Conn, database: &str, schema: &str, table: &str) -> (String, bool) {
    let name = literal(&qualified(database, schema, table));
    for kind in ["TABLE", "VIEW"] {
        let sql = format!("SELECT GET_DDL('{kind}', {name})");
        if let Ok(out) = show(conn, &sql).await {
            if let Some(row) = out.rows.first() {
                let body = text(row, Some(0));
                let head: String = body.chars().take(80).collect::<String>().to_uppercase();
                let view = kind == "VIEW" || head.contains(" VIEW ");
                return (body, view);
            }
        }
    }
    (String::new(), false)
}
