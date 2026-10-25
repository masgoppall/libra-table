//! Postgres cells: typed decoding for common types, text for the rest.

use super::*;
use sqlx::postgres::{types::PgInterval, PgRow, PgValueFormat};

pub fn row(row: &PgRow, bin_limit: usize) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| cell(row, i, bin_limit))
        .collect()
}

fn cell(row: &PgRow, i: usize, bin_limit: usize) -> Value {
    let raw = match row.try_get_raw(i) {
        Ok(r) => r,
        Err(_) => return Value::Null,
    };
    if raw.is_null() {
        return Value::Null;
    }
    let ty = raw.type_info().name().to_string();
    let format = raw.format();
    if let Some(v) = typed(row, i, &ty, bin_limit) {
        return v;
    }
    // Text format (simple query protocol, or columns we cast to ::text): the bytes are
    // the text. Binary format of an unknown type: enums/domains over text are UTF-8;
    // others show as hex.
    match raw.as_bytes() {
        Ok(bytes) if format == PgValueFormat::Text => {
            Value::String(String::from_utf8_lossy(bytes).into_owned())
        }
        Ok(bytes) => text_or_binary(bytes, bin_limit),
        Err(_) => Value::Null,
    }
}

/// Decodes the types we know; None for the others.
fn typed(row: &PgRow, i: usize, ty: &str, bin_limit: usize) -> Option<Value> {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
    match ty {
        "BOOL" => row.try_get::<bool, _>(i).ok().map(Value::Bool),
        "INT2" => row.try_get::<i16, _>(i).ok().map(|v| int(v as i64)),
        "INT4" => row.try_get::<i32, _>(i).ok().map(|v| int(v as i64)),
        "INT8" => row.try_get::<i64, _>(i).ok().map(int),
        "OID" => row
            .try_get::<sqlx::postgres::types::Oid, _>(i)
            .ok()
            .map(|v| int(v.0 as i64)),
        "FLOAT4" => row.try_get::<f32, _>(i).ok().map(|v| float(v as f64)),
        "FLOAT8" => row.try_get::<f64, _>(i).ok().map(float),
        "NUMERIC" => row.try_get::<rust_decimal::Decimal, _>(i).ok().map(text),
        "UUID" => row.try_get::<uuid::Uuid, _>(i).ok().map(text),
        "JSON" | "JSONB" => row.try_get::<Value, _>(i).ok().map(text),
        "DATE" => row.try_get::<NaiveDate, _>(i).ok().map(text),
        "TIME" => row.try_get::<NaiveTime, _>(i).ok().map(text),
        "TIMESTAMP" => row
            .try_get::<NaiveDateTime, _>(i)
            .ok()
            .map(|v| text(v.format("%Y-%m-%d %H:%M:%S%.f"))),
        "TIMESTAMPTZ" => row
            .try_get::<DateTime<Utc>, _>(i)
            .ok()
            .map(|v| text(v.format("%Y-%m-%d %H:%M:%S%.f%:z"))),
        "INTERVAL" => row
            .try_get::<PgInterval, _>(i)
            .ok()
            .map(|v| Value::String(interval(&v))),
        "BYTEA" => row
            .try_get::<Vec<u8>, _>(i)
            .ok()
            .map(|v| binary_with(&v, bin_limit)),
        "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "CHAR" | "CITEXT" | "XML" | "UNKNOWN" => row
            .try_get_unchecked::<String, _>(i)
            .ok()
            .map(Value::String),
        _ => None,
    }
}

fn interval(v: &PgInterval) -> String {
    let mut parts = Vec::new();
    if v.months != 0 {
        let (y, m) = (v.months / 12, v.months % 12);
        if y != 0 {
            parts.push(format!("{y} year{}", if y.abs() == 1 { "" } else { "s" }));
        }
        if m != 0 {
            parts.push(format!("{m} mon{}", if m.abs() == 1 { "" } else { "s" }));
        }
    }
    if v.days != 0 {
        parts.push(format!(
            "{} day{}",
            v.days,
            if v.days.abs() == 1 { "" } else { "s" }
        ));
    }
    if v.microseconds != 0 || parts.is_empty() {
        let us = v.microseconds;
        let sign = if us < 0 { "-" } else { "" };
        let us = us.unsigned_abs();
        let (h, rem) = (us / 3_600_000_000, us % 3_600_000_000);
        let (m, rem) = (rem / 60_000_000, rem % 60_000_000);
        let (s, frac) = (rem / 1_000_000, rem % 1_000_000);
        let mut t = format!("{sign}{h:02}:{m:02}:{s:02}");
        if frac != 0 {
            t.push_str(&format!(".{frac:06}"));
        }
        parts.push(t);
    }
    parts.join(" ")
}

/// Types whose binary form we decode natively. Other columns are selected as
/// `::text` by the grid so arrays, ranges, inet, geometry… display correctly.
pub fn natively_decoded(format_type: &str) -> bool {
    let t = format_type.to_ascii_lowercase();
    let base = t.split('(').next().unwrap_or("").trim();
    matches!(
        base,
        "boolean"
            | "smallint"
            | "integer"
            | "bigint"
            | "oid"
            | "real"
            | "double precision"
            | "numeric"
            | "uuid"
            | "json"
            | "jsonb"
            | "date"
            | "time without time zone"
            | "timestamp without time zone"
            | "timestamp with time zone"
            | "interval"
            | "bytea"
            | "text"
            | "character varying"
            | "character"
            | "name"
            | "\"char\""
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_formatting() {
        let v = PgInterval {
            months: 14,
            days: 3,
            microseconds: 3_723_500_000,
        };
        assert_eq!(interval(&v), "1 year 2 mons 3 days 01:02:03.500000");
        assert_eq!(
            interval(&PgInterval {
                months: 0,
                days: 0,
                microseconds: 0
            }),
            "00:00:00"
        );
    }

    #[test]
    fn native_type_list() {
        assert!(natively_decoded("character varying(255)"));
        assert!(natively_decoded("timestamp with time zone"));
        assert!(!natively_decoded("integer[]"));
        assert!(!natively_decoded("inet"));
    }
}
