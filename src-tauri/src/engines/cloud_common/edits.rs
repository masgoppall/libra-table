//! Grid edits as SQL for the HTTP SQL services: UPDATE / DELETE by primary key, INSERT
//! without DEFAULT columns, and the Quick Look select. Values are bound like filters.

use serde_json::{Map, Value};

use super::sql::{column, Binds, Flavor};
use crate::db::changes::Change;
use crate::db::meta::{primary_key, ColumnInfo};
use crate::error::{AppError, AppResult};

/// One grid change as SQL. `key_where` is the row's key condition (UPDATE / DELETE).
pub struct Edit {
    pub sql: String,
    pub key_where: Option<String>,
}

enum Cell {
    Null,
    Default,
    Text(String),
}

fn cell(column: &str, v: &Value) -> AppResult<Cell> {
    Ok(match v {
        Value::Null => Cell::Null,
        Value::Bool(b) => Cell::Text(b.to_string()),
        Value::Number(n) => Cell::Text(n.to_string()),
        Value::String(s) => Cell::Text(s.clone()),
        Value::Object(o) if o.get("$default").and_then(Value::as_bool) == Some(true) => {
            Cell::Default
        }
        Value::Object(o) if o.contains_key("$binary") => {
            return Err(AppError::validation(format!(
                "Editing binary values is not supported yet (column \"{column}\")."
            )))
        }
        other => Cell::Text(other.to_string()),
    })
}

/// The table of an edit, with its columns.
pub struct EditTable<'a> {
    pub schema: &'a str,
    pub table: &'a str,
    pub columns: &'a [ColumnInfo],
}

impl EditTable<'_> {
    fn editable(&self, f: &dyn Flavor, name: &str) -> AppResult<&ColumnInfo> {
        let c = column(self.columns, name)?;
        if !f.scalar(&c.data_type) {
            return Err(AppError::validation(format!(
                "Column \"{name}\" ({}) can't be edited in the grid; use a query.",
                c.data_type
            )));
        }
        Ok(c)
    }

    fn value(&self, f: &dyn Flavor, c: &ColumnInfo, text: String, b: &mut Binds) -> String {
        f.typed(b.push(f, text), &c.data_type)
    }

    /// `k1 = … AND k2 = …` for one row.
    pub fn key_where(
        &self,
        f: &dyn Flavor,
        pk: &Map<String, Value>,
        b: &mut Binds,
    ) -> AppResult<String> {
        let keys = primary_key(self.columns);
        if keys.is_empty() {
            return Err(AppError::validation(
                "This table has no primary key, so rows can't be edited safely.",
            )
            .hint("Declare a primary key, or change rows with a SQL query."));
        }
        let mut parts = Vec::new();
        for k in &keys {
            let v = pk.get(k).ok_or_else(|| {
                AppError::validation(format!("Missing primary key value for \"{k}\"."))
            })?;
            let c = self.editable(f, k)?;
            match cell(k, v)? {
                Cell::Text(t) => parts.push(format!("{} = {}", f.quote(k), self.value(f, c, t, b))),
                _ => {
                    return Err(AppError::validation(format!(
                        "Primary key \"{k}\" can't be NULL."
                    )))
                }
            }
        }
        Ok(parts.join(" AND "))
    }

    /// The statement for one change; None for an update without values.
    pub fn edit(&self, f: &dyn Flavor, change: &Change, b: &mut Binds) -> AppResult<Option<Edit>> {
        let table = f.table(self.schema, self.table);
        Ok(Some(match change {
            Change::Update { pk, values } => {
                if values.is_empty() {
                    return Ok(None);
                }
                let mut sets = Vec::new();
                for (name, v) in values {
                    let c = self.editable(f, name)?;
                    let q = f.quote(name);
                    sets.push(match cell(name, v)? {
                        Cell::Null => format!("{q} = NULL"),
                        Cell::Default if f.update_default() => format!("{q} = DEFAULT"),
                        Cell::Default => {
                            return Err(AppError::validation(format!(
                                "Column \"{name}\" can't be reset to its default here."
                            )))
                        }
                        Cell::Text(t) => format!("{q} = {}", self.value(f, c, t, b)),
                    });
                }
                let w = self.key_where(f, pk, b)?;
                Edit {
                    sql: format!("UPDATE {table} SET {} WHERE {w}", sets.join(", ")),
                    key_where: Some(w),
                }
            }
            Change::Insert { values } => {
                let mut names = Vec::new();
                let mut vals = Vec::new();
                for (name, v) in values {
                    let c = self.editable(f, name)?;
                    match cell(name, v)? {
                        Cell::Default => continue,
                        Cell::Null => vals.push("NULL".to_string()),
                        Cell::Text(t) => vals.push(self.value(f, c, t, b)),
                    }
                    names.push(f.quote(name));
                }
                if names.is_empty() {
                    return Err(AppError::validation(
                        "Enter at least one value for the new row.",
                    ));
                }
                Edit {
                    sql: format!(
                        "INSERT INTO {table} ({}) {}",
                        names.join(", "),
                        f.insert_values(&vals)
                    ),
                    key_where: None,
                }
            }
            Change::Delete { pk } => {
                let w = self.key_where(f, pk, b)?;
                Edit {
                    sql: format!("DELETE FROM {table} WHERE {w}"),
                    key_where: Some(w),
                }
            }
        }))
    }
}

/// `SELECT column FROM table WHERE key` for Quick Look.
pub fn cell_select(
    f: &dyn Flavor,
    t: &EditTable<'_>,
    pk: &Map<String, Value>,
    col: &str,
    b: &mut Binds,
) -> AppResult<String> {
    let c = column(t.columns, col)?;
    let w = t.key_where(f, pk, b)?;
    Ok(format!(
        "SELECT {} FROM {} WHERE {w}",
        f.quote(&c.name),
        f.table(t.schema, t.table)
    ))
}
