//! Redis bytes and replies as JSON for the UI: UTF-8 text as strings, other bytes as
//! `value::binary`, nested replies as JSON arrays and objects. Keys come back from the UI
//! in the same shape and are decoded to bytes again.

use redis::Value as Reply;
use serde_json::{Map, Value};

use crate::db::value;
use crate::error::{AppError, AppResult};

/// Bytes as text when they are UTF-8, else as a binary preview.
pub fn bytes(b: &[u8]) -> Value {
    match std::str::from_utf8(b) {
        Ok(s) => Value::String(s.to_string()),
        Err(_) => value::binary(b),
    }
}

/// A key (or member, field…) sent by the UI: text, or `{"$binary": hex, "size": n}`.
pub fn key_bytes(v: &Value) -> AppResult<Vec<u8>> {
    match v {
        Value::String(s) => Ok(s.as_bytes().to_vec()),
        Value::Number(n) => Ok(n.to_string().into_bytes()),
        Value::Object(o) => binary_bytes(o),
        _ => Err(AppError::validation("Give the key as text.")),
    }
}

fn binary_bytes(o: &Map<String, Value>) -> AppResult<Vec<u8>> {
    let hex = o.get("$binary").and_then(Value::as_str);
    let size = o.get("size").and_then(Value::as_u64);
    match (hex, size) {
        (Some(h), Some(n)) if h.len() as u64 == n * 2 => {
            hex::decode(h).map_err(|_| AppError::validation("The binary value is not valid hex."))
        }
        (Some(_), Some(_)) => Err(AppError::validation(
            "This binary value is too long to edit here; use the query editor.",
        )),
        _ => Err(AppError::validation("Give the value as text.")),
    }
}

/// Text of a scalar reply (status, number…), None for nested replies.
pub fn scalar_text(r: &Reply) -> Option<String> {
    Some(match r {
        Reply::Okay => "OK".into(),
        Reply::SimpleString(s) => s.clone(),
        Reply::Int(i) => i.to_string(),
        Reply::Double(d) => d.to_string(),
        Reply::Boolean(b) => b.to_string(),
        Reply::BigNumber(n) => n.to_string(),
        Reply::BulkString(b) => String::from_utf8_lossy(b).into_owned(),
        Reply::VerbatimString { text, .. } => text.clone(),
        _ => return None,
    })
}

/// A reply as a JSON value (maps become objects when their keys are text).
pub fn reply(r: &Reply) -> Value {
    match r {
        Reply::Nil => Value::Null,
        Reply::Int(i) => value::int(*i),
        Reply::Double(d) => value::float(*d),
        Reply::Boolean(b) => Value::Bool(*b),
        Reply::BulkString(b) => bytes(b),
        Reply::Array(items) | Reply::Set(items) => Value::Array(items.iter().map(reply).collect()),
        Reply::Push { data, .. } => Value::Array(data.iter().map(reply).collect()),
        Reply::Map(pairs) => map(pairs),
        Reply::Attribute { data, .. } => reply(data),
        Reply::ServerError(e) => {
            let detail = e.details().unwrap_or_default();
            Value::String(format!("(error) {} {detail}", e.code()))
        }
        other => Value::String(scalar_text(other).unwrap_or_default()),
    }
}

fn map(pairs: &[(Reply, Reply)]) -> Value {
    let mut out = Map::new();
    for (k, v) in pairs {
        match scalar_text(k) {
            Some(name) => {
                out.insert(name, reply(v));
            }
            None => {
                let all = pairs
                    .iter()
                    .map(|(k, v)| Value::Array(vec![reply(k), reply(v)]))
                    .collect();
                return Value::Array(all);
            }
        }
    }
    Value::Object(out)
}

/// Bytes of a bulk reply (keys, members…); scalars as their text.
pub fn reply_bytes(r: &Reply) -> Vec<u8> {
    match r {
        Reply::BulkString(b) => b.clone(),
        other => scalar_text(other).unwrap_or_default().into_bytes(),
    }
}

/// Elements of an array reply (empty for anything else).
pub fn items(r: Reply) -> Vec<Reply> {
    match r {
        Reply::Array(items) | Reply::Set(items) => items,
        Reply::Map(pairs) => pairs.into_iter().flat_map(|(k, v)| [k, v]).collect(),
        _ => Vec::new(),
    }
}

pub fn int(r: &Reply) -> Option<i64> {
    match r {
        Reply::Int(i) => Some(*i),
        other => scalar_text(other)?.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bytes_and_keys_round_trip() {
        assert_eq!(bytes(b"user:1"), json!("user:1"));
        let bin = bytes(&[0xff, 0x00]);
        assert_eq!(bin, json!({ "$binary": "ff00", "size": 2 }));
        assert_eq!(key_bytes(&bin).unwrap(), vec![0xff, 0x00]);
        assert_eq!(key_bytes(&json!("k")).unwrap(), b"k");
        let cut = json!({ "$binary": "ff", "size": 5000 });
        assert!(key_bytes(&cut).is_err());
    }

    #[test]
    fn replies_as_json() {
        let r = Reply::Array(vec![
            Reply::BulkString(b"a".to_vec()),
            Reply::Int(2),
            Reply::Nil,
            Reply::Map(vec![(Reply::SimpleString("f".into()), Reply::Okay)]),
        ]);
        assert_eq!(reply(&r), json!(["a", 2, null, { "f": "OK" }]));
        assert_eq!(int(&Reply::BulkString(b"12".to_vec())), Some(12));
    }
}
