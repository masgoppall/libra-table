//! SQLite: `sqlite_master` joined with the table-valued pragmas, so all tables come back
//! in one query for columns and one for foreign keys.
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use sqlx::SqlitePool;

use crate::db::sql::quote_ident;
use crate::error::AppResult;
use crate::models::Driver;

use super::{DiagramColumn, DiagramForeignKey, Loaded};

/// User tables of the schema, as the `m` side of a join with a pragma.
const TABLES: &str = "m.type = 'table' AND m.name NOT LIKE 'sqlite\\_%' ESCAPE '\\'";

pub(super) async fn load(p: &SqlitePool, schema: &str) -> AppResult<Loaded> {
    let master = format!("{}.sqlite_master m", quote_ident(Driver::Sqlite, schema));
    let cols_sql = format!(
        "SELECT m.name, c.name, c.type, c.\"notnull\", c.pk \
         FROM {master}, pragma_table_info(m.name, ?1) c WHERE {TABLES} ORDER BY m.name, c.cid"
    );
    let fk_sql = format!(
        "SELECT m.name, f.id, f.\"table\", f.\"from\", f.\"to\" \
         FROM {master}, pragma_foreign_key_list(m.name, ?1) f WHERE {TABLES} \
         ORDER BY m.name, f.id, f.seq"
    );
    let cols: Vec<(String, String, String, i64, i64)> =
        sqlx::query_as(&cols_sql).bind(schema).fetch_all(p).await?;
    let rows: Vec<(String, i64, String, String, Option<String>)> =
        sqlx::query_as(&fk_sql).bind(schema).fetch_all(p).await?;
    let columns: Vec<(String, DiagramColumn)> = cols
        .into_iter()
        .map(|(table, name, data_type, notnull, pk)| {
            let col = DiagramColumn {
                name,
                data_type,
                nullable: notnull == 0,
                primary_key: pk > 0,
            };
            (table, col)
        })
        .collect();
    Ok((columns.clone(), foreign_keys(rows, schema, &columns)))
}

/// SQLite keys have no names (`fk_<id>`), and `to` is empty when the key points at the
/// referenced table's primary key.
fn foreign_keys(
    rows: Vec<(String, i64, String, String, Option<String>)>,
    schema: &str,
    columns: &[(String, DiagramColumn)],
) -> Vec<(String, DiagramForeignKey)> {
    let mut keys: Vec<(String, i64, DiagramForeignKey)> = Vec::new();
    for (table, id, ref_table, from, to) in rows {
        let to = to.unwrap_or_default();
        match keys.last_mut() {
            Some((t, kid, fk)) if *t == table && *kid == id => {
                fk.columns.push(from);
                fk.ref_columns.push(to);
            }
            _ => keys.push((
                table,
                id,
                DiagramForeignKey {
                    name: format!("fk_{id}"),
                    columns: vec![from],
                    ref_schema: schema.to_string(),
                    ref_table,
                    ref_columns: vec![to],
                },
            )),
        }
    }
    keys.into_iter()
        .map(|(table, _, mut fk)| {
            if fk.ref_columns.iter().all(String::is_empty) {
                fk.ref_columns = columns
                    .iter()
                    .filter(|(t, c)| t == &fk.ref_table && c.primary_key)
                    .map(|(_, c)| c.name.clone())
                    .collect();
            }
            (table, fk)
        })
        .collect()
}
