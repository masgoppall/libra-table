//! Reading import files as a stream of records. CSV goes through the `csv` crate (any
//! one-byte delimiter and quote, UTF-8 or Latin-1); JSON is an array of objects or one
//! object per line. Nothing is loaded whole: records are handed to a callback, which
//! returns false to stop early.

use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::de::{SeqAccess, Visitor};
use serde::Deserializer;
use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

use super::{CsvOptions, Encoding};

/// One data row of the file. `row` counts data rows from 1; `line` is the file line
/// where a CSV record starts.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub row: u64,
    pub line: Option<u64>,
    /// The values in source column order, or why the row could not be read.
    pub values: Result<Vec<Option<String>>, String>,
}

/// What the CSV reader reports: the column names once, then the rows.
#[derive(Debug, PartialEq)]
pub enum CsvEvent {
    Columns(Vec<String>),
    Row(Record),
}

/// Counts the bytes read so far, for progress.
pub struct Counting<R> {
    inner: R,
    read: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Opens `path` for streaming. Returns the reader and the file size.
pub fn open(path: &str, read: Arc<AtomicU64>) -> AppResult<(Counting<BufReader<File>>, u64)> {
    let file = File::open(path)
        .map_err(|e| AppError::from(e).hint("Check that the file still exists and can be read."))?;
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let inner = BufReader::with_capacity(256 * 1024, file);
    Ok((Counting { inner, read }, size))
}

fn one_byte(value: &str, what: &str) -> AppResult<Option<u8>> {
    match value.as_bytes() {
        [] => Ok(None),
        [b] if b.is_ascii() => Ok(Some(*b)),
        _ => Err(AppError::validation(format!(
            "The {what} must be a single ASCII character."
        ))),
    }
}

fn decode(bytes: &[u8], encoding: Encoding) -> Result<String, String> {
    match encoding {
        Encoding::Utf8 => String::from_utf8(bytes.to_vec())
            .map_err(|_| "The row is not valid UTF-8 (try the Latin-1 encoding).".to_string()),
        // Latin-1 maps every byte to the code point of the same number.
        Encoding::Latin1 => Ok(bytes.iter().map(|&b| b as char).collect()),
    }
}

/// Applies the NULL rules to one CSV field.
fn field(text: String, o: &CsvOptions) -> Option<String> {
    let is_null = (o.empty_as_null && text.is_empty())
        || (!o.null_marker.is_empty() && text == o.null_marker);
    (!is_null).then_some(text)
}

/// Column names from the header row: trimmed, blanks named `column_N`, duplicates
/// numbered.
pub fn column_names(raw: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    for (i, name) in raw.into_iter().enumerate() {
        let base = name.trim().trim_start_matches('\u{feff}').to_string();
        let base = if base.is_empty() {
            format!("column_{}", i + 1)
        } else {
            base
        };
        let mut name = base.clone();
        let mut n = 2;
        while out.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        out.push(name);
    }
    out
}

/// Reads CSV from `reader`, calling `f` with the columns and then each row.
pub fn read_csv<R: Read>(
    reader: R,
    o: &CsvOptions,
    mut f: impl FnMut(CsvEvent) -> bool,
) -> AppResult<()> {
    let delimiter = one_byte(&o.delimiter, "delimiter")?.unwrap_or(b',');
    let mut b = csv::ReaderBuilder::new();
    b.delimiter(delimiter).has_headers(false).flexible(true);
    match one_byte(&o.quote, "quote character")? {
        Some(q) => b.quote(q),
        None => b.quoting(false),
    };
    let mut rdr = b.from_reader(reader);
    let mut rec = csv::ByteRecord::new();
    let (mut skipped, mut width, mut row) = (0u64, None::<usize>, 0u64);
    loop {
        match rdr.read_byte_record(&mut rec) {
            Ok(false) => return Ok(()),
            Ok(true) => {}
            Err(e) => return Err(csv_error(e)),
        }
        if skipped < o.skip_rows {
            skipped += 1;
            continue;
        }
        let line = rec.position().map(|p| p.line());
        if width.is_none() {
            width = Some(rec.len());
            if !f(CsvEvent::Columns(header_names(&rec, o.encoding, o.header))) {
                return Ok(());
            }
            if o.header {
                continue;
            }
        }
        row += 1;
        let w = width.unwrap_or(rec.len());
        if !f(CsvEvent::Row(csv_record(&rec, o, w, row, line))) {
            return Ok(());
        }
    }
}

fn header_names(rec: &csv::ByteRecord, encoding: Encoding, header: bool) -> Vec<String> {
    if !header {
        return (1..=rec.len()).map(|i| format!("column_{i}")).collect();
    }
    let raw = rec
        .iter()
        .map(|b| decode(b, encoding).unwrap_or_else(|_| String::from_utf8_lossy(b).into()))
        .collect();
    column_names(raw)
}

fn csv_record(
    rec: &csv::ByteRecord,
    o: &CsvOptions,
    width: usize,
    row: u64,
    line: Option<u64>,
) -> Record {
    let values = if rec.len() != width {
        Err(format!(
            "The row has {} fields; the header has {width}.",
            rec.len()
        ))
    } else {
        rec.iter()
            .map(|b| decode(b, o.encoding).map(|t| field(t, o)))
            .collect()
    };
    Record { row, line, values }
}

fn csv_error(e: csv::Error) -> AppError {
    let at = e
        .position()
        .map(|p| format!(" (line {})", p.line()))
        .unwrap_or_default();
    AppError::validation(format!("Could not read the CSV file{at}: {e}"))
}

/// One element of a JSON file: an object, or why the element isn't one.
pub type JsonRow = Result<Map<String, Value>, String>;

/// Reads a JSON array of objects, or objects separated by whitespace (NDJSON).
pub fn read_json<R: Read>(reader: R, mut f: impl FnMut(u64, JsonRow) -> bool) -> AppResult<()> {
    let mut reader = std::io::BufReader::new(reader);
    let first = first_byte(&mut reader)?;
    let mut row = 0u64;
    let mut emit = |v: Value| {
        row += 1;
        let item = match v {
            Value::Object(m) => Ok(m),
            other => Err(format!("Expected an object, found {}.", kind_name(&other))),
        };
        f(row, item)
    };
    let json_err =
        |e: serde_json::Error| AppError::validation(format!("The JSON file is not valid: {e}"));
    match first {
        None => Ok(()),
        Some(b'[') => {
            let mut de = serde_json::Deserializer::from_reader(reader);
            let r = de.deserialize_seq(Elements { f: &mut emit });
            match r {
                Err(e) if e.to_string().contains(STOPPED) => Ok(()),
                other => other.map_err(json_err),
            }
        }
        Some(_) => {
            for v in serde_json::Deserializer::from_reader(reader).into_iter::<Value>() {
                if !emit(v.map_err(json_err)?) {
                    break;
                }
            }
            Ok(())
        }
    }
}

/// Peeks at the first byte that is not whitespace or a UTF-8 BOM, consuming the BOM.
fn first_byte<R: std::io::BufRead>(r: &mut R) -> AppResult<Option<u8>> {
    loop {
        let buf = r.fill_buf()?;
        let Some(&b) = buf.first() else {
            return Ok(None);
        };
        if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
            r.consume(3);
        } else if b.is_ascii_whitespace() {
            r.consume(1);
        } else {
            return Ok(Some(b));
        }
    }
}

fn kind_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Visits the elements of the top-level array one by one, so it is never held whole.
struct Elements<'a, F: FnMut(Value) -> bool> {
    f: &'a mut F,
}

impl<'de, F: FnMut(Value) -> bool> Visitor<'de> for Elements<'_, F> {
    type Value = ();

    fn expecting(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
        out.write_str("an array of objects")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(v) = seq.next_element::<Value>()? {
            if !(self.f)(v) {
                // Stopping early: drain nothing, the caller drops the reader.
                return Err(serde::de::Error::custom(STOPPED));
            }
        }
        Ok(())
    }
}

/// Marks an early stop requested by the callback (not a real error).
const STOPPED: &str = "libratable: stopped";

/// A JSON value as text for a column: nested objects and arrays stay JSON.
pub fn json_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

/// An object's values in the order of `columns` (missing keys are NULL).
pub fn json_values(obj: &Map<String, Value>, columns: &[String]) -> Vec<Option<String>> {
    columns
        .iter()
        .map(|c| obj.get(c).and_then(json_text))
        .collect()
}
