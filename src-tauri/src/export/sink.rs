//! File writers for each export format, and how values are written in them.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use serde_json::Value;

use crate::db::sql::{literal, quote_ident};
use crate::db::value::ColumnMeta;
use crate::error::{AppError, AppResult, ErrorKind, Layer};
use crate::models::Dialect;

use super::{Format, Options};

const BUFFER_SIZE: usize = 1 << 20;
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

pub(super) enum Sink {
    Csv {
        w: Box<csv::Writer<BufWriter<File>>>,
        null: String,
    },
    Json {
        w: BufWriter<File>,
        first: bool,
        cols: Vec<String>,
    },
    Sql {
        w: BufWriter<File>,
        driver: Dialect,
        table: String,
        cols: Vec<String>,
    },
}

impl Sink {
    pub fn open(
        path: &Path,
        format: Format,
        opts: &Options,
        driver: Dialect,
        default_table: &str,
    ) -> AppResult<Self> {
        let file = File::create(path)?;
        let buf = BufWriter::with_capacity(BUFFER_SIZE, file);
        match format {
            Format::Csv => Self::csv(buf, opts),
            Format::Json => Self::json(buf),
            Format::Sql => Ok(Self::sql(buf, opts, driver, default_table)),
        }
    }

    fn csv(mut buf: BufWriter<File>, opts: &Options) -> AppResult<Self> {
        if opts.bom {
            buf.write_all(UTF8_BOM)?;
        }
        let delimiter = opts.delimiter.as_bytes().first().copied().unwrap_or(b',');
        let quote = opts.quote.as_bytes().first().copied().unwrap_or(b'"');
        let w = csv::WriterBuilder::new()
            .delimiter(delimiter)
            .quote(quote)
            .from_writer(buf);
        let null = if opts.null_as == "null" {
            "NULL".into()
        } else {
            String::new()
        };
        Ok(Sink::Csv {
            w: Box::new(w),
            null,
        })
    }

    fn json(mut buf: BufWriter<File>) -> AppResult<Self> {
        buf.write_all(b"[")?;
        Ok(Sink::Json {
            w: buf,
            first: true,
            cols: Vec::new(),
        })
    }

    fn sql(buf: BufWriter<File>, opts: &Options, driver: Dialect, default_table: &str) -> Self {
        let table = if opts.table_name.trim().is_empty() {
            default_table.to_string()
        } else {
            opts.table_name.trim().to_string()
        };
        Sink::Sql {
            w: buf,
            driver,
            table,
            cols: Vec::new(),
        }
    }

    pub fn header(&mut self, cols: &[ColumnMeta], write_header: bool) -> AppResult<()> {
        let names: Vec<String> = cols.iter().map(|c| c.name.clone()).collect();
        match self {
            Sink::Csv { w, .. } => {
                if write_header {
                    w.write_record(&names).map_err(csv_err)?;
                }
            }
            Sink::Json { cols, .. } | Sink::Sql { cols, .. } => *cols = names,
        }
        Ok(())
    }

    pub fn row(&mut self, values: &[Value]) -> AppResult<()> {
        match self {
            Sink::Csv { w, null } => {
                let rec: Vec<String> = values.iter().map(|v| csv_text(v, null)).collect();
                w.write_record(&rec).map_err(csv_err)?;
            }
            Sink::Json { w, first, cols } => {
                if !*first {
                    w.write_all(b",")?;
                }
                *first = false;
                w.write_all(b"\n  ")?;
                let mut obj = serde_json::Map::with_capacity(cols.len());
                for (c, v) in cols.iter().zip(values) {
                    obj.insert(c.clone(), v.clone());
                }
                serde_json::to_writer(&mut *w, &Value::Object(obj))?;
            }
            Sink::Sql {
                w,
                driver,
                table,
                cols,
            } => {
                let names: Vec<String> = cols.iter().map(|c| quote_ident(*driver, c)).collect();
                let vals: Vec<String> = values.iter().map(|v| sql_literal(*driver, v)).collect();
                writeln!(
                    w,
                    "INSERT INTO {} ({}) VALUES ({});",
                    quote_table(*driver, table),
                    names.join(", "),
                    vals.join(", ")
                )?;
            }
        }
        Ok(())
    }

    pub fn finish(self) -> AppResult<()> {
        match self {
            Sink::Csv { mut w, .. } => w.flush()?,
            Sink::Json { mut w, first, .. } => {
                w.write_all(if first { b"]\n" } else { b"\n]\n" })?;
                w.flush()?;
            }
            Sink::Sql { mut w, .. } => w.flush()?,
        }
        Ok(())
    }
}

fn csv_err(e: csv::Error) -> AppError {
    AppError::new(ErrorKind::Io, Layer::App, format!("CSV write failed: {e}"))
}

fn quote_table(driver: Dialect, name: &str) -> String {
    // "schema.table" is written as two quoted parts.
    match name.split_once('.') {
        Some((s, t)) => format!("{}.{}", quote_ident(driver, s), quote_ident(driver, t)),
        None => quote_ident(driver, name),
    }
}

fn hex_of(v: &Value) -> Option<&str> {
    v.as_object()
        .and_then(|o| o.get("$binary"))
        .and_then(Value::as_str)
}

fn csv_text(v: &Value, null: &str) -> String {
    match v {
        Value::Null => null.to_string(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => match hex_of(other) {
            Some(h) => format!("0x{h}"),
            None => other.to_string(),
        },
    }
}

pub fn sql_literal(driver: Dialect, v: &Value) -> String {
    match v {
        Value::Null => "NULL".into(),
        Value::Bool(b) => {
            if *b {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => match driver {
            // MySQL treats backslash as an escape character in string literals by default.
            Dialect::Mysql => literal(&s.replace('\\', "\\\\")),
            _ => literal(s),
        },
        other => match hex_of(other) {
            Some(h) => match driver {
                Dialect::Postgres => format!("'\\x{h}'::bytea"),
                _ => format!("X'{h}'"),
            },
            None => literal(&other.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(name: &str, type_name: &str) -> ColumnMeta {
        ColumnMeta {
            name: name.into(),
            type_name: type_name.into(),
        }
    }

    #[test]
    fn literals_per_driver() {
        assert_eq!(sql_literal(Dialect::Postgres, &json!("it's")), "'it''s'");
        assert_eq!(sql_literal(Dialect::Mysql, &json!("a\\b")), "'a\\\\b'");
        assert_eq!(sql_literal(Dialect::Sqlite, &json!(null)), "NULL");
        assert_eq!(
            sql_literal(Dialect::Postgres, &json!({"$binary": "beef", "size": 2})),
            "'\\xbeef'::bytea"
        );
        assert_eq!(
            sql_literal(Dialect::Mysql, &json!({"$binary": "beef", "size": 2})),
            "X'beef'"
        );
        assert_eq!(csv_text(&json!(null), "NULL"), "NULL");
        assert_eq!(csv_text(&json!(12.5), ""), "12.5");
        assert_eq!(
            quote_table(Dialect::Mysql, "shop.orders"),
            "`shop`.`orders`"
        );
    }

    #[test]
    fn json_sink_writes_valid_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.json");
        let opts = Options::default();
        let mut s = Sink::open(&path, Format::Json, &opts, Dialect::Sqlite, "t").unwrap();
        s.header(&[meta("a", "INTEGER")], true).unwrap();
        s.row(&[json!(1)]).unwrap();
        s.row(&[json!(null)]).unwrap();
        s.finish().unwrap();
        let parsed: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed, json!([{"a": 1}, {"a": null}]));
    }

    #[test]
    fn csv_sink_with_bom_and_custom_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.csv");
        let opts = Options {
            delimiter: ";".into(),
            bom: true,
            null_as: "null".into(),
            ..Default::default()
        };
        let mut s = Sink::open(&path, Format::Csv, &opts, Dialect::Sqlite, "t").unwrap();
        s.header(&[meta("a", ""), meta("b", "")], true).unwrap();
        s.row(&[json!("x;y"), json!(null)]).unwrap();
        s.finish().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(UTF8_BOM));
        assert_eq!(String::from_utf8_lossy(&bytes[3..]), "a;b\n\"x;y\";NULL\n");
    }
}
