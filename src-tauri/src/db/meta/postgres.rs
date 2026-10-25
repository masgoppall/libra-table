//! Postgres catalog queries (pg_catalog).
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use std::collections::BTreeMap;

use sqlx::PgPool;

use crate::db::sql::{qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Dialect;

use super::{
    group_columns, kind_label, ColumnInfo, ForeignKeyInfo, IndexInfo, ObjectInfo, TableStructure,
};

const SYSTEM_SCHEMAS: &[&str] = &["pg_catalog", "information_schema"];
const D: Dialect = Dialect::Postgres;

const DATABASES: &str = "SELECT datname::text FROM pg_database \
    WHERE datallowconn AND NOT datistemplate ORDER BY datname";

const SCHEMAS: &str = "SELECT nspname::text FROM pg_namespace \
    WHERE nspname NOT LIKE 'pg\\_toast%' AND nspname NOT LIKE 'pg\\_temp\\_%' \
    ORDER BY nspname";

const OBJECTS: &str = "SELECT c.relname::text, c.relkind::text FROM pg_class c \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND c.relkind IN ('r','p','f','v','m') ORDER BY c.relname";

const COLUMNS: &str = "SELECT a.attname::text, format_type(a.atttypid, a.atttypmod), \
        NOT a.attnotnull, pg_get_expr(d.adbin, d.adrelid), \
        array_position(i.indkey::int2[], a.attnum)::int4, \
        a.attidentity::text, col_description(c.oid, a.attnum) \
    FROM pg_attribute a \
    JOIN pg_class c ON c.oid = a.attrelid \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
    LEFT JOIN pg_index i ON i.indrelid = c.oid AND i.indisprimary \
    WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped \
    ORDER BY a.attnum";

const RELKIND: &str = "SELECT c.relkind::text FROM pg_class c \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND c.relname = $2";

const INDEXES: &str = "SELECT i.relname::text, ix.indisunique, ix.indisprimary, \
        pg_get_indexdef(ix.indexrelid), \
        ARRAY(SELECT pg_get_indexdef(ix.indexrelid, k, true) \
              FROM generate_series(1, ix.indnkeyatts) k) \
    FROM pg_index ix JOIN pg_class i ON i.oid = ix.indexrelid \
    JOIN pg_class t ON t.oid = ix.indrelid JOIN pg_namespace n ON n.oid = t.relnamespace \
    WHERE n.nspname = $1 AND t.relname = $2 ORDER BY ix.indisprimary DESC, i.relname";

const FOREIGN_KEYS: &str = "SELECT con.conname::text, pg_get_constraintdef(con.oid), \
        ARRAY(SELECT a.attname::text FROM unnest(con.conkey) WITH ORDINALITY k(n, o) \
              JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.n \
              ORDER BY k.o), \
        (SELECT quote_ident(rn.nspname) || '.' || quote_ident(rc.relname) FROM pg_class rc \
              JOIN pg_namespace rn ON rn.oid = rc.relnamespace WHERE rc.oid = con.confrelid), \
        ARRAY(SELECT a.attname::text FROM unnest(con.confkey) WITH ORDINALITY k(n, o) \
              JOIN pg_attribute a ON a.attrelid = con.confrelid AND a.attnum = k.n \
              ORDER BY k.o), \
        con.confupdtype::text, con.confdeltype::text \
    FROM pg_constraint con JOIN pg_class t ON t.oid = con.conrelid \
    JOIN pg_namespace n ON n.oid = t.relnamespace \
    WHERE n.nspname = $1 AND t.relname = $2 AND con.contype = 'f' ORDER BY con.conname";

const VIEW_DEF: &str = "SELECT pg_get_viewdef(c.oid, true) FROM pg_class c \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND c.relname = $2";

const CONSTRAINTS: &str = "SELECT con.conname::text, pg_get_constraintdef(con.oid) \
    FROM pg_constraint con \
    JOIN pg_class t ON t.oid = con.conrelid JOIN pg_namespace n ON n.oid = t.relnamespace \
    WHERE n.nspname = $1 AND t.relname = $2 AND con.contype IN ('p','u','f','c','x') \
    ORDER BY CASE con.contype WHEN 'p' THEN 0 WHEN 'u' THEN 1 WHEN 'f' THEN 2 ELSE 3 END, \
        con.conname";

const CONSTRAINT_INDEXES: &str = "SELECT i.relname::text FROM pg_constraint con \
    JOIN pg_class i ON i.oid = con.conindid \
    JOIN pg_class t ON t.oid = con.conrelid JOIN pg_namespace n ON n.oid = t.relnamespace \
    WHERE n.nspname = $1 AND t.relname = $2 AND con.conindid <> 0";

const COMPLETION: &str = "SELECT c.relname::text, a.attname::text FROM pg_attribute a \
    JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND c.relkind IN ('r','p','f','v','m') \
        AND a.attnum > 0 AND NOT a.attisdropped \
    ORDER BY c.relname, a.attnum";

/// (name, unique, primary, definition, columns)
type IndexRow = (String, bool, bool, String, Vec<String>);

pub(super) async fn databases(p: &PgPool) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar(DATABASES).fetch_all(p).await?)
}

pub(super) async fn schemas(p: &PgPool, show_system: bool) -> AppResult<Vec<String>> {
    let all: Vec<String> = sqlx::query_scalar(SCHEMAS).fetch_all(p).await?;
    Ok(all
        .into_iter()
        .filter(|s| show_system || !SYSTEM_SCHEMAS.contains(&s.as_str()))
        .collect())
}

pub(super) async fn objects(p: &PgPool, schema: &str) -> AppResult<Vec<ObjectInfo>> {
    let rows: Vec<(String, String)> = sqlx::query_as(OBJECTS).bind(schema).fetch_all(p).await?;
    Ok(rows
        .into_iter()
        .map(|(name, k)| ObjectInfo {
            name,
            kind: kind_label(k == "v" || k == "m"),
        })
        .collect())
}

pub(super) async fn columns(p: &PgPool, schema: &str, table: &str) -> AppResult<Vec<ColumnInfo>> {
    let rows: Vec<(
        String,
        String,
        bool,
        Option<String>,
        Option<i32>,
        String,
        Option<String>,
    )> = sqlx::query_as(COLUMNS)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    Ok(rows
        .into_iter()
        .map(
            |(name, data_type, nullable, default, pk, identity, comment)| ColumnInfo {
                auto_increment: !identity.is_empty()
                    || default
                        .as_deref()
                        .is_some_and(|d| d.starts_with("nextval(")),
                name,
                data_type,
                nullable,
                default,
                pk_position: pk,
                comment: comment.filter(|c| !c.is_empty()),
            },
        )
        .collect())
}

pub(super) async fn structure(
    p: &PgPool,
    schema: &str,
    table: &str,
    columns: Vec<ColumnInfo>,
) -> AppResult<TableStructure> {
    let kind: String = sqlx::query_scalar(RELKIND)
        .bind(schema)
        .bind(table)
        .fetch_one(p)
        .await?;
    let is_view = kind == "v" || kind == "m";
    let indexes: Vec<IndexRow> = sqlx::query_as(INDEXES)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    let ddl = if is_view {
        view_ddl(p, schema, table, &kind).await?
    } else {
        table_ddl(p, schema, table, &columns, &indexes).await?
    };
    Ok(TableStructure {
        kind: kind_label(is_view),
        columns,
        indexes: indexes.into_iter().map(index_info).collect(),
        foreign_keys: foreign_keys(p, schema, table).await?,
        ddl,
    })
}

fn index_info((name, unique, primary, definition, columns): IndexRow) -> IndexInfo {
    IndexInfo {
        name,
        columns,
        unique,
        primary,
        definition,
    }
}

async fn foreign_keys(p: &PgPool, schema: &str, table: &str) -> AppResult<Vec<ForeignKeyInfo>> {
    let rows: Vec<(
        String,
        String,
        Vec<String>,
        String,
        Vec<String>,
        String,
        String,
    )> = sqlx::query_as(FOREIGN_KEYS)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    Ok(rows
        .into_iter()
        .map(
            |(name, definition, columns, ref_table, ref_columns, upd, del)| ForeignKeyInfo {
                name,
                columns,
                ref_table,
                ref_columns,
                on_update: fk_action(&upd).into(),
                on_delete: fk_action(&del).into(),
                definition,
            },
        )
        .collect())
}

/// `pg_constraint.confupdtype` / `confdeltype` codes.
fn fk_action(code: &str) -> &'static str {
    match code {
        "r" => "RESTRICT",
        "c" => "CASCADE",
        "n" => "SET NULL",
        "d" => "SET DEFAULT",
        _ => "NO ACTION",
    }
}

async fn view_ddl(p: &PgPool, schema: &str, table: &str, kind: &str) -> AppResult<String> {
    let def: String = sqlx::query_scalar(VIEW_DEF)
        .bind(schema)
        .bind(table)
        .fetch_one(p)
        .await?;
    let kw = if kind == "m" {
        "MATERIALIZED VIEW"
    } else {
        "VIEW"
    };
    Ok(format!(
        "CREATE {kw} {} AS\n{}",
        qualified(D, schema, table),
        def.trim_end()
    ))
}

async fn table_ddl(
    p: &PgPool,
    schema: &str,
    table: &str,
    columns: &[ColumnInfo],
    indexes: &[IndexRow],
) -> AppResult<String> {
    let constraints: Vec<(String, String)> = sqlx::query_as(CONSTRAINTS)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    let constraint_indexes: Vec<String> = sqlx::query_scalar(CONSTRAINT_INDEXES)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    Ok(create_table_sql(
        schema,
        table,
        columns,
        &constraints,
        indexes,
        &constraint_indexes,
    ))
}

/// A readable CREATE TABLE plus the indexes that no constraint owns.
fn create_table_sql(
    schema: &str,
    table: &str,
    columns: &[ColumnInfo],
    constraints: &[(String, String)],
    indexes: &[IndexRow],
    constraint_indexes: &[String],
) -> String {
    let mut lines: Vec<String> = columns
        .iter()
        .map(|c| {
            let mut l = format!("    {} {}", quote_ident(D, &c.name), c.data_type);
            if let Some(d) = &c.default {
                l.push_str(&format!(" DEFAULT {d}"));
            }
            if !c.nullable {
                l.push_str(" NOT NULL");
            }
            l
        })
        .collect();
    for (name, def) in constraints {
        lines.push(format!("    CONSTRAINT {} {def}", quote_ident(D, name)));
    }
    let mut ddl = format!(
        "CREATE TABLE {} (\n{}\n);",
        qualified(D, schema, table),
        lines.join(",\n")
    );
    for (name, _, _, def, _) in indexes {
        if !constraint_indexes.contains(name) {
            ddl.push_str(&format!("\n{def};"));
        }
    }
    ddl
}

pub(super) async fn completion_tables(
    p: &PgPool,
    schema: &str,
) -> AppResult<BTreeMap<String, Vec<String>>> {
    let rows: Vec<(String, String)> = sqlx::query_as(COMPLETION).bind(schema).fetch_all(p).await?;
    Ok(group_columns(rows))
}
