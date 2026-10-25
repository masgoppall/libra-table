//! Converts database cells to JSON for the UI.
//!
//! * NULL → `null`
//! * booleans → `true`/`false`
//! * integers within ±(2^53−1) and finite floats → numbers; anything larger or exact
//!   (bigint, numeric/decimal) → strings so JavaScript never rounds them
//! * binary → `{"$binary": "<hex of the first 4 KiB>", "size": <bytes>}`
//! * everything else (dates, uuid, json, enums, arrays, …) → its text form

pub mod mysql;
pub mod pg;
pub mod sqlite;

use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Column, Row, TypeInfo, ValueRef};

const MAX_SAFE_INT: i64 = 9_007_199_254_740_991;
const BINARY_PREVIEW: usize = 4096;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
}

pub fn int(v: i64) -> Value {
    if (-MAX_SAFE_INT..=MAX_SAFE_INT).contains(&v) {
        Value::from(v)
    } else {
        Value::String(v.to_string())
    }
}

pub fn uint(v: u64) -> Value {
    if v <= MAX_SAFE_INT as u64 {
        Value::from(v)
    } else {
        Value::String(v.to_string())
    }
}

pub fn float(v: f64) -> Value {
    if v.is_finite() {
        json!(v)
    } else {
        Value::String(v.to_string())
    }
}

pub fn binary(bytes: &[u8]) -> Value {
    binary_with(bytes, BINARY_PREVIEW)
}

/// Binary as hex, keeping at most `limit` bytes (`usize::MAX` for exports).
pub fn binary_with(bytes: &[u8], limit: usize) -> Value {
    let shown = &bytes[..bytes.len().min(limit)];
    json!({ "$binary": hex::encode(shown), "size": bytes.len() })
}

/// A value shown as its text form (dates, decimals, uuids, JSON…).
fn text(v: impl ToString) -> Value {
    Value::String(v.to_string())
}

fn text_or_binary(bytes: &[u8], limit: usize) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(s) => Value::String(s.to_string()),
        Err(_) => binary_with(bytes, limit),
    }
}

/// Grid / editor cells: binary values are previewed (first 4 KiB).
pub const PREVIEW: usize = BINARY_PREVIEW;
/// Exports: binary values are kept whole.
pub const FULL: usize = usize::MAX;

pub fn columns<R: Row>(row: &R) -> Vec<ColumnMeta> {
    row.columns()
        .iter()
        .map(|c| ColumnMeta {
            name: c.name().to_string(),
            type_name: c.type_info().name().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_numbers_become_strings() {
        assert_eq!(int(42), json!(42));
        assert_eq!(int(9_007_199_254_740_993), json!("9007199254740993"));
        assert_eq!(uint(u64::MAX), json!(u64::MAX.to_string()));
        assert_eq!(float(f64::NAN), json!("NaN"));
    }

    #[test]
    fn binary_is_truncated_hex_with_size() {
        let v = binary(&vec![0xab; 5000]);
        assert_eq!(v["size"], 5000);
        assert_eq!(v["$binary"].as_str().unwrap().len(), BINARY_PREVIEW * 2);
        assert_eq!(text_or_binary(b"hi", PREVIEW), json!("hi"));
        assert!(text_or_binary(&[0xff, 0xfe], PREVIEW)
            .get("$binary")
            .is_some());
        assert_eq!(
            binary_with(&vec![1; 5000], FULL)["$binary"]
                .as_str()
                .unwrap()
                .len(),
            10000
        );
    }
}
