//! Grid edits as plans. Grid rows are keys: rename a key (`key`), change its TTL (`ttl`),
//! set a string (`value`), insert a string key, delete keys. The value panel's edits
//! are in `key_edits`.

use serde_json::{Map, Value};

use crate::db::changes::Change;
use crate::error::{AppError, AppResult};

use super::plan::{Check, Plan};
use super::values::key_bytes;

/// A value cell or argument as bytes; NULL isn't a Redis value.
pub(super) fn arg(v: &Value, what: &str) -> AppResult<Vec<u8>> {
    match v {
        Value::Null => Err(AppError::validation(format!(
            "Redis has no NULL; give the {what} a value."
        ))),
        Value::Bool(b) => Ok(b.to_string().into_bytes()),
        other => key_bytes(other),
    }
}

fn key_of(pk: &Map<String, Value>) -> AppResult<Vec<u8>> {
    let k = pk
        .get("key")
        .ok_or_else(|| AppError::validation("The row has no key."))?;
    arg(k, "key")
}

/// TTL seconds from a cell: a positive number sets it, empty/NULL/≤0 removes it.
fn ttl_of(v: &Value) -> AppResult<Option<i64>> {
    let n = match v {
        Value::Null => return Ok(None),
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) if s.trim().is_empty() => return Ok(None),
        Value::String(s) => s.trim().parse::<f64>().map_err(|_| bad_ttl())?,
        _ => return Err(bad_ttl()),
    };
    Ok((n >= 1.0).then_some(n.round() as i64))
}

fn bad_ttl() -> AppError {
    AppError::validation("The TTL is a number of seconds (-1 or empty for no expiry).")
}

pub(super) fn set_ttl(plan: &mut Plan, key: &[u8], ttl: Option<i64>) {
    match ttl {
        Some(s) => plan.push([b"EXPIRE".as_slice(), key, s.to_string().as_bytes()]),
        None => plan.push([b"PERSIST".as_slice(), key]),
    }
}

/// Grid edits as one plan (one transaction).
pub fn grid_plan(changes: &[Change]) -> AppResult<Plan> {
    let mut plan = Plan::default();
    for c in changes {
        match c {
            Change::Update { pk, values } => update(&mut plan, key_of(pk)?, values)?,
            Change::Insert { values } => insert(&mut plan, values)?,
            Change::Delete { pk } => {
                let key = key_of(pk)?;
                plan.check(Check::Exists(key.clone()));
                plan.push([b"DEL".as_slice(), &key]);
            }
        }
    }
    Ok(plan)
}

fn update(plan: &mut Plan, key: Vec<u8>, values: &Map<String, Value>) -> AppResult<()> {
    plan.check(Check::Exists(key.clone()));
    for column in values.keys() {
        if !["key", "ttl", "value"].contains(&column.as_str()) {
            return Err(AppError::validation(format!(
                "The {column} of a key can't be edited."
            )));
        }
    }
    if let Some(v) = values.get("value") {
        plan.check(Check::Type(key.clone(), "string"));
        let value = arg(v, "value")?;
        plan.push([b"SET".as_slice(), &key, &value, b"KEEPTTL"]);
    }
    if let Some(v) = values.get("ttl") {
        set_ttl(plan, &key, ttl_of(v)?);
    }
    if let Some(v) = values.get("key") {
        let to = arg(v, "key")?;
        if to.is_empty() {
            return Err(AppError::validation("A key name can't be empty."));
        }
        if to != key {
            plan.check(Check::Missing(to.clone()));
            plan.push([b"RENAME".as_slice(), &key, &to]);
        }
    }
    Ok(())
}

/// A new grid row is a string key.
fn insert(plan: &mut Plan, values: &Map<String, Value>) -> AppResult<()> {
    let key = values
        .get("key")
        .map(|k| arg(k, "key"))
        .transpose()?
        .filter(|k| !k.is_empty())
        .ok_or_else(|| AppError::validation("Give the new key a name."))?;
    match values.get("type").and_then(Value::as_str) {
        None | Some("") | Some("string") => {}
        Some(_) => {
            return Err(AppError::validation(
                "New rows are string keys; create other types with New key.",
            ))
        }
    }
    let value = match values.get("value") {
        None | Some(Value::Null) => Vec::new(),
        Some(v) => arg(v, "value")?,
    };
    plan.check(Check::Missing(key.clone()));
    let ttl = values.get("ttl").map(ttl_of).transpose()?.flatten();
    match ttl {
        Some(s) => plan.push([
            b"SET".as_slice(),
            &key,
            &value,
            b"EX",
            s.to_string().as_bytes(),
        ]),
        None => plan.push([b"SET".as_slice(), &key, &value]),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn grid(changes: Value) -> AppResult<Plan> {
        let changes: Vec<Change> = serde_json::from_value(changes).unwrap();
        grid_plan(&changes)
    }

    #[test]
    fn grid_updates_rename_last() {
        let p = grid(json!([{
            "kind": "update",
            "pk": { "key": "a" },
            "values": { "key": "b", "ttl": 60, "value": "x" },
        }]))
        .unwrap();
        assert_eq!(
            p.previews(),
            ["SET a x KEEPTTL", "EXPIRE a 60", "RENAME a b"]
        );
        assert_eq!(
            p.checks,
            [
                Check::Exists(b"a".to_vec()),
                Check::Type(b"a".to_vec(), "string"),
                Check::Missing(b"b".to_vec()),
            ]
        );
        let persist = grid(json!([{ "kind": "update", "pk": { "key": "a" },
            "values": { "ttl": -1 } }]))
        .unwrap();
        assert_eq!(persist.previews(), ["PERSIST a"]);
        let bad = json!([{ "kind": "update", "pk": { "key": "a" }, "values": { "size": 2 } }]);
        assert!(grid(bad).is_err());
        let null = json!([{ "kind": "update", "pk": { "key": "a" }, "values": { "value": null } }]);
        assert!(grid(null).is_err());
    }

    #[test]
    fn grid_inserts_and_deletes() {
        let p = grid(json!([
            { "kind": "insert", "values": { "key": "n", "value": "v", "ttl": 10 } },
            { "kind": "delete", "pk": { "key": "old" } },
        ]))
        .unwrap();
        assert_eq!(p.previews(), ["SET n v EX 10", "DEL old"]);
        assert!(grid(json!([{ "kind": "insert", "values": { "value": "v" } }])).is_err());
        let hash = json!([{ "kind": "insert", "values": { "key": "h", "type": "hash" } }]);
        assert!(grid(hash).is_err());
    }
}
