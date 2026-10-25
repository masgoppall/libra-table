//! SQLite metadata from `sqlite_master` and the table-valued pragmas. Attached
//! databases are the schemas.
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use std::collections::BTreeMap;

use sqlx::SqlitePool;

use crate::db::sql::quote_ident;
use crate::error::AppResult;
use crate::models::Dialect;

use super::{
    fk_definition, kind_label, primary_key, ColumnInfo, ForeignKeyInfo, IndexInfo, ObjectInfo,
    TableStructure,
};

const TABLE_INFO: &str =
    "SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?1, ?2)";

const INDEX_LIST: &str =
    "SELECT seq, name, \"unique\", origin, partial FROM pragma_index_list(?1, ?2)";

const INDEX_INFO: &str = "SELECT seqno, cid, name FROM pragma_index_info(?1, ?2) ORDER BY seqno";

const FOREIGN_KEY_LIST: &str = "SELECT id, seq, \"table\", \"from\", \"to\", on_update, \
    on_delete FROM pragma_foreign_key_list(?1, ?2) ORDER BY id, seq";

/// `"schema".sqlite_master`
fn master(schema: &str) -> String {
    format!("{}.sqlite_master", quote_ident(Dialect::Sqlite, schema))
}

pub(super) async fn databases(p: &SqlitePool) -> AppResult<Vec<String>> {
    let rows: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("PRAGMA database_list").fetch_all(p).await?;
    Ok(rows
        .into_iter()
        .map(|r| r.1)
        .filter(|n| n != "temp")
        .collect())
}

pub(super) async fn objects(p: &SqlitePool, schema: &str) -> AppResult<Vec<ObjectInfo>> {
    let sql = format!(
        "SELECT name, type FROM {} WHERE type IN ('table','view') \
         AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY name",
        master(schema)
    );
    let rows: Vec<(String, String)> = sqlx::query_as(&sql).fetch_all(p).await?;
    Ok(rows
        .into_iter()
        .map(|(name, t)| ObjectInfo {
            name,
            kind: kind_label(t == "view"),
        })
        .collect())
}

pub(super) async fn columns(
    p: &SqlitePool,
    schema: &str,
    table: &str,
) -> AppResult<Vec<ColumnInfo>> {
    let rows: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(TABLE_INFO)
        .bind(table)
        .bind(schema)
        .fetch_all(p)
        .await?;
    let single_pk = rows.iter().filter(|r| r.5 > 0).count() == 1;
    Ok(rows
        .into_iter()
        .map(|(_, name, data_type, notnull, default, pk)| ColumnInfo {
            // INTEGER PRIMARY KEY is the rowid alias: it numbers new rows itself.
            auto_increment: pk > 0 && single_pk && data_type.eq_ignore_ascii_case("integer"),
            name,
            data_type,
            nullable: notnull == 0,
            default,
            pk_position: if pk > 0 { Some(pk as i32) } else { None },
            comment: None,
        })
        .collect())
}

pub(super) async fn structure(
    p: &SqlitePool,
    schema: &str,
    table: &str,
    columns: Vec<ColumnInfo>,
) -> AppResult<TableStructure> {
    let (ttype, sql): (String, Option<String>) = sqlx::query_as(&format!(
        "SELECT type, sql FROM {} WHERE name = ?1 AND type IN ('table','view')",
        master(schema)
    ))
    .bind(table)
    .fetch_one(p)
    .await?;
    let indexes = indexes(p, schema, table, &columns).await?;
    let foreign_keys = foreign_keys(p, schema, table).await?;
    let ddl = ddl(sql.unwrap_or_default(), &indexes);
    Ok(TableStructure {
        kind: kind_label(ttype == "view"),
        columns,
        indexes,
        foreign_keys,
        ddl,
    })
}

async fn indexes(
    p: &SqlitePool,
    schema: &str,
    table: &str,
    columns: &[ColumnInfo],
) -> AppResult<Vec<IndexInfo>> {
    let list: Vec<(i64, String, i64, String, i64)> = sqlx::query_as(INDEX_LIST)
        .bind(table)
        .bind(schema)
        .fetch_all(p)
        .await?;
    let mut indexes = Vec::new();
    for (_, name, unique, origin, _) in list {
        indexes.push(index(p, schema, name, unique != 0, origin).await?);
    }
    // INTEGER PRIMARY KEY is the rowid alias and has no entry in index_list.
    let pk = primary_key(columns);
    if !pk.is_empty() && !indexes.iter().any(|i| i.primary) {
        let rowid = IndexInfo {
            name: "PRIMARY".into(),
            definition: format!("PRIMARY KEY ({}) — rowid alias", pk.join(", ")),
            columns: pk,
            unique: true,
            primary: true,
        };
        indexes.insert(0, rowid);
    }
    Ok(indexes)
}

/// One index; automatic indexes (UNIQUE / PRIMARY KEY constraints) have no SQL.
async fn index(
    p: &SqlitePool,
    schema: &str,
    name: String,
    unique: bool,
    origin: String,
) -> AppResult<IndexInfo> {
    let cols: Vec<(i64, i64, Option<String>)> = sqlx::query_as(INDEX_INFO)
        .bind(&name)
        .bind(schema)
        .fetch_all(p)
        .await?;
    let definition: Option<String> = sqlx::query_scalar(&format!(
        "SELECT sql FROM {} WHERE type = 'index' AND name = ?1",
        master(schema)
    ))
    .bind(&name)
    .fetch_optional(p)
    .await?
    .flatten();
    let columns: Vec<String> = cols
        .into_iter()
        .map(|c| c.2.unwrap_or_else(|| "<expr>".into()))
        .collect();
    Ok(IndexInfo {
        primary: origin == "pk",
        unique,
        definition: definition
            .unwrap_or_else(|| format!("({}) — automatic index ({origin})", columns.join(", "))),
        columns,
        name,
    })
}

/// SQLite foreign keys have no name; they are called `fk_<id>`.
async fn foreign_keys(p: &SqlitePool, schema: &str, table: &str) -> AppResult<Vec<ForeignKeyInfo>> {
    let rows: Vec<(i64, i64, String, String, Option<String>, String, String)> =
        sqlx::query_as(FOREIGN_KEY_LIST)
            .bind(table)
            .bind(schema)
            .fetch_all(p)
            .await?;
    let mut fks: Vec<(i64, ForeignKeyInfo)> = Vec::new();
    for (id, _, rtable, from, to, on_update, on_delete) in rows {
        let to = to.unwrap_or_default();
        match fks.iter_mut().find(|(fid, _)| *fid == id) {
            Some((_, f)) => {
                f.columns.push(from);
                f.ref_columns.push(to);
            }
            None => fks.push((
                id,
                ForeignKeyInfo {
                    name: format!("fk_{id}"),
                    columns: vec![from],
                    ref_table: rtable,
                    ref_columns: vec![to],
                    on_update,
                    on_delete,
                    definition: String::new(),
                },
            )),
        }
    }
    Ok(fks
        .into_iter()
        .map(|(_, mut f)| {
            f.definition = fk_definition(&f);
            f
        })
        .collect())
}

/// The table's CREATE TABLE followed by its CREATE INDEX statements.
fn ddl(table_sql: String, indexes: &[IndexInfo]) -> String {
    let mut ddl = table_sql;
    for i in indexes {
        if i.definition.starts_with("CREATE") {
            ddl.push_str(";\n");
            ddl.push_str(&i.definition);
        }
    }
    if !ddl.is_empty() {
        ddl.push(';');
    }
    ddl
}

/// Columns of each table, one pragma per table, at most `limit` tables.
pub(super) async fn completion_tables(
    p: &SqlitePool,
    schema: &str,
    limit: usize,
) -> AppResult<BTreeMap<String, Vec<String>>> {
    let mut tables = BTreeMap::new();
    for obj in objects(p, schema).await?.into_iter().take(limit) {
        let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?1, ?2)")
            .bind(&obj.name)
            .bind(schema)
            .fetch_all(p)
            .await?;
        tables.insert(obj.name, cols);
    }
    Ok(tables)
}
