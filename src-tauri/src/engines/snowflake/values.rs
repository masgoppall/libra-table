//! Snowflake JSON result cells (all text) to UI values, by the column's `rowtype`.
//! Dates are days since the epoch, times and timestamps decimal seconds (TIMESTAMP_TZ
//! adds the offset in minutes + 1440), binary is hex, VARIANT/OBJECT/ARRAY JSON text.

use chrono::{DateTime, Duration, NaiveDate, NaiveTime};
use serde_json::Value;

use super::client::RowType;
use crate::db::value::{self, ColumnMeta};

/// The display type of a result column (NUMBER(38,0), TIMESTAMP_NTZ…).
pub fn type_name(c: &RowType) -> String {
    let base = match c.kind.as_str() {
        "fixed" => "NUMBER",
        "real" => "FLOAT",
        "text" => "VARCHAR",
        other => return other.to_ascii_uppercase(),
    };
    match (c.kind.as_str(), c.precision, c.scale) {
        ("fixed", Some(p), Some(s)) => format!("{base}({p},{s})"),
        _ => base.to_string(),
    }
}

pub fn columns(cols: &[RowType]) -> Vec<ColumnMeta> {
    cols.iter()
        .map(|c| ColumnMeta {
            name: c.name.clone(),
            type_name: type_name(c),
        })
        .collect()
}

/// A row for the UI; `binary_limit` caps binary previews (`value::PREVIEW` / `FULL`).
pub fn row(cols: &[RowType], cells: &[Value], binary_limit: usize) -> Vec<Value> {
    cols.iter()
        .enumerate()
        .map(|(i, c)| match cells.get(i) {
            Some(Value::String(s)) => cell(c, s, binary_limit),
            Some(Value::Null) | None => Value::Null,
            Some(other) => other.clone(),
        })
        .collect()
}

pub fn cell(c: &RowType, s: &str, binary_limit: usize) -> Value {
    let scale = c.scale.unwrap_or(9).clamp(0, 9) as usize;
    let decoded = match c.kind.as_str() {
        "fixed" if c.scale.unwrap_or(0) == 0 => s.parse::<i64>().ok().map(value::int),
        "real" => s
            .parse::<f64>()
            .ok()
            .filter(|f| f.is_finite())
            .map(value::float),
        "boolean" => match s {
            "1" | "true" | "TRUE" => Some(Value::Bool(true)),
            "0" | "false" | "FALSE" => Some(Value::Bool(false)),
            _ => None,
        },
        "date" => s.parse::<i64>().ok().and_then(date).map(Value::String),
        "time" => nanos(s).and_then(|n| time(n, scale)).map(Value::String),
        "timestamp_ntz" => nanos(s)
            .and_then(|n| timestamp(n, scale))
            .map(Value::String),
        "timestamp_ltz" => nanos(s)
            .and_then(|n| timestamp(n, scale))
            .map(|t| Value::String(format!("{t} +00:00"))),
        "timestamp_tz" => timestamp_tz(s, scale).map(Value::String),
        "binary" => hex::decode(s)
            .ok()
            .map(|b| value::binary_with(&b, binary_limit)),
        "variant" | "object" | "array" => serde_json::from_str(s).ok(),
        _ => None,
    };
    decoded.unwrap_or_else(|| Value::String(s.to_string()))
}

/// Decimal seconds ("-1.500000000") as nanoseconds.
fn nanos(s: &str) -> Option<i128> {
    let (neg, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    if frac.len() > 9 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole: i128 = int.parse().ok()?;
    let frac_n: i128 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<9}").parse().ok()?
    };
    let n = whole * 1_000_000_000 + frac_n;
    Some(if neg { -n } else { n })
}

fn split(n: i128) -> (i64, u32) {
    let secs = n.div_euclid(1_000_000_000) as i64;
    (secs, n.rem_euclid(1_000_000_000) as u32)
}

fn fraction(nsub: u32, scale: usize) -> String {
    if scale == 0 {
        return String::new();
    }
    format!(".{:09}", nsub)[..scale + 1].to_string()
}

fn date(days: i64) -> Option<String> {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    let d = epoch.checked_add_signed(Duration::try_days(days)?)?;
    Some(d.format("%Y-%m-%d").to_string())
}

fn time(n: i128, scale: usize) -> Option<String> {
    let (secs, nsub) = split(n);
    let t = NaiveTime::from_num_seconds_from_midnight_opt(u32::try_from(secs).ok()?, 0)?;
    Some(format!("{}{}", t.format("%H:%M:%S"), fraction(nsub, scale)))
}

fn timestamp(n: i128, scale: usize) -> Option<String> {
    let (secs, nsub) = split(n);
    let t = DateTime::from_timestamp(secs, 0)?.naive_utc();
    Some(format!(
        "{}{}",
        t.format("%Y-%m-%d %H:%M:%S"),
        fraction(nsub, scale)
    ))
}

/// "secs.frac offset" where offset = minutes east of UTC + 1440.
fn timestamp_tz(s: &str, scale: usize) -> Option<String> {
    let (when, tz) = s.split_once(' ')?;
    let offset = tz.trim().parse::<i64>().ok()? - 1440;
    let local = nanos(when)? + offset as i128 * 60 * 1_000_000_000;
    let sign = if offset < 0 { '-' } else { '+' };
    let (h, m) = (offset.abs() / 60, offset.abs() % 60);
    Some(format!("{} {sign}{h:02}:{m:02}", timestamp(local, scale)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rt(kind: &str, scale: Option<i64>) -> RowType {
        RowType {
            name: "c".into(),
            kind: kind.into(),
            precision: Some(38),
            scale,
        }
    }

    #[test]
    fn numbers_keep_exactness() {
        let c = |k, s, v| cell(&rt(k, s), v, value::PREVIEW);
        assert_eq!(c("fixed", Some(0), "42"), json!(42));
        assert_eq!(
            c("fixed", Some(0), "99999999999999999999"),
            json!("99999999999999999999")
        );
        assert_eq!(c("fixed", Some(2), "1.50"), json!("1.50"));
        assert_eq!(c("real", None, "1.25"), json!(1.25));
        assert_eq!(c("real", None, "NaN"), json!("NaN"));
        assert_eq!(c("boolean", None, "1"), json!(true));
        assert_eq!(type_name(&rt("fixed", Some(2))), "NUMBER(38,2)");
    }

    #[test]
    fn dates_and_times() {
        let c = |k, s, v| cell(&rt(k, s), v, value::PREVIEW);
        assert_eq!(c("date", None, "19725"), json!("2024-01-03"));
        assert_eq!(c("time", Some(3), "3723.500000000"), json!("01:02:03.500"));
        assert_eq!(
            c("timestamp_ntz", Some(9), "1704164645.123456789"),
            json!("2024-01-02 03:04:05.123456789")
        );
        assert_eq!(
            c("timestamp_ntz", Some(1), "-0.500000000"),
            json!("1969-12-31 23:59:59.5")
        );
        assert_eq!(
            c("timestamp_tz", Some(0), "1704164645.000000000 1500"),
            json!("2024-01-02 04:04:05 +01:00")
        );
        assert_eq!(
            c("timestamp_ltz", Some(0), "0"),
            json!("1970-01-01 00:00:00 +00:00")
        );
    }

    #[test]
    fn binary_and_documents() {
        let b = cell(&rt("binary", None), "00ff", value::PREVIEW);
        assert_eq!(b, json!({ "$binary": "00ff", "size": 2 }));
        let v = cell(
            &rt("variant", None),
            "{\n  \"a\": [1, 2]\n}",
            value::PREVIEW,
        );
        assert_eq!(v, json!({ "a": [1, 2] }));
        assert_eq!(cell(&rt("text", None), "x", value::PREVIEW), json!("x"));
    }
}
