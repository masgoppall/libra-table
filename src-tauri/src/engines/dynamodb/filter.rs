//! Filter rows as DynamoDB expressions: attribute names go through
//! `ExpressionAttributeNames` (#n0…), values through `ExpressionAttributeValues`
//! (:v0…), typed by the column. An `=` row on the partition key becomes the
//! KeyConditionExpression of a Query instead of a Scan filter.

use serde_json::{Map, Value};

use super::values::typed_text;
use crate::db::filter::{FilterRow, Op, ANY_COLUMN, RAW_SQL};
use crate::db::meta::ColumnInfo;
use crate::error::{AppError, AppResult};

/// Expression text plus the names and values it refers to.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Expr {
    pub key: String,
    pub filter: String,
    pub names: Map<String, Value>,
    pub values: Map<String, Value>,
}

impl Expr {
    pub fn name(&mut self, attr: &str) -> String {
        if let Some((k, _)) = self.names.iter().find(|(_, v)| v.as_str() == Some(attr)) {
            return k.clone();
        }
        let k = format!("#n{}", self.names.len());
        self.names
            .insert(k.clone(), Value::String(attr.to_string()));
        k
    }

    pub fn value(&mut self, av: Value) -> String {
        let k = format!(":v{}", self.values.len());
        self.values.insert(k.clone(), av);
        k
    }

    /// The request fields (`KeyConditionExpression`, `FilterExpression`, names, values).
    pub fn apply(&self, body: &mut Map<String, Value>) {
        if !self.key.is_empty() {
            body.insert("KeyConditionExpression".into(), self.key.clone().into());
        }
        if !self.filter.is_empty() {
            body.insert("FilterExpression".into(), self.filter.clone().into());
        }
        if !self.names.is_empty() {
            body.insert("ExpressionAttributeNames".into(), self.names.clone().into());
        }
        if !self.values.is_empty() {
            body.insert(
                "ExpressionAttributeValues".into(),
                self.values.clone().into(),
            );
        }
    }
}

fn column<'a>(cols: &'a [ColumnInfo], name: &str) -> AppResult<&'a ColumnInfo> {
    cols.iter()
        .find(|c| c.name == name)
        .ok_or_else(|| AppError::validation(format!("Unknown column \"{name}\".")))
}

/// The attribute type to send a value as: the column's own, else N for numbers.
fn value_type<'a>(c: &'a ColumnInfo, text: &str) -> &'a str {
    match c.data_type.as_str() {
        "S" | "N" | "B" | "BOOL" => &c.data_type,
        "NS" => "N",
        _ if text.trim().parse::<f64>().is_ok_and(f64::is_finite) => "N",
        _ => "S",
    }
}

/// Sort-key operators a KeyConditionExpression accepts.
const SORT_KEY_OPS: &[&str] = &["eq", "lt", "gt", "lte", "gte", "has_prefix"];

/// The rows a Query can take as its key condition: one `=` on the partition key and at
/// most one key-friendly row on the sort key. None when the rows need a Scan (a Query's
/// filter can't mention key attributes).
fn key_rows(cols: &[ColumnInfo], selected: &[&FilterRow]) -> Option<Vec<usize>> {
    let hash = cols.iter().find(|c| c.pk_position == Some(1))?;
    let range = cols.iter().find(|c| c.pk_position == Some(2));
    let (mut out, mut hashes, mut ranges) = (Vec::new(), 0, 0);
    for (i, row) in selected.iter().enumerate() {
        if row.column == ANY_COLUMN {
            return None;
        }
        if row.column == hash.name {
            if row.operator != "eq" {
                return None;
            }
            hashes += 1;
            out.push(i);
        } else if range.is_some_and(|r| r.name == row.column) {
            if !SORT_KEY_OPS.contains(&row.operator.as_str()) {
                return None;
            }
            ranges += 1;
            out.push(i);
        }
    }
    (hashes == 1 && ranges <= 1).then_some(out)
}

/// Builds the expressions for the enabled rows (or only row `only`). With `key_query`,
/// rows on the key become a Query's key condition when they can.
pub fn build(
    cols: &[ColumnInfo],
    rows: &[FilterRow],
    only: Option<usize>,
    ops: &[&str],
    key_query: bool,
) -> AppResult<Expr> {
    let selected: Vec<&FilterRow> = match only {
        Some(i) => vec![rows
            .get(i)
            .ok_or_else(|| AppError::validation("That filter row no longer exists."))?],
        None => rows.iter().filter(|r| r.enabled).collect(),
    };
    let key = if key_query {
        key_rows(cols, &selected).unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut e = Expr::default();
    let (mut keys, mut parts) = (Vec::new(), Vec::new());
    for (i, row) in selected.iter().enumerate() {
        if row.column == RAW_SQL {
            if !row.value.trim().is_empty() {
                parts.push(format!("({})", row.value.trim()));
            }
            continue;
        }
        let op = Op::parse(&row.operator)
            .filter(|_| ops.contains(&row.operator.as_str()))
            .ok_or_else(|| {
                AppError::validation(format!(
                    "The operator \"{}\" is not available for DynamoDB.",
                    row.operator
                ))
            })?;
        let cond = if row.column == ANY_COLUMN {
            any_column(&mut e, cols, op, &row.value)?
        } else {
            condition(&mut e, column(cols, &row.column)?, op, &row.value)?
        };
        if key.contains(&i) {
            keys.push(cond);
        } else {
            parts.push(format!("({cond})"));
        }
    }
    e.key = keys.join(" AND ");
    e.filter = parts.join(" AND ");
    Ok(e)
}

/// One condition. The value is encoded before any name is added, so a failure leaves
/// no unused names or values behind (DynamoDB rejects those).
fn condition(e: &mut Expr, c: &ColumnInfo, op: Op, text: &str) -> AppResult<String> {
    let sym = match op {
        Op::Eq => "=",
        Op::Neq => "<>",
        Op::Lt => "<",
        Op::Gt => ">",
        Op::Lte => "<=",
        Op::Gte => ">=",
        Op::Contains(_) | Op::NotContains(_) | Op::HasPrefix(_) if !text.is_empty() => "",
        Op::Contains(_) | Op::NotContains(_) | Op::HasPrefix(_) => {
            return Err(AppError::validation("Enter a value for this filter."))
        }
        _ => {
            return Err(AppError::validation(
                "That operator is not available for DynamoDB.",
            ))
        }
    };
    let av = typed_text(text, send_as(c, op, text))?;
    let n = e.name(&c.name);
    let v = e.value(av);
    Ok(match op {
        Op::Contains(_) => format!("contains({n}, {v})"),
        Op::NotContains(_) => format!("NOT contains({n}, {v})"),
        Op::HasPrefix(_) => format!("begins_with({n}, {v})"),
        _ => format!("{n} {sym} {v}"),
    })
}

/// begins_with needs a string; contains a string, or a number for number sets.
fn send_as<'a>(c: &'a ColumnInfo, op: Op, text: &str) -> &'a str {
    match op {
        Op::HasPrefix(_) => "S",
        Op::Contains(_) | Op::NotContains(_) if c.data_type == "NS" => "N",
        Op::Contains(_) | Op::NotContains(_) => "S",
        _ => value_type(c, text),
    }
}

/// Any column: positive operators match when one attribute matches, negative ones
/// (`<>`, Not contains) when none matches the positive form. Columns the value can't
/// be typed for (text against a number column) are left out.
fn any_column(e: &mut Expr, cols: &[ColumnInfo], op: Op, text: &str) -> AppResult<String> {
    let first = cols
        .first()
        .ok_or_else(|| AppError::validation("This table has no columns to search."))?;
    let (positive, negate) = match op {
        Op::Neq => (Op::Eq, true),
        Op::NotContains(c) => (Op::Contains(c), true),
        other => (other, false),
    };
    let mut ors = Vec::new();
    for c in cols {
        if typed_text(text, send_as(c, positive, text)).is_ok() {
            ors.push(condition(e, c, positive, text)?);
        }
    }
    if ors.is_empty() {
        let n = e.name(&first.name);
        return Ok(if negate {
            format!("attribute_exists({n}) OR attribute_not_exists({n})")
        } else {
            format!("attribute_exists({n}) AND attribute_not_exists({n})")
        });
    }
    let joined = ors.join(" OR ");
    Ok(if negate {
        format!("NOT ({joined})")
    } else {
        joined
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn col(name: &str, ty: &str, pk: Option<i32>) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            data_type: ty.into(),
            nullable: pk.is_none(),
            default: None,
            pk_position: pk,
            auto_increment: false,
            comment: None,
        }
    }

    fn row(column: &str, op: &str, value: &str) -> FilterRow {
        FilterRow {
            enabled: true,
            column: column.into(),
            operator: op.into(),
            value: value.into(),
            value2: String::new(),
        }
    }

    const OPS: &[&str] = &["eq", "neq", "gt", "contains", "not_contains", "has_prefix"];

    #[test]
    fn key_condition_and_filters() {
        let cols = [
            col("pk", "S", Some(1)),
            col("age", "N", None),
            col("tags", "SS", None),
        ];
        let rows = [
            row("pk", "eq", "u#1"),
            row("age", "gt", "30"),
            row("tags", "contains", "red"),
        ];
        let e = build(&cols, &rows, None, OPS, true).unwrap();
        assert_eq!(e.key, "#n0 = :v0");
        assert_eq!(e.filter, "(#n1 > :v1) AND (contains(#n2, :v2))");
        assert_eq!(e.names["#n2"], json!("tags"));
        assert_eq!(e.values[":v1"], json!({"N": "30"}));
        assert_eq!(e.values[":v2"], json!({"S": "red"}));
        let scan = build(&cols, &rows, Some(0), OPS, false).unwrap();
        assert_eq!(
            (scan.key.as_str(), scan.filter.as_str()),
            ("", "(#n0 = :v0)")
        );
        assert!(build(&cols, &[row("age", "gt", "x")], None, OPS, false).is_err());
        assert!(build(&cols, &[row("age", "in", "1")], None, OPS, false).is_err());
        let cols = [col("pk", "S", Some(1)), col("sk", "N", Some(2))];
        let rows = [row("pk", "eq", "a"), row("sk", "gt", "3")];
        let e = build(&cols, &rows, None, OPS, true).unwrap();
        assert_eq!(
            (e.key.as_str(), e.filter.as_str()),
            ("#n0 = :v0 AND #n1 > :v1", "")
        );
        let rows = [row("pk", "eq", "a"), row("sk", "neq", "3")];
        let e = build(&cols, &rows, None, OPS, true).unwrap();
        assert_eq!(
            (e.key.as_str(), e.filter.as_str()),
            ("", "(#n0 = :v0) AND (#n1 <> :v1)")
        );
    }

    #[test]
    fn any_column_skips_mismatched_types() {
        let cols = [col("pk", "S", Some(1)), col("age", "N", None)];
        let e = build(
            &cols,
            &[row(ANY_COLUMN, "not_contains", "bo")],
            None,
            OPS,
            false,
        )
        .unwrap();
        assert_eq!(e.filter, "(NOT (contains(#n0, :v0) OR contains(#n1, :v1)))");
        let e = build(&cols, &[row(ANY_COLUMN, "eq", "7")], None, OPS, false).unwrap();
        assert_eq!(e.filter, "(#n0 = :v0 OR #n1 = :v1)");
        assert_eq!(e.values[":v0"], json!({"S": "7"}));
        assert_eq!(e.values[":v1"], json!({"N": "7"}));
    }
}
