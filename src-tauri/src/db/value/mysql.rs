//! MySQL cells: typed decoding for numbers, dates and blobs; bytes as text otherwise.

use super::*;
use sqlx::mysql::MySqlRow;

pub fn row(row: &MySqlRow, bin_limit: usize) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| cell(row, i, bin_limit))
        .collect()
}

fn cell(row: &MySqlRow, i: usize, bin_limit: usize) -> Value {
    let raw = match row.try_get_raw(i) {
        Ok(r) => r,
        Err(_) => return Value::Null,
    };
    if raw.is_null() {
        return Value::Null;
    }
    let ty = raw.type_info().name().to_string();
    if let Some(v) = typed(row, i, &ty, bin_limit) {
        return v;
    }
    // Strings, ENUM, SET and anything unknown: raw bytes, shown as text when valid UTF-8.
    match row.try_get_unchecked::<Vec<u8>, _>(i) {
        Ok(bytes) => text_or_binary(&bytes, bin_limit),
        Err(_) => row
            .try_get_unchecked::<String, _>(i)
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

/// Decodes the types we know; None for the others.
fn typed(row: &MySqlRow, i: usize, ty: &str, bin_limit: usize) -> Option<Value> {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    match ty {
        "BOOLEAN" => row
            .try_get_unchecked::<i64, _>(i)
            .ok()
            .map(|v| Value::Bool(v != 0)),
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" | "YEAR" => {
            row.try_get_unchecked::<i64, _>(i).ok().map(int)
        }
        "TINYINT UNSIGNED" | "SMALLINT UNSIGNED" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED"
        | "BIGINT UNSIGNED" => row.try_get_unchecked::<u64, _>(i).ok().map(uint),
        "FLOAT" => row
            .try_get_unchecked::<f32, _>(i)
            .ok()
            .map(|v| float(v as f64)),
        "DOUBLE" => row.try_get_unchecked::<f64, _>(i).ok().map(float),
        "DECIMAL" => row
            .try_get_unchecked::<rust_decimal::Decimal, _>(i)
            .ok()
            .map(text),
        "DATE" => row.try_get_unchecked::<NaiveDate, _>(i).ok().map(text),
        "TIME" => row.try_get_unchecked::<NaiveTime, _>(i).ok().map(text),
        "DATETIME" | "TIMESTAMP" => row
            .try_get_unchecked::<NaiveDateTime, _>(i)
            .ok()
            .map(|v| text(v.format("%Y-%m-%d %H:%M:%S%.f"))),
        "JSON" => row.try_get_unchecked::<Value, _>(i).ok().map(text),
        "BINARY" | "VARBINARY" | "TINYBLOB" | "BLOB" | "MEDIUMBLOB" | "LONGBLOB" | "GEOMETRY"
        | "BIT" => row
            .try_get_unchecked::<Vec<u8>, _>(i)
            .ok()
            .map(|v| binary_with(&v, bin_limit)),
        _ => None,
    }
}
