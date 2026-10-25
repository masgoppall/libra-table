//! Turns filter rows (`column · operator · value`) into a parameterized WHERE fragment.
//!
//! Unlike TablePlus, values are never pasted into SQL: every value becomes a bind
//! parameter (FLT-4). `%` and `_` typed into Contains / prefix / suffix are escaped with
//! `ESCAPE '!'`, which behaves the same on Postgres, MySQL (even with
//! NO_BACKSLASH_ESCAPES) and SQLite. Enabled rows combine as `(k1) AND (k2)` (FLT-5).

mod operators;
#[cfg(test)]
mod tests;
mod values;

pub use operators::{operators, Case, Op, OperatorInfo};
pub use values::{escape_like, parse_list};

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::db::sql::{quote_ident, Placeholder};
use crate::error::{AppError, AppResult};
use crate::models::Dialect;
use operators::supported;
use values::LIKE_ESCAPE;

/// Pseudo-column: apply the operator to every column (FLT-3).
pub const ANY_COLUMN: &str = "__any__";
/// Pseudo-column: the value is a raw WHERE fragment (FLT-3).
pub const RAW_SQL: &str = "__raw__";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FilterRow {
    pub enabled: bool,
    pub column: String,
    pub operator: String,
    pub value: String,
    /// Upper bound for BETWEEN / NOT BETWEEN.
    pub value2: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    pub column: String,
    #[serde(default)]
    pub desc: bool,
}

/// A WHERE fragment (without the `WHERE` keyword) plus its bind values, in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterSql {
    pub sql: String,
    pub binds: Vec<String>,
}

impl FilterSql {
    pub fn is_empty(&self) -> bool {
        self.sql.is_empty()
    }
}

/// A whole statement with its binds, as `select_sql` returns it.
impl From<(String, Vec<String>)> for FilterSql {
    fn from((sql, binds): (String, Vec<String>)) -> Self {
        Self { sql, binds }
    }
}

fn need_value(v: &str) -> AppResult<String> {
    if v.is_empty() {
        Err(AppError::validation("Enter a value for this filter."))
    } else {
        Ok(v.to_string())
    }
}

/// Builds a parameterized WHERE fragment.
///
/// * `columns`: every column of the table in display order, with its SQL type
///   (`format_type` on Postgres; used to cast text binds to the column type).
/// * `only`: apply just this row (the per-row "Apply" button); otherwise all enabled rows.
pub struct FilterBuilder<'a> {
    driver: Dialect,
    columns: &'a [(String, String)],
    types: HashMap<&'a str, &'a str>,
    ph: Placeholder,
    binds: Vec<String>,
}

impl<'a> FilterBuilder<'a> {
    pub fn new(driver: Dialect, columns: &'a [(String, String)], first_placeholder: usize) -> Self {
        let types = columns
            .iter()
            .map(|(n, t)| (n.as_str(), t.as_str()))
            .collect();
        Self {
            driver,
            columns,
            types,
            ph: Placeholder::new(driver, first_placeholder),
            binds: Vec::new(),
        }
    }

    pub fn build(mut self, rows: &[FilterRow], only: Option<usize>) -> AppResult<FilterSql> {
        let selected: Vec<&FilterRow> = match only {
            Some(i) => vec![rows
                .get(i)
                .ok_or_else(|| AppError::validation("That filter row no longer exists."))?],
            None => rows.iter().filter(|r| r.enabled).collect(),
        };
        let mut parts = Vec::new();
        for row in selected {
            if let Some(cond) = self.row(row)? {
                parts.push(format!("({cond})"));
            }
        }
        Ok(FilterSql {
            sql: parts.join(" AND "),
            binds: self.binds,
        })
    }

    fn row(&mut self, row: &FilterRow) -> AppResult<Option<String>> {
        if row.column == RAW_SQL {
            return raw_sql(&row.value);
        }
        let op = Op::parse(&row.operator).ok_or_else(|| {
            AppError::validation(format!("Unknown filter operator \"{}\".", row.operator))
        })?;
        if !supported(self.driver, op) {
            return Err(AppError::validation(format!(
                "The operator \"{}\" is not available for {}.",
                row.operator,
                self.driver.label()
            )));
        }
        if row.column == ANY_COLUMN {
            return self.any_column(op, row).map(Some);
        }
        let ty = *self
            .types
            .get(row.column.as_str())
            .ok_or_else(|| AppError::validation(format!("Unknown column \"{}\".", row.column)))?;
        let col = quote_ident(self.driver, &row.column);
        self.condition(&col, Some(ty), op, row, false).map(Some)
    }

    /// Any column: positive operators match when any column matches (OR); negative
    /// operators match when no column matches: `NOT (c1 … OR c2 …)`.
    fn any_column(&mut self, op: Op, row: &FilterRow) -> AppResult<String> {
        if self.columns.is_empty() {
            return Err(AppError::validation("This table has no columns to search."));
        }
        let positive = op.positive();
        let negative = op.is_negative();
        let mut ors = Vec::new();
        for (name, _) in self.columns {
            let col = quote_ident(self.driver, name);
            let cond = self.condition(&col, None, positive, row, true)?;
            // NULL columns make `c LIKE p` NULL, and NOT (… OR NULL) is NULL, which would
            // drop the row. For "no column matches", a NULL column counts as not matching.
            ors.push(if negative {
                format!("COALESCE({cond}, FALSE)")
            } else {
                cond
            });
        }
        let joined = ors.join(" OR ");
        Ok(if negative {
            format!("NOT ({joined})")
        } else {
            joined
        })
    }

    /// The column as text, for operators that compare text (LIKE family, Any column).
    fn text_expr(&self, col: &str) -> String {
        match self.driver {
            Dialect::Postgres => format!("{col}::text"),
            Dialect::Mysql => col.to_string(),
            Dialect::Sqlite => col.to_string(),
        }
    }

    /// A bind placeholder cast to the column type on Postgres (binds are sent as text).
    fn param(&mut self, value: String, ty: Option<&str>) -> String {
        self.binds.push(value);
        let p = self.ph.next_param();
        match (self.driver, ty) {
            (Dialect::Postgres, Some(t)) if !t.is_empty() => format!("{p}::{t}"),
            _ => p,
        }
    }

    fn like_param(&mut self, pattern: String) -> String {
        self.binds.push(pattern);
        let p = self.ph.next_param();
        format!("{p} ESCAPE '{LIKE_ESCAPE}'")
    }

    /// `as_text`: compare the column as text (Any column, where types differ per column).
    fn condition(
        &mut self,
        col: &str,
        ty: Option<&str>,
        op: Op,
        row: &FilterRow,
        as_text: bool,
    ) -> AppResult<String> {
        // Any column compares as text. On MySQL a bare `int_col = 'abc'` would convert
        // 'abc' to 0 and match every zero, so cast explicitly there.
        let text = if as_text && self.driver == Dialect::Mysql {
            format!("CAST({col} AS CHAR)")
        } else {
            self.text_expr(col)
        };
        let col: &str = if as_text { &text } else { col };
        Ok(match op {
            Op::Eq | Op::Neq | Op::Lt | Op::Gt | Op::Lte | Op::Gte => {
                let p = self.param(row.value.clone(), ty);
                format!("{col} {} {p}", comparison(op))
            }
            Op::In | Op::NotIn => self.in_list(col, op, &row.value, ty)?,
            Op::IsNull => format!("{col} IS NULL"),
            Op::IsNotNull => format!("{col} IS NOT NULL"),
            Op::Between | Op::NotBetween => self.between(col, op, row, ty)?,
            Op::Like => {
                let p = self.param(need_value(&row.value)?, None);
                format!("{text} LIKE {p}")
            }
            Op::ILike => {
                let p = self.param(need_value(&row.value)?, None);
                format!("{text} ILIKE {p}")
            }
            Op::Contains(case)
            | Op::NotContains(case)
            | Op::HasPrefix(case)
            | Op::HasSuffix(case) => self.pattern(&text, op, case, &row.value)?,
        })
    }

    fn in_list(&mut self, col: &str, op: Op, value: &str, ty: Option<&str>) -> AppResult<String> {
        let items = parse_list(value);
        if items.is_empty() {
            return Err(AppError::validation(
                "Enter at least one value, separated by commas.",
            ));
        }
        let ps: Vec<String> = items.into_iter().map(|v| self.param(v, ty)).collect();
        let kw = if op == Op::In { "IN" } else { "NOT IN" };
        Ok(format!("{col} {kw} ({})", ps.join(", ")))
    }

    fn between(
        &mut self,
        col: &str,
        op: Op,
        row: &FilterRow,
        ty: Option<&str>,
    ) -> AppResult<String> {
        let lo = need_value(&row.value)?;
        let hi = need_value(&row.value2)
            .map_err(|_| AppError::validation("Enter both bounds for BETWEEN."))?;
        let a = self.param(lo, ty);
        let b = self.param(hi, ty);
        let kw = if op == Op::Between {
            "BETWEEN"
        } else {
            "NOT BETWEEN"
        };
        Ok(format!("{col} {kw} {a} AND {b}"))
    }

    /// Contains / Not contains / Has prefix / Has suffix, matched literally.
    fn pattern(&mut self, text: &str, op: Op, case: Case, value: &str) -> AppResult<String> {
        let v = escape_like(&need_value(value)?);
        let pattern = match op {
            Op::HasPrefix(_) => format!("{v}%"),
            Op::HasSuffix(_) => format!("%{v}"),
            _ => format!("%{v}%"),
        };
        let not = if matches!(op, Op::NotContains(_)) {
            "NOT "
        } else {
            ""
        };
        let p = self.like_param(pattern);
        Ok(match (self.driver, case) {
            (Dialect::Postgres, Case::Insensitive) => format!("{text} {not}ILIKE {p}"),
            (Dialect::Mysql, Case::Sensitive) => format!("CAST({text} AS BINARY) {not}LIKE {p}"),
            _ => format!("{text} {not}LIKE {p}"),
        })
    }
}

fn comparison(op: Op) -> &'static str {
    match op {
        Op::Eq => "=",
        Op::Neq => "<>",
        Op::Lt => "<",
        Op::Gt => ">",
        Op::Lte => "<=",
        _ => ">=",
    }
}

/// A Raw SQL row is used as typed, but it can't end the statement.
fn raw_sql(value: &str) -> AppResult<Option<String>> {
    let raw = value.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.contains(';') {
        return Err(AppError::validation(
            "A Raw SQL filter can't contain \";\".",
        ));
    }
    Ok(Some(raw.to_string()))
}

/// ORDER BY fragment (without the keywords) for validated column names (GRD-3).
pub fn order_by(
    driver: Dialect,
    columns: &[(String, String)],
    sort: &[SortSpec],
) -> AppResult<String> {
    let mut parts = Vec::new();
    for s in sort {
        if !columns.iter().any(|(n, _)| n == &s.column) {
            return Err(AppError::validation(format!(
                "Unknown sort column \"{}\".",
                s.column
            )));
        }
        parts.push(format!(
            "{} {}",
            quote_ident(driver, &s.column),
            if s.desc { "DESC" } else { "ASC" }
        ));
    }
    Ok(parts.join(", "))
}

/// Renders a fragment with its binds inlined as quoted literals. For the preview only;
/// never executed.
pub fn inline_preview(driver: Dialect, fsql: &FilterSql) -> String {
    let binds: Vec<Option<String>> = fsql.binds.iter().cloned().map(Some).collect();
    crate::db::sql::inline_binds(driver, &fsql.sql, &binds)
}
