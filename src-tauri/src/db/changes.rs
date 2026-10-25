//! Cell edits (GRD-4): pending updates / inserts / deletes become parameterized
//! statements, previewed as SQL, then applied in one transaction. Every UPDATE and
//! DELETE must touch exactly one row, or the whole commit rolls back.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::db::meta::{primary_key, ColumnInfo};
use crate::db::sql::{qualified, quote_ident, Placeholder};
use crate::db::{bind_all, DbPool};
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    Update {
        pk: Map<String, Value>,
        values: Map<String, Value>,
    },
    Insert {
        values: Map<String, Value>,
    },
    Delete {
        pk: Map<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub sql: String,
    pub binds: Vec<Option<String>>,
    /// UPDATE / DELETE must affect exactly one row.
    pub expect_one: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub affected: u64,
    pub statements: Vec<String>,
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

const STALE_ROW_HINT: &str =
    "The row may have been changed or deleted by someone else. Refresh and try again.";

pub struct Builder<'a> {
    driver: Dialect,
    schema: &'a str,
    table: &'a str,
    columns: &'a [ColumnInfo],
}

impl<'a> Builder<'a> {
    pub fn new(
        driver: Dialect,
        schema: &'a str,
        table: &'a str,
        columns: &'a [ColumnInfo],
    ) -> Self {
        Self {
            driver,
            schema,
            table,
            columns,
        }
    }

    fn column(&self, name: &str) -> AppResult<&ColumnInfo> {
        self.columns
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| AppError::validation(format!("Unknown column \"{name}\".")))
    }

    fn param(&self, ph: &mut Placeholder, col: &ColumnInfo) -> String {
        let p = ph.next_param();
        if self.driver == Dialect::Postgres {
            format!("{p}::{}", col.data_type)
        } else {
            p
        }
    }

    fn where_pk(
        &self,
        pk: &Map<String, Value>,
        ph: &mut Placeholder,
        binds: &mut Vec<Option<String>>,
    ) -> AppResult<String> {
        let keys = primary_key(self.columns);
        if keys.is_empty() {
            return Err(AppError::validation(
                "This table has no primary key, so rows can't be edited safely.",
            )
            .hint("Add a primary key, or change rows with a SQL query."));
        }
        let mut parts = Vec::new();
        for k in &keys {
            let v = pk.get(k).ok_or_else(|| {
                AppError::validation(format!("Missing primary key value for \"{k}\"."))
            })?;
            let col = self.column(k)?;
            match cell(k, v)? {
                Cell::Text(t) => {
                    binds.push(Some(t));
                    parts.push(format!(
                        "{} = {}",
                        quote_ident(self.driver, k),
                        self.param(ph, col)
                    ));
                }
                _ => {
                    return Err(AppError::validation(format!(
                        "Primary key \"{k}\" can't be NULL."
                    )))
                }
            }
        }
        Ok(parts.join(" AND "))
    }

    /// `pk = …` conditions for one row and their binds (placeholders start at 1).
    pub(crate) fn pk_condition(
        &self,
        pk: &Map<String, Value>,
    ) -> AppResult<(String, Vec<Option<String>>)> {
        let mut ph = Placeholder::new(self.driver, 1);
        let mut binds = Vec::new();
        let sql = self.where_pk(pk, &mut ph, &mut binds)?;
        Ok((sql, binds))
    }

    pub fn build(&self, changes: &[Change]) -> AppResult<Vec<Stmt>> {
        let mut out = Vec::new();
        for ch in changes {
            let stmt = match ch {
                Change::Update { pk, values } => {
                    if values.is_empty() {
                        continue;
                    }
                    self.update(pk, values)?
                }
                Change::Insert { values } => self.insert(values)?,
                Change::Delete { pk } => self.delete(pk)?,
            };
            out.push(stmt);
        }
        Ok(out)
    }

    fn table(&self) -> String {
        qualified(self.driver, self.schema, self.table)
    }

    fn update(&self, pk: &Map<String, Value>, values: &Map<String, Value>) -> AppResult<Stmt> {
        let mut ph = Placeholder::new(self.driver, 1);
        let mut binds = Vec::new();
        let mut sets = Vec::new();
        for (name, v) in values {
            let col = self.column(name)?;
            let qn = quote_ident(self.driver, name);
            match cell(name, v)? {
                Cell::Null => sets.push(format!("{qn} = NULL")),
                Cell::Default => sets.push(format!("{qn} = DEFAULT")),
                Cell::Text(t) => {
                    binds.push(Some(t));
                    sets.push(format!("{qn} = {}", self.param(&mut ph, col)));
                }
            }
        }
        let w = self.where_pk(pk, &mut ph, &mut binds)?;
        Ok(Stmt {
            sql: format!("UPDATE {} SET {} WHERE {w}", self.table(), sets.join(", ")),
            binds,
            expect_one: true,
        })
    }

    /// Columns set to DEFAULT are left out, so the database fills them in.
    fn insert(&self, values: &Map<String, Value>) -> AppResult<Stmt> {
        let mut ph = Placeholder::new(self.driver, 1);
        let mut binds = Vec::new();
        let mut names = Vec::new();
        let mut vals = Vec::new();
        for (name, v) in values {
            let col = self.column(name)?;
            match cell(name, v)? {
                Cell::Default => continue,
                Cell::Null => {
                    names.push(quote_ident(self.driver, name));
                    vals.push("NULL".to_string());
                }
                Cell::Text(t) => {
                    names.push(quote_ident(self.driver, name));
                    binds.push(Some(t));
                    vals.push(self.param(&mut ph, col));
                }
            }
        }
        let table = self.table();
        let sql = if names.is_empty() {
            match self.driver {
                Dialect::Mysql => format!("INSERT INTO {table} () VALUES ()"),
                _ => format!("INSERT INTO {table} DEFAULT VALUES"),
            }
        } else {
            format!(
                "INSERT INTO {table} ({}) VALUES ({})",
                names.join(", "),
                vals.join(", ")
            )
        };
        Ok(Stmt {
            sql,
            binds,
            expect_one: false,
        })
    }

    fn delete(&self, pk: &Map<String, Value>) -> AppResult<Stmt> {
        let mut ph = Placeholder::new(self.driver, 1);
        let mut binds = Vec::new();
        let w = self.where_pk(pk, &mut ph, &mut binds)?;
        Ok(Stmt {
            sql: format!("DELETE FROM {} WHERE {w}", self.table()),
            binds,
            expect_one: true,
        })
    }
}

/// SQL with values inlined, for the preview dialog only (never executed).
pub fn preview(driver: Dialect, stmt: &Stmt) -> String {
    crate::db::sql::inline_binds(driver, &stmt.sql, &stmt.binds)
}

/// An UPDATE or DELETE that didn't touch exactly one row.
fn not_one_row(change: usize, affected: u64) -> AppError {
    AppError::query(format!(
        "Change {change} affected {affected} rows instead of 1, so nothing was saved."
    ))
    .hint(STALE_ROW_HINT)
}

/// Applies the statements in one transaction.
pub async fn apply(pool: &DbPool, stmts: &[Stmt]) -> AppResult<u64> {
    macro_rules! run {
        ($p:expr) => {{
            let mut tx = $p.begin().await?;
            let mut total = 0u64;
            for (i, s) in stmts.iter().enumerate() {
                let res = bind_all!(sqlx::query(&s.sql), &s.binds)
                    .execute(&mut *tx)
                    .await?;
                let n = res.rows_affected();
                if s.expect_one && n != 1 {
                    tx.rollback().await?;
                    return Err(not_one_row(i + 1, n));
                }
                total += n;
            }
            tx.commit().await?;
            Ok(total)
        }};
    }
    match pool {
        DbPool::Postgres(p) => run!(p),
        DbPool::Mysql(p) => run!(p),
        DbPool::Sqlite(p) => run!(p),
        DbPool::Engine(_) => Err(crate::db::engine::sqlx_only()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cols() -> Vec<ColumnInfo> {
        vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "integer".into(),
                nullable: false,
                default: None,
                pk_position: Some(1),
                auto_increment: false,
                comment: None,
            },
            ColumnInfo {
                name: "name".into(),
                data_type: "text".into(),
                nullable: true,
                default: None,
                pk_position: None,
                auto_increment: false,
                comment: None,
            },
            ColumnInfo {
                name: "at".into(),
                data_type: "timestamp with time zone".into(),
                nullable: true,
                default: Some("now()".into()),
                pk_position: None,
                auto_increment: false,
                comment: None,
            },
        ]
    }

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn update_insert_delete_postgres() {
        let c = cols();
        let b = Builder::new(Dialect::Postgres, "public", "t", &c);
        let stmts = b
            .build(&[
                Change::Update {
                    pk: map(json!({"id": 7})),
                    values: map(json!({"name": "O'Neil", "at": {"$default": true}})),
                },
                Change::Insert {
                    values: map(json!({"name": null, "at": {"$default": true}})),
                },
                Change::Delete {
                    pk: map(json!({"id": "9"})),
                },
            ])
            .unwrap();
        assert_eq!(
            stmts[0].sql,
            r#"UPDATE "public"."t" SET "at" = DEFAULT, "name" = $1::text WHERE "id" = $2::integer"#
        );
        assert_eq!(
            stmts[0].binds,
            vec![Some("O'Neil".into()), Some("7".into())]
        );
        assert!(stmts[0].expect_one);
        assert_eq!(
            stmts[1].sql,
            r#"INSERT INTO "public"."t" ("name") VALUES (NULL)"#
        );
        assert_eq!(
            stmts[2].sql,
            r#"DELETE FROM "public"."t" WHERE "id" = $1::integer"#
        );
        assert_eq!(
            preview(Dialect::Postgres, &stmts[0]),
            concat!(
                r#"UPDATE "public"."t" SET "at" = DEFAULT, "name" = 'O''Neil'::text "#,
                r#"WHERE "id" = '7'::integer"#
            )
        );
    }

    #[test]
    fn empty_insert_and_mysql_placeholders() {
        let c = cols();
        let b = Builder::new(Dialect::Mysql, "shop", "t", &c);
        let s = b
            .build(&[
                Change::Insert { values: Map::new() },
                Change::Update {
                    pk: map(json!({"id": 1})),
                    values: map(json!({"name": "x"})),
                },
            ])
            .unwrap();
        assert_eq!(s[0].sql, "INSERT INTO `shop`.`t` () VALUES ()");
        assert_eq!(s[1].sql, "UPDATE `shop`.`t` SET `name` = ? WHERE `id` = ?");
        assert_eq!(
            preview(Dialect::Mysql, &s[1]),
            "UPDATE `shop`.`t` SET `name` = 'x' WHERE `id` = '1'"
        );
    }

    #[test]
    fn rejects_missing_pk_binary_and_unknown_columns() {
        let mut no_pk = cols();
        no_pk[0].pk_position = None;
        let b = Builder::new(Dialect::Sqlite, "main", "t", &no_pk);
        assert!(b
            .build(&[Change::Delete {
                pk: map(json!({"id": 1}))
            }])
            .is_err());
        let c = cols();
        let b = Builder::new(Dialect::Sqlite, "main", "t", &c);
        assert!(b
            .build(&[Change::Update {
                pk: map(json!({"id": 1})),
                values: map(json!({"name": {"$binary": "00", "size": 1}}))
            }])
            .is_err());
        assert!(b
            .build(&[Change::Insert {
                values: map(json!({"nope": 1}))
            }])
            .is_err());
        assert!(b
            .build(&[Change::Delete {
                pk: map(json!({"id": null}))
            }])
            .is_err());
    }
}
