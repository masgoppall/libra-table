//! SQLite cells, by the storage class of each value.

use super::*;
use sqlx::sqlite::SqliteRow;

pub fn row(row: &SqliteRow, bin_limit: usize) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| cell(row, i, bin_limit))
        .collect()
}

fn cell(row: &SqliteRow, i: usize, bin_limit: usize) -> Value {
    let raw = match row.try_get_raw(i) {
        Ok(r) => r,
        Err(_) => return Value::Null,
    };
    if raw.is_null() {
        return Value::Null;
    }
    // SQLite is dynamically typed: this is the storage class of this value.
    let ty = raw.type_info().name().to_string();
    match ty.as_str() {
        "INTEGER" | "BOOLEAN" => row
            .try_get_unchecked::<i64, _>(i)
            .map(int)
            .unwrap_or(Value::Null),
        "REAL" => row
            .try_get_unchecked::<f64, _>(i)
            .map(float)
            .unwrap_or(Value::Null),
        "BLOB" => row
            .try_get_unchecked::<Vec<u8>, _>(i)
            .map(|v| binary_with(&v, bin_limit))
            .unwrap_or(Value::Null),
        _ => match row.try_get_unchecked::<String, _>(i) {
            Ok(s) => Value::String(s),
            Err(_) => row
                .try_get_unchecked::<Vec<u8>, _>(i)
                .map(|v| text_or_binary(&v, bin_limit))
                .unwrap_or(Value::Null),
        },
    }
}
