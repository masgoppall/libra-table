//! Grid SQL for the HTTP SQL services (Snowflake, BigQuery): filter rows, sort, paging,
//! and counts (edits are in `edits`). Values become bind parameters; a `Binds::inline`
//! pass renders the same statement with literals for the previews (never executed).

use crate::db::data::{PageRequest, COUNT_CAP};
use crate::db::filter::{parse_list, Case, FilterRow, Op, SortSpec, ANY_COLUMN, RAW_SQL};
use crate::db::meta::ColumnInfo;
use crate::error::{AppError, AppResult};

/// How one service quotes, binds, converts and matches.
pub trait Flavor: Sync {
    fn quote(&self, name: &str) -> String;
    /// The table's qualified name.
    fn table(&self, schema: &str, table: &str) -> String;
    /// A string literal (previews only).
    fn literal(&self, value: &str) -> String;
    /// Marker of the `n`-th bind (1-based).
    fn marker(&self, n: usize) -> String;
    /// Text SQL (a marker or literal) converted to the column type.
    fn typed(&self, text: String, ty: &str) -> String;
    /// The column as text, for the text operators and Any column.
    fn as_text(&self, col: &str, ty: &str) -> String;
    /// `text [NOT] LIKE pattern` for an escaped pattern.
    fn like(&self, text: &str, pattern: String, not: bool, case: Case) -> String;
    /// Escapes LIKE wildcards so the value matches literally.
    fn escape_like(&self, value: &str) -> String;
    /// False for nested types (compared as text, not editable).
    fn scalar(&self, ty: &str) -> bool;
    /// Whether `UPDATE … SET c = DEFAULT` exists.
    fn update_default(&self) -> bool;
    /// `VALUES (…)` or `SELECT …` for an INSERT of these value expressions.
    fn insert_values(&self, values: &[String]) -> String {
        format!("VALUES ({})", values.join(", "))
    }
}

/// Bind values in order, or literals for a preview.
#[derive(Default)]
pub struct Binds {
    pub values: Vec<String>,
    inline: bool,
}

impl Binds {
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders values as literals instead of binding them.
    pub fn inline() -> Self {
        Self {
            values: Vec::new(),
            inline: true,
        }
    }

    /// A text value: the next marker, or a literal in previews.
    pub fn push(&mut self, f: &dyn Flavor, value: String) -> String {
        if self.inline {
            return f.literal(&value);
        }
        self.values.push(value);
        f.marker(self.values.len())
    }
}

fn need_value(v: &str) -> AppResult<String> {
    if v.is_empty() {
        Err(AppError::validation("Enter a value for this filter."))
    } else {
        Ok(v.to_string())
    }
}

pub(super) fn column<'a>(cols: &'a [ColumnInfo], name: &str) -> AppResult<&'a ColumnInfo> {
    cols.iter()
        .find(|c| c.name == name)
        .ok_or_else(|| AppError::validation(format!("Unknown column \"{name}\".")))
}

/// The WHERE fragment (without `WHERE`) of the enabled rows, or of row `only`.
/// `ops` lists the operator ids the driver offers.
pub fn filter(
    f: &dyn Flavor,
    cols: &[ColumnInfo],
    rows: &[FilterRow],
    only: Option<usize>,
    ops: &[&str],
    b: &mut Binds,
) -> AppResult<String> {
    let selected: Vec<&FilterRow> = match only {
        Some(i) => vec![rows
            .get(i)
            .ok_or_else(|| AppError::validation("That filter row no longer exists."))?],
        None => rows.iter().filter(|r| r.enabled).collect(),
    };
    let mut parts = Vec::new();
    for row in selected {
        if let Some(cond) = filter_row(f, cols, row, ops, b)? {
            parts.push(format!("({cond})"));
        }
    }
    Ok(parts.join(" AND "))
}

fn filter_row(
    f: &dyn Flavor,
    cols: &[ColumnInfo],
    row: &FilterRow,
    ops: &[&str],
    b: &mut Binds,
) -> AppResult<Option<String>> {
    if row.column == RAW_SQL {
        return raw_sql(&row.value);
    }
    let op = Op::parse(&row.operator)
        .filter(|_| ops.contains(&row.operator.as_str()))
        .ok_or_else(|| {
            AppError::validation(format!(
                "The operator \"{}\" is not available here.",
                row.operator
            ))
        })?;
    if row.column == ANY_COLUMN {
        return any_column(f, cols, op, row, b).map(Some);
    }
    let c = column(cols, &row.column)?;
    let as_text = !f.scalar(&c.data_type);
    condition(f, c, op, row, as_text, b).map(Some)
}

fn negative(op: Op) -> Option<Op> {
    Some(match op {
        Op::Neq => Op::Eq,
        Op::NotIn => Op::In,
        Op::NotBetween => Op::Between,
        Op::NotContains(c) => Op::Contains(c),
        Op::IsNotNull => Op::IsNull,
        _ => return None,
    })
}

/// Positive operators match when any column matches; negative ones when none does
/// (a NULL column counts as not matching).
fn any_column(
    f: &dyn Flavor,
    cols: &[ColumnInfo],
    op: Op,
    row: &FilterRow,
    b: &mut Binds,
) -> AppResult<String> {
    if cols.is_empty() {
        return Err(AppError::validation("This table has no columns to search."));
    }
    let positive = negative(op);
    let mut ors = Vec::new();
    for c in cols {
        let cond = condition(f, c, positive.unwrap_or(op), row, true, b)?;
        ors.push(match positive {
            Some(_) => format!("COALESCE({cond}, FALSE)"),
            None => cond,
        });
    }
    let joined = ors.join(" OR ");
    Ok(match positive {
        Some(_) => format!("NOT ({joined})"),
        None => joined,
    })
}

fn condition(
    f: &dyn Flavor,
    c: &ColumnInfo,
    op: Op,
    row: &FilterRow,
    as_text: bool,
    b: &mut Binds,
) -> AppResult<String> {
    let quoted = f.quote(&c.name);
    let text = f.as_text(&quoted, &c.data_type);
    let (col, ty) = if as_text {
        (text.as_str(), "")
    } else {
        (quoted.as_str(), c.data_type.as_str())
    };
    let value = |v: String, b: &mut Binds| {
        let m = b.push(f, v);
        if ty.is_empty() {
            m
        } else {
            f.typed(m, ty)
        }
    };
    Ok(match op {
        Op::Eq | Op::Neq | Op::Lt | Op::Gt | Op::Lte | Op::Gte => {
            format!("{col} {} {}", comparison(op), value(row.value.clone(), b))
        }
        Op::In | Op::NotIn => {
            let items = parse_list(&row.value);
            if items.is_empty() {
                return Err(AppError::validation(
                    "Enter at least one value, separated by commas.",
                ));
            }
            let vs: Vec<String> = items.into_iter().map(|v| value(v, b)).collect();
            let kw = if op == Op::In { "IN" } else { "NOT IN" };
            format!("{col} {kw} ({})", vs.join(", "))
        }
        Op::IsNull => format!("{quoted} IS NULL"),
        Op::IsNotNull => format!("{quoted} IS NOT NULL"),
        Op::Between | Op::NotBetween => {
            let lo = need_value(&row.value)?;
            let hi = need_value(&row.value2)
                .map_err(|_| AppError::validation("Enter both bounds for BETWEEN."))?;
            let kw = if op == Op::Between {
                "BETWEEN"
            } else {
                "NOT BETWEEN"
            };
            let (a, z) = (value(lo, b), value(hi, b));
            format!("{col} {kw} {a} AND {z}")
        }
        Op::Contains(case) | Op::NotContains(case) | Op::HasPrefix(case) | Op::HasSuffix(case) => {
            let v = f.escape_like(&need_value(&row.value)?);
            let pattern = match op {
                Op::HasPrefix(_) => format!("{v}%"),
                Op::HasSuffix(_) => format!("%{v}"),
                _ => format!("%{v}%"),
            };
            let not = matches!(op, Op::NotContains(_));
            let p = b.push(f, pattern);
            f.like(&text, p, not, case)
        }
        Op::Like | Op::ILike => {
            return Err(AppError::validation(
                "Use Contains, Has prefix or Has suffix here.",
            ))
        }
    })
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

/// ORDER BY fragment (without the keywords) for known columns.
pub fn order_by(f: &dyn Flavor, cols: &[ColumnInfo], sort: &[SortSpec]) -> AppResult<String> {
    let mut parts = Vec::new();
    for s in sort {
        let c = column(cols, &s.column)
            .map_err(|_| AppError::validation(format!("Unknown sort column \"{}\".", s.column)))?;
        let dir = if s.desc { "DESC" } else { "ASC" };
        parts.push(format!("{} {dir}", f.quote(&c.name)));
    }
    Ok(parts.join(", "))
}

/// The grid's SELECT of one page (`page` = (size, offset)) or of every row.
pub fn select(
    f: &dyn Flavor,
    cols: &[ColumnInfo],
    req: &PageRequest,
    ops: &[&str],
    page: Option<(u32, u64)>,
    b: &mut Binds,
) -> AppResult<String> {
    let list: Vec<String> = cols.iter().map(|c| f.quote(&c.name)).collect();
    let mut sql = format!(
        "SELECT {} FROM {}",
        list.join(", "),
        f.table(&req.schema, &req.table)
    );
    let cond = filter(f, cols, &req.filters, req.apply_only, ops, b)?;
    if !cond.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&cond);
    }
    let order = order_by(f, cols, &req.sort)?;
    if !order.is_empty() {
        sql.push_str(" ORDER BY ");
        sql.push_str(&order);
    }
    if let Some((size, offset)) = page {
        sql.push_str(&format!(" LIMIT {size} OFFSET {offset}"));
    }
    Ok(sql)
}

/// `COUNT(*)` of the filtered rows, stopping just past `COUNT_CAP`; None without filters.
pub fn capped_count(
    f: &dyn Flavor,
    cols: &[ColumnInfo],
    req: &PageRequest,
    ops: &[&str],
    b: &mut Binds,
) -> AppResult<Option<String>> {
    let cond = filter(f, cols, &req.filters, req.apply_only, ops, b)?;
    if cond.is_empty() {
        return Ok(None);
    }
    let table = f.table(&req.schema, &req.table);
    Ok(Some(format!(
        "SELECT COUNT(*) FROM (SELECT 1 FROM {table} WHERE {cond} LIMIT {})",
        COUNT_CAP + 1
    )))
}
