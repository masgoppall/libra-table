//! MySQL catalog queries (information_schema). Text columns are cast to CHAR so they
//! decode as strings whatever the server's collation.
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use std::collections::BTreeMap;

use sqlx::{MySqlPool, Row};

use crate::db::sql::{mysql_literal, qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Dialect;

use super::{
    fk_definition, group_columns, kind_label, ColumnInfo, ForeignKeyInfo, IndexInfo, ObjectInfo,
    TableStructure,
};

const SYSTEM_DATABASES: &[&str] = &["information_schema", "mysql", "performance_schema", "sys"];
const D: Dialect = Dialect::Mysql;

const DATABASES: &str =
    "SELECT CAST(SCHEMA_NAME AS CHAR) FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME";

const OBJECTS: &str = "SELECT CAST(TABLE_NAME AS CHAR), CAST(TABLE_TYPE AS CHAR) \
    FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME";

const COLUMNS: &str = "SELECT CAST(COLUMN_NAME AS CHAR), CAST(COLUMN_TYPE AS CHAR), \
        CAST(IS_NULLABLE AS CHAR), CAST(COLUMN_DEFAULT AS CHAR), CAST(EXTRA AS CHAR), \
        CAST(COLUMN_COMMENT AS CHAR), CAST(CHARACTER_SET_NAME AS CHAR), \
        CAST(COLLATION_NAME AS CHAR) \
    FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
    ORDER BY ORDINAL_POSITION";

const PRIMARY_KEY: &str = "SELECT CAST(COLUMN_NAME AS CHAR), \
        CAST(ORDINAL_POSITION AS SIGNED) \
    FROM information_schema.KEY_COLUMN_USAGE \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY'";

const TABLE_TYPE: &str = "SELECT CAST(TABLE_TYPE AS CHAR) FROM information_schema.TABLES \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?";

const INDEX_COLUMNS: &str = "SELECT CAST(INDEX_NAME AS CHAR), CAST(NON_UNIQUE AS SIGNED), \
        CAST(COLUMN_NAME AS CHAR), CAST(SEQ_IN_INDEX AS SIGNED), CAST(INDEX_TYPE AS CHAR) \
    FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
    ORDER BY INDEX_NAME, SEQ_IN_INDEX";

const FK_COLUMNS: &str = "SELECT CAST(CONSTRAINT_NAME AS CHAR), CAST(COLUMN_NAME AS CHAR), \
        CAST(REFERENCED_TABLE_SCHEMA AS CHAR), CAST(REFERENCED_TABLE_NAME AS CHAR), \
        CAST(REFERENCED_COLUMN_NAME AS CHAR) \
    FROM information_schema.KEY_COLUMN_USAGE \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND REFERENCED_TABLE_NAME IS NOT NULL \
    ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION";

const FK_RULES: &str = "SELECT CAST(CONSTRAINT_NAME AS CHAR), CAST(UPDATE_RULE AS CHAR), \
        CAST(DELETE_RULE AS CHAR) \
    FROM information_schema.REFERENTIAL_CONSTRAINTS \
    WHERE CONSTRAINT_SCHEMA = ? AND TABLE_NAME = ?";

const COMPLETION: &str = "SELECT CAST(TABLE_NAME AS CHAR), CAST(COLUMN_NAME AS CHAR) \
    FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? \
    ORDER BY TABLE_NAME, ORDINAL_POSITION";

pub(super) async fn databases(p: &MySqlPool, show_system: bool) -> AppResult<Vec<String>> {
    let all: Vec<String> = sqlx::query_scalar(DATABASES).fetch_all(p).await?;
    Ok(all
        .into_iter()
        .filter(|d| show_system || !SYSTEM_DATABASES.contains(&d.to_ascii_lowercase().as_str()))
        .collect())
}

pub(super) async fn objects(p: &MySqlPool, schema: &str) -> AppResult<Vec<ObjectInfo>> {
    let rows: Vec<(String, String)> = sqlx::query_as(OBJECTS).bind(schema).fetch_all(p).await?;
    Ok(rows
        .into_iter()
        .map(|(name, t)| ObjectInfo {
            name,
            kind: kind_label(t.contains("VIEW")),
        })
        .collect())
}

pub(super) async fn columns(
    p: &MySqlPool,
    schema: &str,
    table: &str,
) -> AppResult<Vec<ColumnInfo>> {
    type Row = (
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(COLUMNS)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    let pks: Vec<(String, i64)> = sqlx::query_as(PRIMARY_KEY)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    Ok(rows
        .into_iter()
        .map(
            |(name, data_type, nullable, default, extra, comment, charset, collation)| {
                let pk_position = pks
                    .iter()
                    .find(|(c, _)| c == &name)
                    .map(|(_, pos)| *pos as i32);
                let flags = extra.to_ascii_lowercase();
                ColumnInfo {
                    name,
                    data_type,
                    nullable: nullable == "YES",
                    default: default
                        .and_then(|d| mysql_default_expr(&d, flags.contains("default_generated"))),
                    pk_position,
                    auto_increment: flags.contains("auto_increment"),
                    comment: Some(comment).filter(|c| !c.is_empty()),
                    charset: charset.filter(|c| !c.is_empty()),
                    collation: collation.filter(|c| !c.is_empty()),
                    extra: extra.trim().to_string(),
                }
            },
        )
        .collect())
}

/// MySQL reports defaults as bare values (`abc`, `0`) or, for expressions, the
/// expression with `DEFAULT_GENERATED` in EXTRA; MariaDB quotes literals itself.
/// Normalized to an SQL expression so it can be shown and written back unchanged.
pub fn mysql_default_expr(raw: &str, generated: bool) -> Option<String> {
    let t = raw.trim();
    let lower = t.to_ascii_lowercase();
    let is_now = [
        "current_timestamp",
        "now(",
        "localtime",
        "current_date",
        "current_time",
    ]
    .iter()
    .any(|k| lower.starts_with(k));
    if generated {
        return Some(if is_now || (t.starts_with('(') && t.ends_with(')')) {
            t.to_string()
        } else {
            format!("({t})")
        });
    }
    if t.eq_ignore_ascii_case("null") {
        return None;
    }
    if is_now
        || (t.len() >= 2 && t.starts_with('\'') && t.ends_with('\''))
        || t.parse::<f64>().is_ok()
    {
        return Some(t.to_string());
    }
    Some(mysql_literal(raw))
}

pub(super) async fn structure(
    p: &MySqlPool,
    schema: &str,
    table: &str,
    columns: Vec<ColumnInfo>,
) -> AppResult<TableStructure> {
    let ttype: String = sqlx::query_scalar(TABLE_TYPE)
        .bind(schema)
        .bind(table)
        .fetch_one(p)
        .await?;
    let is_view = ttype.contains("VIEW");
    Ok(TableStructure {
        kind: kind_label(is_view),
        columns,
        indexes: indexes(p, schema, table).await?,
        foreign_keys: foreign_keys(p, schema, table).await?,
        ddl: show_create(p, schema, table, is_view).await?,
    })
}

/// `SHOW CREATE TABLE` / `SHOW CREATE VIEW`; the text may come back as bytes.
async fn show_create(p: &MySqlPool, schema: &str, table: &str, is_view: bool) -> AppResult<String> {
    let what = if is_view { "VIEW" } else { "TABLE" };
    let sql = format!("SHOW CREATE {what} {}", qualified(D, schema, table));
    let row = sqlx::query(&sql).fetch_one(p).await?;
    Ok(row
        .try_get_unchecked::<String, _>(1)
        .or_else(|_| {
            row.try_get_unchecked::<Vec<u8>, _>(1)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        })
        .unwrap_or_default())
}

async fn indexes(p: &MySqlPool, schema: &str, table: &str) -> AppResult<Vec<IndexInfo>> {
    let rows: Vec<(String, i64, String, i64, String)> = sqlx::query_as(INDEX_COLUMNS)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    let mut indexes: Vec<IndexInfo> = Vec::new();
    for (name, non_unique, col, _, algorithm) in rows {
        match indexes.iter_mut().find(|i| i.name == name) {
            Some(i) => i.columns.push(col),
            None => indexes.push(IndexInfo {
                primary: name == "PRIMARY",
                unique: non_unique == 0,
                columns: vec![col],
                definition: String::new(),
                name,
                algorithm,
            }),
        }
    }
    for i in &mut indexes {
        i.definition = index_definition(i);
    }
    indexes.sort_by_key(|i| (!i.primary, i.name.clone()));
    Ok(indexes)
}

fn index_definition(i: &IndexInfo) -> String {
    let cols: Vec<String> = i.columns.iter().map(|c| quote_ident(D, c)).collect();
    if i.primary {
        format!("PRIMARY KEY ({})", cols.join(", "))
    } else {
        format!(
            "{}KEY {} ({})",
            if i.unique { "UNIQUE " } else { "" },
            quote_ident(D, &i.name),
            cols.join(", ")
        )
    }
}

async fn foreign_keys(p: &MySqlPool, schema: &str, table: &str) -> AppResult<Vec<ForeignKeyInfo>> {
    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(FK_COLUMNS)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    let mut fks: Vec<ForeignKeyInfo> = Vec::new();
    for (name, col, rschema, rtable, rcol) in rows {
        match fks.iter_mut().find(|f| f.name == name) {
            Some(f) => {
                f.columns.push(col);
                f.ref_columns.push(rcol);
            }
            None => fks.push(ForeignKeyInfo {
                name,
                columns: vec![col],
                ref_table: if rschema == schema {
                    rtable
                } else {
                    format!("{rschema}.{rtable}")
                },
                ref_columns: vec![rcol],
                on_update: "NO ACTION".into(),
                on_delete: "NO ACTION".into(),
                definition: String::new(),
            }),
        }
    }
    let rules: Vec<(String, String, String)> = sqlx::query_as(FK_RULES)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    for f in &mut fks {
        if let Some((_, upd, del)) = rules.iter().find(|r| r.0 == f.name) {
            f.on_update = upd.clone();
            f.on_delete = del.clone();
        }
        f.definition = fk_definition(f);
    }
    Ok(fks)
}

pub(super) async fn completion_tables(
    p: &MySqlPool,
    schema: &str,
) -> AppResult<BTreeMap<String, Vec<String>>> {
    let rows: Vec<(String, String)> = sqlx::query_as(COMPLETION).bind(schema).fetch_all(p).await?;
    Ok(group_columns(rows))
}
