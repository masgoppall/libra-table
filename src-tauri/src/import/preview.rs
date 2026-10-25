//! The wizard's preview: column names, the first rows, and a type for each column
//! inferred from a sample of the file.

use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use serde::Serialize;

use crate::error::AppResult;
use crate::models::Driver;

use super::infer::{Infer, Kind};
use super::run::RowError;
use super::source::{self, CsvEvent, Record};
use super::{FileFormat, SourceSpec};

/// Rows shown in the preview grid.
const PREVIEW_ROWS: usize = 50;
/// Rows read to infer types.
const SAMPLE_ROWS: u64 = 1000;
const MAX_ERRORS: usize = 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceColumn {
    pub name: String,
    pub kind: Kind,
    /// A column type for the connection's driver (new tables).
    pub suggested_type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub columns: Vec<SourceColumn>,
    pub rows: Vec<Vec<Option<String>>>,
    /// Rows read for the type inference.
    pub sampled: u64,
    pub total_bytes: u64,
    /// Rows of the sample that could not be read.
    pub errors: Vec<RowError>,
}

/// Collects the sample: rows for display, a type per column, row errors.
#[derive(Default)]
struct Sample {
    names: Vec<String>,
    infer: Vec<Infer>,
    rows: Vec<Vec<Option<String>>>,
    sampled: u64,
    errors: Vec<RowError>,
}

impl Sample {
    fn add(&mut self, r: Record) -> bool {
        self.sampled += 1;
        match r.values {
            Ok(values) => {
                for (i, v) in values.iter().enumerate() {
                    if let Some(inf) = self.infer.get_mut(i) {
                        inf.add(v.as_deref());
                    }
                }
                if self.rows.len() < PREVIEW_ROWS {
                    self.rows.push(values);
                }
            }
            Err(message) if self.errors.len() < MAX_ERRORS => self.errors.push(RowError {
                row: r.row,
                line: r.line,
                message,
            }),
            Err(_) => {}
        }
        self.sampled < SAMPLE_ROWS
    }

    fn set_columns(&mut self, names: Vec<String>) {
        self.infer = vec![Infer::default(); names.len()];
        self.names = names;
    }

    fn finish(self, driver: Driver, total_bytes: u64) -> FilePreview {
        let columns = self
            .names
            .into_iter()
            .zip(self.infer)
            .map(|(name, inf)| SourceColumn {
                name,
                kind: inf.kind(),
                suggested_type: inf.sql_type(driver),
                nullable: inf.nulls > 0,
            })
            .collect();
        FilePreview {
            columns,
            rows: self.rows,
            sampled: self.sampled,
            total_bytes,
            errors: self.errors,
        }
    }
}

/// Reads the start of the file. Blocking: call it off the async runtime.
pub fn preview_file(spec: &SourceSpec, driver: Driver) -> AppResult<FilePreview> {
    let (reader, size) = source::open(&spec.path, Arc::new(AtomicU64::new(0)))?;
    let mut sample = Sample::default();
    match spec.format {
        FileFormat::Csv => source::read_csv(reader, &spec.csv, |ev| match ev {
            CsvEvent::Columns(names) => {
                sample.set_columns(names);
                true
            }
            CsvEvent::Row(r) => sample.add(r),
        })?,
        FileFormat::Json => json_sample(reader, &mut sample)?,
    }
    Ok(sample.finish(driver, size))
}

/// JSON has no header: the columns are the keys of the sampled objects, in the order
/// they first appear.
fn json_sample<R: std::io::Read>(reader: R, sample: &mut Sample) -> AppResult<()> {
    let mut objects: Vec<(u64, source::JsonRow)> = Vec::new();
    source::read_json(reader, |row, obj| {
        objects.push((row, obj));
        (objects.len() as u64) < SAMPLE_ROWS
    })?;
    let mut names: Vec<String> = Vec::new();
    for (_, obj) in &objects {
        for key in obj.iter().flat_map(|o| o.keys()) {
            if !names.contains(key) {
                names.push(key.clone());
            }
        }
    }
    sample.set_columns(names);
    for (row, obj) in objects {
        let values = obj.map(|o| source::json_values(&o, &sample.names));
        sample.add(Record {
            row,
            line: None,
            values,
        });
    }
    Ok(())
}
