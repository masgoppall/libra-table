//! DynamoDB AttributeValues ↔ UI JSON. S → string, N → number when JSON keeps it
//! exact (else its text), B → binary, BOOL, NULL → null, M → object, L → array, string
//! and number sets → arrays, BS → array of binary. Grid edits come back as JSON (or
//! text typed into a cell) and are encoded by the column's type.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Map, Value};

use crate::db::value;
use crate::error::{AppError, AppResult};

/// The type tag of an AttributeValue (`S`, `N`, `M`…).
pub fn kind(av: &Value) -> &str {
    av.as_object()
        .and_then(|o| o.keys().next())
        .map(String::as_str)
        .unwrap_or("")
}

/// A number as JSON when that is exact, else as text.
pub fn number(s: &str) -> Value {
    if let Ok(i) = s.parse::<i64>() {
        return value::int(i);
    }
    match s.parse::<f64>() {
        Ok(f) if f.is_finite() && f.to_string() == s => value::float(f),
        _ => Value::String(s.to_string()),
    }
}

fn binary(b64: &Value, limit: usize) -> Value {
    let bytes = b64
        .as_str()
        .and_then(|s| STANDARD.decode(s).ok())
        .unwrap_or_default();
    value::binary_with(&bytes, limit)
}

/// An AttributeValue for the UI; `limit` caps binary previews.
pub fn to_json(av: &Value, limit: usize) -> Value {
    let Some((k, v)) = av.as_object().and_then(|o| o.iter().next()) else {
        return Value::Null;
    };
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    match k.as_str() {
        "S" => v.clone(),
        "N" => number(v.as_str().unwrap_or_default()),
        "B" => binary(v, limit),
        "BOOL" => v.clone(),
        "NULL" => Value::Null,
        "M" => Value::Object(
            v.as_object()
                .map(|m| {
                    m.iter()
                        .map(|(k, x)| (k.clone(), to_json(x, limit)))
                        .collect()
                })
                .unwrap_or_default(),
        ),
        "L" => Value::Array(list(v).iter().map(|x| to_json(x, limit)).collect()),
        "SS" => Value::Array(list(v)),
        "NS" => Value::Array(
            list(v)
                .iter()
                .map(|n| number(n.as_str().unwrap_or_default()))
                .collect(),
        ),
        "BS" => Value::Array(list(v).iter().map(|b| binary(b, limit)).collect()),
        _ => v.clone(),
    }
}

fn is_number(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.parse::<f64>().is_ok_and(f64::is_finite)
}

/// A text value for a filter or key, by the attribute's type (`N`, `S`, `B`…).
pub fn typed_text(text: &str, ty: &str) -> AppResult<Value> {
    Ok(match ty {
        "N" | "NS" if is_number(text) => json!({ "N": text.trim() }),
        "N" => return Err(AppError::validation(format!("\"{text}\" is not a number."))),
        "BOOL" if text == "true" || text == "false" => json!({ "BOOL": text == "true" }),
        "B" | "BS" => {
            let bytes = hex::decode(text.trim())
                .map_err(|_| AppError::validation("Enter binary values as hex."))?;
            json!({ "B": STANDARD.encode(bytes) })
        }
        _ => json!({ "S": text }),
    })
}

/// A UI value (or text typed into a cell) as an AttributeValue for an attribute of type
/// `ty` ("" when unknown).
pub fn from_json(v: &Value, ty: &str) -> AppResult<Value> {
    Ok(match v {
        Value::Null => json!({ "NULL": true }),
        Value::Bool(b) => json!({ "BOOL": b }),
        Value::Number(n) => json!({ "N": n.to_string() }),
        Value::String(s) => match ty {
            "M" | "L" | "SS" | "NS" | "BS" => match serde_json::from_str::<Value>(s) {
                Ok(parsed @ (Value::Object(_) | Value::Array(_))) => from_json(&parsed, ty)?,
                _ => json!({ "S": s }),
            },
            _ => typed_text(s, ty)?,
        },
        Value::Object(o) if o.contains_key("$binary") => {
            return Err(AppError::validation(
                "Editing binary values is not supported yet.",
            ))
        }
        Value::Object(o) => {
            let m: AppResult<Map<String, Value>> = o
                .iter()
                .map(|(k, x)| Ok((k.clone(), from_json(x, "")?)))
                .collect();
            json!({ "M": m? })
        }
        Value::Array(items) => set_or_list(items, ty)?,
    })
}

fn set_or_list(items: &[Value], ty: &str) -> AppResult<Value> {
    let strings = || {
        items
            .iter()
            .map(|i| i.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
    };
    match ty {
        "SS" if !items.is_empty() => {
            if let Some(s) = strings() {
                return Ok(json!({ "SS": s }));
            }
        }
        "NS" if !items.is_empty() => {
            let nums: Option<Vec<String>> = items
                .iter()
                .map(|i| match i {
                    Value::Number(n) => Some(n.to_string()),
                    Value::String(s) if is_number(s) => Some(s.trim().to_string()),
                    _ => None,
                })
                .collect();
            if let Some(n) = nums {
                return Ok(json!({ "NS": n }));
            }
        }
        _ => {}
    }
    let list: AppResult<Vec<Value>> = items.iter().map(|x| from_json(x, "")).collect();
    Ok(json!({ "L": list? }))
}

/// The display type of a column: the one type seen, or "mixed".
pub fn merge_type(seen: &str, new: &str) -> String {
    match (seen, new) {
        ("", n) => n.to_string(),
        (s, n) if s == n || n == "NULL" => s.to_string(),
        ("NULL", n) => n.to_string(),
        _ => "mixed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_values_to_json() {
        let item = json!({"M": {
            "s": {"S": "x"}, "n": {"N": "12"}, "f": {"N": "1.5"},
            "big": {"N": "123456789012345678901234567890"}, "b": {"B": "AP8="},
            "ok": {"BOOL": true}, "none": {"NULL": true}, "l": {"L": [{"N": "1"}, {"S": "a"}]},
            "ss": {"SS": ["a", "b"]}, "ns": {"NS": ["1", "2.25"]}
        }});
        assert_eq!(
            to_json(&item, value::PREVIEW),
            json!({
                "s": "x", "n": 12, "f": 1.5, "big": "123456789012345678901234567890",
                "b": {"$binary": "00ff", "size": 2}, "ok": true, "none": null,
                "l": [1, "a"], "ss": ["a", "b"], "ns": [1, 2.25]
            })
        );
        assert_eq!(number("0.1000"), json!("0.1000"));
        assert_eq!(kind(&json!({"SS": []})), "SS");
    }

    #[test]
    fn json_back_to_attribute_values() {
        assert_eq!(from_json(&json!("42"), "N").unwrap(), json!({"N": "42"}));
        assert!(from_json(&json!("abc"), "N").is_err());
        assert_eq!(from_json(&json!("42"), "S").unwrap(), json!({"S": "42"}));
        assert_eq!(
            from_json(&json!("{\"a\": [1, \"x\"]}"), "M").unwrap(),
            json!({"M": {"a": {"L": [{"N": "1"}, {"S": "x"}]}}})
        );
        assert_eq!(
            from_json(&json!(["a"]), "SS").unwrap(),
            json!({"SS": ["a"]})
        );
        assert_eq!(
            from_json(&json!([1, "2"]), "NS").unwrap(),
            json!({"NS": ["1", "2"]})
        );
        assert_eq!(from_json(&json!([]), "SS").unwrap(), json!({"L": []}));
        assert_eq!(from_json(&Value::Null, "S").unwrap(), json!({"NULL": true}));
        assert_eq!(typed_text("00ff", "B").unwrap(), json!({"B": "AP8="}));
        assert_eq!(merge_type("S", "N"), "mixed");
        assert_eq!(merge_type("NULL", "N"), "N");
    }
}
