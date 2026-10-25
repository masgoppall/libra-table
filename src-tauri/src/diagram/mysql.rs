//! MySQL: columns of the schema's base tables and the key column usage of its foreign
//! keys, one information_schema query each.
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use sqlx::MySqlPool;

use crate::error::AppResult;

use super::{DiagramColumn, DiagramForeignKey, Loaded};

const COLUMNS: &str = "SELECT CAST(c.TABLE_NAME AS CHAR), CAST(c.COLUMN_NAME AS CHAR), \
        CAST(c.COLUMN_TYPE AS CHAR), CAST(c.IS_NULLABLE AS CHAR), CAST(c.COLUMN_KEY AS CHAR) \
    FROM information_schema.COLUMNS c JOIN information_schema.TABLES t \
        ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME \
    WHERE c.TABLE_SCHEMA = ? AND t.TABLE_TYPE = 'BASE TABLE' \
    ORDER BY c.TABLE_NAME, c.ORDINAL_POSITION";

const FOREIGN_KEYS: &str = "SELECT CAST(TABLE_NAME AS CHAR), CAST(CONSTRAINT_NAME AS CHAR), \
        CAST(COLUMN_NAME AS CHAR), CAST(REFERENCED_TABLE_SCHEMA AS CHAR), \
        CAST(REFERENCED_TABLE_NAME AS CHAR), CAST(REFERENCED_COLUMN_NAME AS CHAR) \
    FROM information_schema.KEY_COLUMN_USAGE \
    WHERE TABLE_SCHEMA = ? AND REFERENCED_TABLE_NAME IS NOT NULL \
    ORDER BY TABLE_NAME, CONSTRAINT_NAME, ORDINAL_POSITION";

pub(super) async fn load(p: &MySqlPool, schema: &str) -> AppResult<Loaded> {
    let cols: Vec<(String, String, String, String, String)> =
        sqlx::query_as(COLUMNS).bind(schema).fetch_all(p).await?;
    let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as(FOREIGN_KEYS)
        .bind(schema)
        .fetch_all(p)
        .await?;
    let columns = cols
        .into_iter()
        .map(|(table, name, data_type, nullable, key)| {
            let col = DiagramColumn {
                name,
                data_type,
                nullable: nullable == "YES",
                primary_key: key == "PRI",
            };
            (table, col)
        })
        .collect();
    let mut keys: Vec<(String, DiagramForeignKey)> = Vec::new();
    for (table, name, col, ref_schema, ref_table, ref_col) in rows {
        match keys.last_mut() {
            Some((t, fk)) if *t == table && fk.name == name => {
                fk.columns.push(col);
                fk.ref_columns.push(ref_col);
            }
            _ => keys.push((
                table,
                DiagramForeignKey {
                    name,
                    columns: vec![col],
                    ref_schema,
                    ref_table,
                    ref_columns: vec![ref_col],
                },
            )),
        }
    }
    Ok((columns, keys))
}
