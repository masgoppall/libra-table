//! Column type inference for "import into a new table": every sampled value is tested
//! against the kinds that are still possible, and the narrowest one left wins.
//! Numbers with leading zeros (zip codes, ids like `007`) stay text.

use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde::Serialize;

use crate::models::Dialect;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Integer,
    BigInt,
    Decimal,
    Double,
    Boolean,
    Date,
    Timestamp,
    TimestampTz,
    Json,
    Text,
}

/// What one column's values allow so far.
#[derive(Debug, Clone)]
pub struct Infer {
    int: bool,
    bigint: bool,
    decimal: bool,
    double: bool,
    boolean: bool,
    date: bool,
    timestamp: bool,
    tz: bool,
    json: bool,
    values: u64,
    pub nulls: u64,
    max_len: usize,
    int_digits: usize,
    scale: usize,
}

impl Default for Infer {
    fn default() -> Self {
        Self {
            int: true,
            bigint: true,
            decimal: true,
            double: true,
            boolean: true,
            date: true,
            timestamp: true,
            tz: false,
            json: true,
            values: 0,
            nulls: 0,
            max_len: 0,
            int_digits: 0,
            scale: 0,
        }
    }
}

const BOOLS: &[&str] = &["true", "false", "t", "f", "yes", "no"];

const TIMESTAMP_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%d %H:%M",
    "%Y-%m-%dT%H:%M",
];

/// `-12.50` → Some((2, 2)): digits before and after the point. None if not plain decimal.
fn decimal_digits(v: &str) -> Option<(usize, usize)> {
    let body = v.strip_prefix('-').unwrap_or(v);
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if int.is_empty() || !digits(int) || !digits(frac) || (body.contains('.') && frac.is_empty()) {
        return None;
    }
    if int.len() > 1 && int.starts_with('0') {
        return None;
    }
    Some((int.trim_start_matches('0').len().max(1), frac.len()))
}

fn is_integer(v: &str) -> bool {
    decimal_digits(v).is_some_and(|(_, s)| s == 0) && !v.contains('.')
}

fn is_double(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    lower.contains('e') && !lower.starts_with('e') && v.parse::<f64>().is_ok_and(f64::is_finite)
}

fn is_timestamp(v: &str) -> bool {
    TIMESTAMP_FORMATS
        .iter()
        .any(|f| NaiveDateTime::parse_from_str(v, f).is_ok())
}

fn is_timestamp_tz(v: &str) -> bool {
    DateTime::parse_from_rfc3339(v).is_ok()
        || DateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S%.f%#z").is_ok()
}

fn is_json(v: &str) -> bool {
    let t = v.trim_start();
    (t.starts_with('{') || t.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(v).is_ok()
}

impl Infer {
    pub fn add(&mut self, value: Option<&str>) {
        let Some(v) = value else {
            self.nulls += 1;
            return;
        };
        self.values += 1;
        self.max_len = self.max_len.max(v.chars().count());
        let dec = decimal_digits(v);
        if let Some((i, s)) = dec {
            self.int_digits = self.int_digits.max(i);
            self.scale = self.scale.max(s);
        }
        let integer = is_integer(v);
        self.int &= integer && v.parse::<i32>().is_ok();
        self.bigint &= integer && v.parse::<i64>().is_ok();
        self.decimal &= dec.is_some();
        self.double &= dec.is_some() || is_double(v);
        self.boolean &= BOOLS.contains(&v.to_ascii_lowercase().as_str());
        self.date &= NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok();
        let tz = self.timestamp && is_timestamp_tz(v);
        self.timestamp &= tz || is_timestamp(v);
        self.tz |= tz;
        self.json &= is_json(v);
    }

    /// The narrowest kind every value fits (text when there were no values).
    pub fn kind(&self) -> Kind {
        if self.values == 0 {
            return Kind::Text;
        }
        let picks = [
            (self.int, Kind::Integer),
            (self.bigint, Kind::BigInt),
            (
                self.decimal && self.int_digits + self.scale <= 38,
                Kind::Decimal,
            ),
            (self.double, Kind::Double),
            (self.boolean, Kind::Boolean),
            (self.date, Kind::Date),
            (self.timestamp && self.tz, Kind::TimestampTz),
            (self.timestamp, Kind::Timestamp),
            (self.json, Kind::Json),
        ];
        picks
            .into_iter()
            .find(|(ok, _)| *ok)
            .map(|(_, k)| k)
            .unwrap_or(Kind::Text)
    }

    /// A column type for `driver`, from the names the table designer suggests.
    pub fn sql_type(&self, driver: Dialect) -> String {
        let kind = self.kind();
        let decimal = format!(
            "decimal({},{})",
            (self.int_digits + self.scale).max(1),
            self.scale
        );
        let s = |pg: &str, my: &str, lite: &str| match driver {
            Dialect::Postgres => pg.to_string(),
            Dialect::Mysql => my.to_string(),
            Dialect::Sqlite => lite.to_string(),
        };
        match kind {
            Kind::Integer => s("integer", "int", "INTEGER"),
            Kind::BigInt => s("bigint", "bigint", "INTEGER"),
            Kind::Decimal => s(&format!("numeric{}", &decimal[7..]), &decimal, "NUMERIC"),
            Kind::Double => s("double precision", "double", "REAL"),
            Kind::Boolean => s("boolean", "tinyint(1)", "BOOLEAN"),
            Kind::Date => s("date", "date", "DATE"),
            Kind::Timestamp => s("timestamp", "datetime", "DATETIME"),
            Kind::TimestampTz => s("timestamptz", "datetime", "DATETIME"),
            Kind::Json => s("jsonb", "json", "TEXT"),
            Kind::Text if self.max_len <= 255 => s("text", "varchar(255)", "TEXT"),
            Kind::Text => s("text", "longtext", "TEXT"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn infer(values: &[Option<&str>]) -> Infer {
        let mut i = Infer::default();
        for v in values {
            i.add(*v);
        }
        i
    }

    fn kind(values: &[&str]) -> Kind {
        infer(&values.iter().map(|v| Some(*v)).collect::<Vec<_>>()).kind()
    }

    #[test]
    fn picks_the_narrowest_kind() {
        assert_eq!(kind(&["1", "-20", "300"]), Kind::Integer);
        assert_eq!(kind(&["1", "9000000000"]), Kind::BigInt);
        assert_eq!(kind(&["1.5", "20", "-0.25"]), Kind::Decimal);
        assert_eq!(kind(&["1e3", "2.5"]), Kind::Double);
        assert_eq!(kind(&["true", "False", "yes"]), Kind::Boolean);
        assert_eq!(kind(&["2024-01-31", "1999-12-01"]), Kind::Date);
        assert_eq!(
            kind(&["2024-01-31 10:00:00", "2024-02-01T08:30"]),
            Kind::Timestamp
        );
        assert_eq!(
            kind(&["2024-01-31T10:00:00Z", "2024-01-31 10:00:00"]),
            Kind::TimestampTz
        );
        assert_eq!(kind(&["{\"a\":1}", "[1,2]"]), Kind::Json);
        assert_eq!(kind(&["1", "x"]), Kind::Text);
        assert_eq!(kind(&["2024-02-30"]), Kind::Text);
    }

    #[test]
    fn leading_zeros_stay_text() {
        assert_eq!(kind(&["007", "123"]), Kind::Text);
        assert_eq!(kind(&["0", "0.5"]), Kind::Decimal);
    }

    #[test]
    fn nulls_are_ignored_but_counted() {
        let i = infer(&[None, Some("4"), None]);
        assert_eq!((i.kind(), i.nulls), (Kind::Integer, 2));
        assert_eq!(infer(&[None]).kind(), Kind::Text);
    }

    #[test]
    fn types_per_driver() {
        let money = infer(&[Some("12.50"), Some("1234.5")]);
        assert_eq!(money.sql_type(Dialect::Mysql), "decimal(6,2)");
        assert_eq!(money.sql_type(Dialect::Postgres), "numeric(6,2)");
        assert_eq!(money.sql_type(Dialect::Sqlite), "NUMERIC");
        let long = "x".repeat(300);
        assert_eq!(infer(&[Some(&long)]).sql_type(Dialect::Mysql), "longtext");
        assert_eq!(
            infer(&[Some("ok")]).sql_type(Dialect::Mysql),
            "varchar(255)"
        );
        assert_eq!(infer(&[Some("1")]).sql_type(Dialect::Postgres), "integer");
    }
}
