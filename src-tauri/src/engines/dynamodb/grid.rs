//! Grid pages as Scan (or Query, for `=` on the partition key) with Limit and
//! ExclusiveStartKey. The start key of every page reached is cached per table, filter
//! and page size, so page N+1 continues where page N ended instead of rescanning.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{json, Map, Value};

use super::client::Client;
use super::filter::{build, Expr};
use super::values::to_json;
use crate::db::data::{PageRequest, COUNT_CAP};
use crate::db::filter::SortSpec;
use crate::db::meta::ColumnInfo;
use crate::error::{AppError, AppResult};

/// Where a page starts.
#[derive(Clone)]
enum Start {
    Begin,
    After(Value),
    End,
}

#[derive(Default)]
pub struct PageCache {
    starts: Mutex<HashMap<String, Vec<Start>>>,
}

impl PageCache {
    fn get(&self, sig: &str) -> Vec<Start> {
        self.starts
            .lock()
            .unwrap()
            .get(sig)
            .cloned()
            .unwrap_or_else(|| vec![Start::Begin])
    }

    fn put(&self, sig: &str, page: usize, next: Start) {
        let mut all = self.starts.lock().unwrap();
        let v = all
            .entry(sig.to_string())
            .or_insert_with(|| vec![Start::Begin]);
        if v.len() == page + 1 {
            v.push(next);
        }
    }

    /// Forgets a table's pages (after edits).
    pub fn forget(&self, table: &str) {
        let prefix = format!("{table}\u{0}");
        self.starts
            .lock()
            .unwrap()
            .retain(|k, _| !k.starts_with(&prefix));
    }
}

/// The request a page reads with: Scan or Query, and its expressions.
pub struct Plan {
    pub action: &'static str,
    pub body: Map<String, Value>,
}

pub fn plan(table: &str, cols: &[ColumnInfo], req: &PageRequest, ops: &[&str]) -> AppResult<Plan> {
    let e: Expr = build(cols, &req.filters, req.apply_only, ops, true)?;
    let mut body = Map::new();
    body.insert("TableName".into(), table.into());
    e.apply(&mut body);
    let action = if e.key.is_empty() { "Scan" } else { "Query" };
    Ok(Plan { action, body })
}

/// The preview line of a plan.
pub fn describe(p: &Plan, limit: u32, sorted: bool) -> String {
    let mut body = p.body.clone();
    body.insert("Limit".into(), limit.into());
    let note = if sorted {
        "  -- sorted within this page"
    } else {
        ""
    };
    format!("{} {}{note}", p.action, Value::Object(body))
}

/// The key attributes of an item, which is a valid ExclusiveStartKey for the table.
fn key_of(item: &Value, cols: &[ColumnInfo]) -> Value {
    let m: Map<String, Value> = cols
        .iter()
        .filter(|c| c.pk_position.is_some())
        .filter_map(|c| item.get(&c.name).map(|v| (c.name.clone(), v.clone())))
        .collect();
    Value::Object(m)
}

/// Reads `size` items from `start`; returns them and where the next page starts.
async fn read(
    c: &Client,
    p: &Plan,
    cols: &[ColumnInfo],
    start: Start,
    size: usize,
) -> AppResult<(Vec<Value>, Start)> {
    let mut items: Vec<Value> = Vec::new();
    let mut from = match start {
        Start::Begin => None,
        Start::After(k) => Some(k),
        Start::End => return Ok((items, Start::End)),
    };
    loop {
        let mut body = p.body.clone();
        body.insert("Limit".into(), size.into());
        if let Some(k) = from.take() {
            body.insert("ExclusiveStartKey".into(), k);
        }
        let page = c.call(p.action, &Value::Object(body)).await?;
        if let Some(got) = page.get("Items").and_then(Value::as_array) {
            items.extend(got.iter().cloned());
        }
        let last = page
            .get("LastEvaluatedKey")
            .filter(|v| !v.is_null())
            .cloned();
        if items.len() > size {
            items.truncate(size);
            let next = key_of(items.last().unwrap_or(&Value::Null), cols);
            return Ok((items, Start::After(next)));
        }
        match last {
            None => return Ok((items, Start::End)),
            Some(k) if items.len() == size => return Ok((items, Start::After(k))),
            Some(k) => from = Some(k),
        }
    }
}

/// Page `page` of `size` items, walking from the nearest cached start.
pub async fn page(
    c: &Client,
    cache: &PageCache,
    p: &Plan,
    cols: &[ColumnInfo],
    page: usize,
    size: usize,
) -> AppResult<Vec<Value>> {
    let sig = format!(
        "{}\u{0}{}\u{0}{size}",
        p.body
            .get("TableName")
            .and_then(Value::as_str)
            .unwrap_or(""),
        Value::Object(p.body.clone())
    );
    let starts = cache.get(&sig);
    let mut k = page.min(starts.len() - 1);
    let mut start = starts[k].clone();
    loop {
        if matches!(start, Start::End) {
            return Ok(Vec::new());
        }
        let (items, next) = read(c, p, cols, start, size).await?;
        cache.put(&sig, k, next.clone());
        if k == page {
            return Ok(items);
        }
        start = next;
        k += 1;
    }
}

/// Items as grid rows in column order.
pub fn rows(items: &[Value], cols: &[ColumnInfo], limit: usize) -> Vec<Vec<Value>> {
    items
        .iter()
        .map(|item| {
            cols.iter()
                .map(|c| {
                    item.get(&c.name)
                        .map(|v| to_json(v, limit))
                        .unwrap_or(Value::Null)
                })
                .collect()
        })
        .collect()
}

fn compare(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Greater,
        (_, Value::Null) => Ordering::Less,
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(0.0), y.as_f64().unwrap_or(0.0));
            x.partial_cmp(&y).unwrap_or(Ordering::Equal)
        }
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (x, y) => x.to_string().cmp(&y.to_string()),
    }
}

/// Sorts one page by the requested columns (a scan has no order of its own).
pub fn sort_page(rows: &mut [Vec<Value>], cols: &[ColumnInfo], sort: &[SortSpec]) -> AppResult<()> {
    let mut keys = Vec::new();
    for s in sort {
        let i = cols
            .iter()
            .position(|c| c.name == s.column)
            .ok_or_else(|| {
                AppError::validation(format!("Unknown sort column \"{}\".", s.column))
            })?;
        keys.push((i, s.desc));
    }
    rows.sort_by(|a, b| {
        for &(i, desc) in &keys {
            let o = compare(&a[i], &b[i]);
            let o = if desc { o.reverse() } else { o };
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    });
    Ok(())
}

/// Counts matching items with `Select: COUNT`, stopping past COUNT_CAP scanned items.
pub async fn count(c: &Client, p: &Plan) -> AppResult<(i64, bool)> {
    let mut total = 0i64;
    let mut scanned = 0i64;
    let mut from: Option<Value> = None;
    loop {
        let mut body = p.body.clone();
        body.insert("Select".into(), json!("COUNT"));
        if let Some(k) = from.take() {
            body.insert("ExclusiveStartKey".into(), k);
        }
        let page = c.call(p.action, &Value::Object(body)).await?;
        total += page.get("Count").and_then(Value::as_i64).unwrap_or(0);
        scanned += page
            .get("ScannedCount")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        match page.get("LastEvaluatedKey").filter(|v| !v.is_null()) {
            Some(_) if scanned > COUNT_CAP => return Ok((total, false)),
            Some(k) => from = Some(k.clone()),
            None => return Ok((total, true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_sort_with_nulls_last() {
        let cols = vec![ColumnInfo {
            name: "n".into(),
            data_type: "N".into(),
            nullable: true,
            default: None,
            pk_position: None,
            auto_increment: false,
            comment: None,
        }];
        let mut r = vec![vec![json!(2)], vec![Value::Null], vec![json!(10)]];
        let spec = [SortSpec {
            column: "n".into(),
            desc: false,
        }];
        sort_page(&mut r, &cols, &spec).unwrap();
        assert_eq!(r, vec![vec![json!(2)], vec![json!(10)], vec![Value::Null]]);
        let item = json!({"n": {"N": "1"}, "x": {"S": "a"}});
        assert_eq!(rows(&[item], &cols, 10), vec![vec![json!(1)]]);
    }
}
