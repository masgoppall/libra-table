//! What the value panel changes, as plans: the inside of hashes, lists, sets and sorted
//! sets, TTLs, renames, deletes and new keys of each type.

use serde::Deserialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::edits::{arg, set_ttl};
use super::plan::{Check, Plan};

/// Placeholder written into a list element before `LREM` removes it by value.
const REMOVED: &str = "__libratable_removed__";

/// A change the value panel makes to one key.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum KeyEdit {
    SetString {
        key: Value,
        value: Value,
    },
    /// Sets a field; with `previous` the field is renamed too.
    HashSet {
        key: Value,
        field: Value,
        value: Value,
        #[serde(default)]
        previous: Option<Value>,
    },
    HashDelete {
        key: Value,
        fields: Vec<Value>,
    },
    ListSet {
        key: Value,
        index: i64,
        value: Value,
        expected: Value,
    },
    ListPush {
        key: Value,
        values: Vec<Value>,
        #[serde(default)]
        head: bool,
    },
    ListRemove {
        key: Value,
        index: i64,
        expected: Value,
    },
    SetAdd {
        key: Value,
        members: Vec<Value>,
    },
    SetRemove {
        key: Value,
        members: Vec<Value>,
    },
    /// Adds a member or changes its score; with `previous` the member is renamed.
    ZsetAdd {
        key: Value,
        member: Value,
        score: f64,
        #[serde(default)]
        previous: Option<Value>,
    },
    ZsetRemove {
        key: Value,
        members: Vec<Value>,
    },
    /// Seconds to live; None (or ≤ 0) removes the expiry.
    Expire {
        key: Value,
        seconds: Option<i64>,
    },
    Rename {
        key: Value,
        to: Value,
    },
    Delete {
        keys: Vec<Value>,
    },
    Create {
        key: Value,
        #[serde(rename = "type")]
        key_type: String,
        value: Value,
        #[serde(default)]
        ttl: Option<i64>,
    },
}

impl KeyEdit {
    /// Keys the edit changes (for the result count).
    pub fn affected(&self) -> u64 {
        match self {
            KeyEdit::Delete { keys } => keys.len() as u64,
            _ => 1,
        }
    }
}

fn args_of(values: &[Value], what: &str) -> AppResult<Vec<Vec<u8>>> {
    if values.is_empty() {
        return Err(AppError::validation(format!("Give at least one {what}.")));
    }
    values.iter().map(|v| arg(v, what)).collect()
}

fn command(name: &str, key: &[u8], rest: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut c = vec![name.as_bytes().to_vec(), key.to_vec()];
    c.extend(rest);
    c
}

fn score_text(score: f64) -> AppResult<String> {
    if score.is_nan() {
        return Err(AppError::validation("The score must be a number."));
    }
    Ok(if score.is_infinite() {
        if score > 0.0 { "+inf" } else { "-inf" }.to_string()
    } else {
        score.to_string()
    })
}

/// The plan for a value-panel edit.
pub fn key_plan(edit: &KeyEdit) -> AppResult<Plan> {
    let mut p = Plan::default();
    match edit {
        KeyEdit::SetString { key, value } => {
            let k = arg(key, "key")?;
            p.check(Check::Type(k.clone(), "string"));
            p.push([b"SET".as_slice(), &k, &arg(value, "value")?, b"KEEPTTL"]);
        }
        KeyEdit::HashSet {
            key,
            field,
            value,
            previous,
        } => {
            let k = arg(key, "key")?;
            let f = arg(field, "field")?;
            p.check(Check::Type(k.clone(), "hash"));
            if let Some(prev) = previous.as_ref().map(|v| arg(v, "field")).transpose()? {
                if prev != f {
                    p.commands.push(command("HDEL", &k, vec![prev]));
                }
            }
            p.commands
                .push(command("HSET", &k, vec![f, arg(value, "value")?]));
        }
        KeyEdit::HashDelete { key, fields } => {
            let k = arg(key, "key")?;
            p.check(Check::Type(k.clone(), "hash"));
            p.commands
                .push(command("HDEL", &k, args_of(fields, "field")?));
        }
        KeyEdit::ListSet {
            key,
            index,
            value,
            expected,
        } => {
            let k = arg(key, "key")?;
            p.check(Check::ListItem(k.clone(), *index, arg(expected, "value")?));
            let i = index.to_string().into_bytes();
            p.commands
                .push(command("LSET", &k, vec![i, arg(value, "value")?]));
        }
        KeyEdit::ListPush { key, values, head } => {
            let k = arg(key, "key")?;
            p.check(Check::Type(k.clone(), "list"));
            let name = if *head { "LPUSH" } else { "RPUSH" };
            p.commands
                .push(command(name, &k, args_of(values, "value")?));
        }
        KeyEdit::ListRemove {
            key,
            index,
            expected,
        } => {
            let k = arg(key, "key")?;
            p.check(Check::ListItem(k.clone(), *index, arg(expected, "value")?));
            let i = index.to_string().into_bytes();
            let mark = REMOVED.as_bytes().to_vec();
            p.commands.push(command("LSET", &k, vec![i, mark.clone()]));
            p.commands
                .push(command("LREM", &k, vec![b"1".to_vec(), mark]));
        }
        KeyEdit::SetAdd { key, members } | KeyEdit::SetRemove { key, members } => {
            let k = arg(key, "key")?;
            p.check(Check::Type(k.clone(), "set"));
            let name = if matches!(edit, KeyEdit::SetAdd { .. }) {
                "SADD"
            } else {
                "SREM"
            };
            p.commands
                .push(command(name, &k, args_of(members, "member")?));
        }
        KeyEdit::ZsetAdd {
            key,
            member,
            score,
            previous,
        } => {
            let k = arg(key, "key")?;
            let m = arg(member, "member")?;
            p.check(Check::Type(k.clone(), "zset"));
            if let Some(prev) = previous.as_ref().map(|v| arg(v, "member")).transpose()? {
                if prev != m {
                    p.commands.push(command("ZREM", &k, vec![prev]));
                }
            }
            let s = score_text(*score)?.into_bytes();
            p.commands.push(command("ZADD", &k, vec![s, m]));
        }
        KeyEdit::ZsetRemove { key, members } => {
            let k = arg(key, "key")?;
            p.check(Check::Type(k.clone(), "zset"));
            p.commands
                .push(command("ZREM", &k, args_of(members, "member")?));
        }
        KeyEdit::Expire { key, seconds } => {
            let k = arg(key, "key")?;
            p.check(Check::Exists(k.clone()));
            set_ttl(&mut p, &k, seconds.filter(|s| *s > 0));
        }
        KeyEdit::Rename { key, to } => {
            let k = arg(key, "key")?;
            let t = arg(to, "key")?;
            if t.is_empty() {
                return Err(AppError::validation("A key name can't be empty."));
            }
            if t != k {
                p.check(Check::Exists(k.clone()));
                p.check(Check::Missing(t.clone()));
                p.push([b"RENAME".as_slice(), &k, &t]);
            }
        }
        KeyEdit::Delete { keys } => {
            let ks = args_of(keys, "key")?;
            for k in &ks {
                p.check(Check::Exists(k.clone()));
            }
            let mut c = vec![b"DEL".to_vec()];
            c.extend(ks);
            p.commands.push(c);
        }
        KeyEdit::Create {
            key,
            key_type,
            value,
            ttl,
        } => create(&mut p, arg(key, "key")?, key_type, value, *ttl)?,
    }
    Ok(p)
}

/// `[{"a": "field", "b": "value"}, …]` (or an object) as flat argument pairs.
fn pairs(value: &Value, a: &str, b: &str) -> AppResult<Vec<Vec<u8>>> {
    let mut out = Vec::new();
    match value {
        Value::Object(o) => {
            for (k, v) in o {
                out.push(k.as_bytes().to_vec());
                out.push(arg(v, b)?);
            }
        }
        Value::Array(items) => {
            for item in items {
                let first = item.get(a).unwrap_or(&Value::Null);
                let second = item.get(b).unwrap_or(&Value::Null);
                out.push(arg(first, a)?);
                out.push(arg(second, b)?);
            }
        }
        _ => {}
    }
    if out.is_empty() {
        return Err(AppError::validation(format!("Add at least one {a}.")));
    }
    Ok(out)
}

fn list_items(value: &Value, what: &str) -> AppResult<Vec<Vec<u8>>> {
    match value {
        Value::Array(items) => args_of(items, what),
        other => args_of(std::slice::from_ref(other), what),
    }
}

fn create(
    p: &mut Plan,
    key: Vec<u8>,
    key_type: &str,
    value: &Value,
    ttl: Option<i64>,
) -> AppResult<()> {
    if key.is_empty() {
        return Err(AppError::validation("Give the new key a name."));
    }
    p.check(Check::Missing(key.clone()));
    let cmd = match key_type {
        "string" => command("SET", &key, vec![arg(value, "value")?]),
        "hash" => command("HSET", &key, pairs(value, "field", "value")?),
        "list" => command("RPUSH", &key, list_items(value, "value")?),
        "set" => command("SADD", &key, list_items(value, "member")?),
        "zset" => {
            // ZADD wants score before member.
            let flat = pairs(value, "member", "score")?;
            let mut swapped = Vec::with_capacity(flat.len());
            for pair in flat.chunks(2) {
                let score = String::from_utf8_lossy(&pair[1]);
                let n: f64 = score
                    .trim()
                    .parse()
                    .map_err(|_| AppError::validation(format!("\"{score}\" is not a score.")))?;
                swapped.push(score_text(n)?.into_bytes());
                swapped.push(pair[0].clone());
            }
            command("ZADD", &key, swapped)
        }
        "stream" => {
            let mut rest = vec![b"*".to_vec()];
            rest.extend(pairs(value, "field", "value")?);
            command("XADD", &key, rest)
        }
        other => {
            return Err(AppError::validation(format!(
                "LibraTable can't create {other} keys; use the query editor."
            )))
        }
    };
    p.commands.push(cmd);
    if let Some(s) = ttl.filter(|s| *s > 0) {
        set_ttl(p, &key, Some(s));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn key(edit: Value) -> AppResult<Plan> {
        key_plan(&serde_json::from_value(edit).unwrap())
    }

    #[test]
    fn value_panel_edits() {
        let p = key(
            json!({ "op": "hash_set", "key": "h", "field": "new", "value": "1",
            "previous": "old" }),
        )
        .unwrap();
        assert_eq!(p.previews(), ["HDEL h old", "HSET h new 1"]);
        let p =
            key(json!({ "op": "list_remove", "key": "l", "index": 2, "expected": "x" })).unwrap();
        assert_eq!(p.checks, [Check::ListItem(b"l".to_vec(), 2, b"x".to_vec())]);
        assert_eq!(p.commands.len(), 2);
        let p = key(json!({ "op": "zset_add", "key": "z", "member": "m", "score": 1.5 })).unwrap();
        assert_eq!(p.previews(), ["ZADD z 1.5 m"]);
        let p = key(json!({ "op": "expire", "key": "k", "seconds": null })).unwrap();
        assert_eq!(p.previews(), ["PERSIST k"]);
        let p = key(json!({ "op": "delete", "keys": ["a", "b"] })).unwrap();
        assert_eq!(p.previews(), ["DEL a b"]);
        assert!(key(json!({ "op": "set_add", "key": "s", "members": [] })).is_err());
        let same = key(json!({ "op": "rename", "key": "a", "to": "a" })).unwrap();
        assert!(same.commands.is_empty());
    }

    #[test]
    fn creates_each_type() {
        let make = |t: &str, v: Value| {
            key(json!({ "op": "create", "key": "k", "type": t, "value": v, "ttl": 5 }))
                .map(|p| p.previews())
        };
        assert_eq!(
            make("string", json!("v")).unwrap(),
            ["SET k v", "EXPIRE k 5"]
        );
        assert_eq!(
            make("hash", json!([{ "field": "f", "value": "v" }])).unwrap()[0],
            "HSET k f v"
        );
        assert_eq!(make("list", json!(["a", "b"])).unwrap()[0], "RPUSH k a b");
        assert_eq!(make("set", json!(["a"])).unwrap()[0], "SADD k a");
        assert_eq!(
            make("zset", json!([{ "member": "m", "score": "2" }])).unwrap()[0],
            "ZADD k 2 m"
        );
        assert_eq!(
            make("stream", json!([{ "field": "f", "value": "v" }])).unwrap()[0],
            "XADD k * f v"
        );
        assert!(make("hash", json!([])).is_err());
        assert!(make("ReJSON-RL", json!("{}")).is_err());
    }
}
