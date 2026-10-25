//! Postgres: all columns of the schema's tables in one query, all foreign keys in another.
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use sqlx::PgPool;

use crate::error::AppResult;

use super::{DiagramColumn, DiagramForeignKey, Loaded};

/// Partitions are left out: their parent stands for them.
const COLUMNS: &str = "SELECT c.relname::text, a.attname::text, \
        format_type(a.atttypid, a.atttypmod), NOT a.attnotnull, \
        COALESCE(a.attnum = ANY(i.indkey::int2[]), false) \
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
    JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
    LEFT JOIN pg_index i ON i.indrelid = c.oid AND i.indisprimary \
    WHERE n.nspname = $1 AND c.relkind IN ('r','p','f') AND NOT c.relispartition \
    ORDER BY c.relname, a.attnum";

const FOREIGN_KEYS: &str = "SELECT t.relname::text, con.conname::text, \
        rn.nspname::text, rt.relname::text, \
        ARRAY(SELECT a.attname::text FROM unnest(con.conkey) WITH ORDINALITY k(n, o) \
              JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.n \
              ORDER BY k.o), \
        ARRAY(SELECT a.attname::text FROM unnest(con.confkey) WITH ORDINALITY k(n, o) \
              JOIN pg_attribute a ON a.attrelid = con.confrelid AND a.attnum = k.n \
              ORDER BY k.o) \
    FROM pg_constraint con JOIN pg_class t ON t.oid = con.conrelid \
    JOIN pg_namespace n ON n.oid = t.relnamespace \
    JOIN pg_class rt ON rt.oid = con.confrelid \
    JOIN pg_namespace rn ON rn.oid = rt.relnamespace \
    WHERE n.nspname = $1 AND con.contype = 'f' AND con.conparentid = 0 \
    ORDER BY t.relname, con.conname";

pub(super) async fn load(p: &PgPool, schema: &str) -> AppResult<Loaded> {
    let cols: Vec<(String, String, String, bool, bool)> =
        sqlx::query_as(COLUMNS).bind(schema).fetch_all(p).await?;
    let fks: Vec<(String, String, String, String, Vec<String>, Vec<String>)> =
        sqlx::query_as(FOREIGN_KEYS)
            .bind(schema)
            .fetch_all(p)
            .await?;
    let columns = cols
        .into_iter()
        .map(|(table, name, data_type, nullable, primary_key)| {
            let col = DiagramColumn {
                name,
                data_type,
                nullable,
                primary_key,
            };
            (table, col)
        })
        .collect();
    let keys = fks
        .into_iter()
        .map(
            |(table, name, ref_schema, ref_table, columns, ref_columns)| {
                let fk = DiagramForeignKey {
                    name,
                    columns,
                    ref_schema,
                    ref_table,
                    ref_columns,
                };
                (table, fk)
            },
        )
        .collect();
    Ok((columns, keys))
}
