//! BigQuery schemas and rows. Columns are the top-level fields; a RECORD (STRUCT) is
//! one column holding a JSON object and a REPEATED field one holding a JSON array, not
//! flattened, so every grid row stays one table row. Types are shown in GoogleSQL
//! spelling (INT64, STRUCT<…>, ARRAY<…>).

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use chrono::DateTime;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::db::meta::ColumnInfo;
use crate::db::value::{self, ColumnMeta};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default_value_expression: Option<String>,
    #[serde(default)]
    pub max_length: Option<String>,
    #[serde(default)]
    pub precision: Option<String>,
    #[serde(default)]
    pub scale: Option<String>,
}

impl Field {
    fn repeated(&self) -> bool {
        self.mode.as_deref() == Some("REPEATED")
    }

    fn base(&self) -> &str {
        match self.kind.as_str() {
            "INTEGER" => "INT64",
            "FLOAT" => "FLOAT64",
            "BOOLEAN" => "BOOL",
            "RECORD" => "STRUCT",
            other => other,
        }
    }

    /// The GoogleSQL type of the field.
    pub fn type_name(&self) -> String {
        let base = self.base();
        let scalar = match (base, &self.max_length, &self.precision, &self.scale) {
            ("STRUCT", ..) => {
                let inner: Vec<String> = self
                    .fields
                    .iter()
                    .map(|f| format!("{} {}", f.name, f.type_name()))
                    .collect();
                format!("STRUCT<{}>", inner.join(", "))
            }
            ("STRING" | "BYTES", Some(n), ..) => format!("{base}({n})"),
            ("NUMERIC" | "BIGNUMERIC", _, Some(p), Some(s)) => format!("{base}({p}, {s})"),
            ("NUMERIC" | "BIGNUMERIC", _, Some(p), None) => format!("{base}({p})"),
            _ => base.to_string(),
        };
        if self.repeated() {
            format!("ARRAY<{scalar}>")
        } else {
            scalar
        }
    }

    pub fn column(&self, pk_position: Option<i32>) -> ColumnInfo {
        ColumnInfo {
            name: self.name.clone(),
            data_type: self.type_name(),
            nullable: self.mode.as_deref() != Some("REQUIRED"),
            default: self.default_value_expression.clone(),
            pk_position,
            auto_increment: false,
            comment: self.description.clone().filter(|d| !d.is_empty()),
            ..Default::default()
        }
    }
}

pub fn columns(fields: &[Field]) -> Vec<ColumnMeta> {
    fields
        .iter()
        .map(|f| ColumnMeta {
            name: f.name.clone(),
            type_name: f.type_name(),
        })
        .collect()
}

/// One `{"f": [{"v": …}]}` row for the UI.
pub fn row(fields: &[Field], raw: &Value, binary_limit: usize) -> Vec<Value> {
    let cells = raw.get("f").and_then(Value::as_array);
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let v = cells
                .and_then(|c| c.get(i))
                .and_then(|c| c.get("v"))
                .unwrap_or(&Value::Null);
            cell(f, v, binary_limit)
        })
        .collect()
}

pub fn cell(f: &Field, v: &Value, binary_limit: usize) -> Value {
    if v.is_null() {
        return Value::Null;
    }
    if f.repeated() {
        let item = Field {
            mode: None,
            ..f.clone()
        };
        let items = v.as_array().map(Vec::as_slice).unwrap_or_default();
        return Value::Array(
            items
                .iter()
                .map(|x| cell(&item, x.get("v").unwrap_or(&Value::Null), binary_limit))
                .collect(),
        );
    }
    if f.base() == "STRUCT" {
        let subs = v.get("f").and_then(Value::as_array);
        let obj: Map<String, Value> = f
            .fields
            .iter()
            .enumerate()
            .map(|(i, sf)| {
                let x = subs
                    .and_then(|s| s.get(i))
                    .and_then(|c| c.get("v"))
                    .unwrap_or(&Value::Null);
                (sf.name.clone(), cell(sf, x, binary_limit))
            })
            .collect();
        return Value::Object(obj);
    }
    let Some(s) = v.as_str() else {
        return v.clone();
    };
    let decoded = match f.base() {
        "INT64" => s.parse::<i64>().ok().map(value::int),
        "FLOAT64" => s
            .parse::<f64>()
            .ok()
            .filter(|x| x.is_finite())
            .map(value::float),
        "BOOL" => s.parse::<bool>().ok().map(Value::Bool),
        "TIMESTAMP" => timestamp(s).map(Value::String),
        "BYTES" => STANDARD
            .decode(s)
            .ok()
            .map(|b| value::binary_with(&b, binary_limit)),
        "JSON" => serde_json::from_str(s).ok(),
        _ => None,
    };
    decoded.unwrap_or_else(|| Value::String(s.to_string()))
}

/// Microseconds since the epoch (`useInt64Timestamp`) or float seconds (older answers).
fn timestamp(s: &str) -> Option<String> {
    let micros: i64 = match s.parse::<i64>() {
        Ok(m) => m,
        Err(_) => (s.parse::<f64>().ok()? * 1e6).round() as i64,
    };
    let t = DateTime::from_timestamp_micros(micros)?.naive_utc();
    let frac = micros.rem_euclid(1_000_000);
    Some(if frac == 0 {
        format!("{} UTC", t.format("%Y-%m-%d %H:%M:%S"))
    } else {
        format!("{}.{frac:06} UTC", t.format("%Y-%m-%d %H:%M:%S"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fields() -> Vec<Field> {
        serde_json::from_value(json!([
            {"name": "id", "type": "INTEGER", "mode": "REQUIRED"},
            {"name": "rec", "type": "RECORD", "fields": [
                {"name": "a", "type": "INTEGER"}, {"name": "b", "type": "STRING"}]},
            {"name": "tags", "type": "STRING", "mode": "REPEATED"},
            {"name": "ts", "type": "TIMESTAMP"},
            {"name": "raw", "type": "BYTES"},
            {"name": "price", "type": "NUMERIC", "precision": "10", "scale": "2"},
            {"name": "ok", "type": "BOOLEAN"}
        ]))
        .unwrap()
    }

    #[test]
    fn types_in_googlesql_spelling() {
        let names: Vec<String> = fields().iter().map(Field::type_name).collect();
        assert_eq!(
            names,
            [
                "INT64",
                "STRUCT<a INT64, b STRING>",
                "ARRAY<STRING>",
                "TIMESTAMP",
                "BYTES",
                "NUMERIC(10, 2)",
                "BOOL"
            ]
        );
        assert!(!fields()[0].column(Some(1)).nullable);
    }

    #[test]
    fn rows_keep_nested_values_as_json() {
        let raw = json!({"f": [
            {"v": "9007199254740993"},
            {"v": {"f": [{"v": "1"}, {"v": "x"}]}},
            {"v": [{"v": "t1"}, {"v": "t2"}]},
            {"v": "1704164645123456"},
            {"v": "AP8="},
            {"v": "1.50"},
            {"v": "true"}
        ]});
        assert_eq!(
            row(&fields(), &raw, value::PREVIEW),
            vec![
                json!("9007199254740993"),
                json!({"a": 1, "b": "x"}),
                json!(["t1", "t2"]),
                json!("2024-01-02 03:04:05.123456 UTC"),
                json!({"$binary": "00ff", "size": 2}),
                json!("1.50"),
                json!(true),
            ]
        );
        assert_eq!(
            timestamp("1.704164645E9").unwrap(),
            "2024-01-02 03:04:05 UTC"
        );
    }
}
