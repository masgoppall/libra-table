//! CREATE TABLE with its primary key, foreign keys, comments and indexes.

use std::collections::HashSet;

use crate::db::sql::{literal_for, qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Driver;

use super::clauses::{
    bad, column_def, comment_of, create_index, fk_clause, need_name, quoted_list, sql,
    SQLITE_AUTOINCREMENT_RULE,
};
use super::plan::Step;
use super::spec::{ColumnSpec, ForeignKeySpec, IndexSpec};

/// The parts of a `SchemaChange::CreateTable`.
pub(super) struct TableDef<'a> {
    pub schema: &'a str,
    pub name: &'a str,
    pub columns: &'a [ColumnSpec],
    pub indexes: &'a [IndexSpec],
    pub foreign_keys: &'a [ForeignKeySpec],
    pub comment: &'a Option<String>,
}

pub(super) fn build_create_table(
    driver: Driver,
    t: &TableDef<'_>,
    steps: &mut Vec<Step>,
) -> AppResult<()> {
    need_name("The table", t.name)?;
    check_columns(t.columns)?;
    let pk: Vec<String> = t
        .columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.name.clone())
        .collect();
    // SQLite numbers rows itself only for a single INTEGER PRIMARY KEY.
    let sqlite_inline = driver == Driver::Sqlite && pk.len() == 1;
    if driver == Driver::Sqlite {
        check_sqlite_autoincrement(t.columns, &pk)?;
    }
    let mut lines = Vec::new();
    for c in t.columns {
        lines.push(column_def(driver, c, sqlite_inline && c.primary_key, true)?);
    }
    if !pk.is_empty() && !sqlite_inline {
        lines.push(format!("PRIMARY KEY ({})", quoted_list(driver, &pk)));
    }
    for fk in t.foreign_keys {
        lines.push(fk_clause(driver, t.schema, fk)?);
    }
    let table = qualified(driver, t.schema, t.name);
    let mut s = format!("CREATE TABLE {table} (\n  {}\n)", lines.join(",\n  "));
    if driver == Driver::Mysql {
        if let Some(cm) = comment_of(t.comment) {
            s.push_str(&format!(" COMMENT={}", literal_for(driver, cm)));
        }
    }
    sql(steps, s);
    if driver == Driver::Postgres {
        pg_comments(&table, t, steps);
    }
    for idx in t.indexes {
        sql(steps, create_index(driver, t.schema, t.name, idx)?);
    }
    Ok(())
}

fn check_columns(columns: &[ColumnSpec]) -> AppResult<()> {
    if columns.is_empty() {
        return Err(bad("A table needs at least one column."));
    }
    let mut seen = HashSet::new();
    for c in columns {
        if !seen.insert(c.name.to_lowercase()) {
            return Err(bad(format!("Column \"{}\" appears twice.", c.name)));
        }
    }
    Ok(())
}

fn check_sqlite_autoincrement(columns: &[ColumnSpec], pk: &[String]) -> AppResult<()> {
    let autos: Vec<&ColumnSpec> = columns.iter().filter(|c| c.auto_increment).collect();
    let Some(a) = autos.first() else {
        return Ok(());
    };
    if autos.len() > 1
        || !a.primary_key
        || pk.len() != 1
        || !a.data_type.trim().eq_ignore_ascii_case("integer")
    {
        return Err(bad(SQLITE_AUTOINCREMENT_RULE));
    }
    Ok(())
}

/// Postgres keeps comments outside CREATE TABLE.
fn pg_comments(table: &str, t: &TableDef<'_>, steps: &mut Vec<Step>) {
    let d = Driver::Postgres;
    if let Some(cm) = comment_of(t.comment) {
        sql(
            steps,
            format!("COMMENT ON TABLE {table} IS {}", literal_for(d, cm)),
        );
    }
    for c in t.columns {
        if let Some(cm) = comment_of(&c.comment) {
            let col = quote_ident(d, &c.name);
            sql(
                steps,
                format!("COMMENT ON COLUMN {table}.{col} IS {}", literal_for(d, cm)),
            );
        }
    }
}
