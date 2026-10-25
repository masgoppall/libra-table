//! Importing data files (ImportCSVWizard / ImportJSONWizard) and running SQL files
//! (ImportSQLWizard).
//!
//! A blocking thread parses the file and streams records over a bounded channel, so a
//! file of any size uses little memory. The async side groups records into multi-row
//! INSERTs with bind parameters and runs them on one connection, in one transaction for
//! the whole run or one per batch. Every statement runs inside a savepoint: when a batch
//! fails it is retried row by row, so the bad rows can be reported (and skipped).

mod conn;
mod infer;
mod insert;
mod job;
mod preview;
mod run;
pub mod source;
pub mod sqlfile;
pub mod sqlrun;
#[cfg(test)]
mod tests;

pub use conn::Conn;
pub use infer::{Infer, Kind};
pub use insert::Target;
pub use preview::{preview_file, FilePreview, SourceColumn};
pub use run::{run, ImportProgress, ImportSummary, RowError};

use serde::{Deserialize, Serialize};

use crate::db::meta::{self, ColumnInfo};
use crate::error::AppResult;
use crate::state::Session;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    #[default]
    Csv,
    Json,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    #[default]
    Utf8,
    Latin1,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CsvOptions {
    /// One ASCII character (`\t` for tab).
    pub delimiter: String,
    /// One ASCII character, or empty for no quoting.
    pub quote: String,
    /// The first row (after the skipped ones) holds the column names.
    pub header: bool,
    pub encoding: Encoding,
    /// A field equal to this is NULL (empty: no marker).
    pub null_marker: String,
    pub empty_as_null: bool,
    /// Rows to skip at the start of the file, before the header.
    pub skip_rows: u64,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: ",".into(),
            quote: "\"".into(),
            header: true,
            encoding: Encoding::Utf8,
            null_marker: "NULL".into(),
            empty_as_null: true,
            skip_rows: 0,
        }
    }
}

/// The file to read and how.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpec {
    pub path: String,
    pub format: FileFormat,
    #[serde(default)]
    pub csv: CsvOptions,
}

/// Source column (index in the preview's columns) → target column name.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMap {
    pub source: usize,
    pub target: String,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    /// Stop at the first bad row and roll back what is not committed.
    #[default]
    Stop,
    /// Skip bad rows and report them.
    Skip,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TxScope {
    /// One transaction for the whole run: all or nothing.
    #[default]
    Run,
    /// Commit after every batch.
    Batch,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub source: SourceSpec,
    /// The preview's column names (JSON keys are matched by name).
    #[serde(default)]
    pub source_columns: Vec<String>,
    pub schema: String,
    pub table: String,
    pub mapping: Vec<ColumnMap>,
    #[serde(default = "default_batch")]
    pub batch_size: usize,
    #[serde(default)]
    pub on_error: OnError,
    #[serde(default)]
    pub transaction: TxScope,
}

fn default_batch() -> usize {
    500
}

/// A column of a table that does not exist yet (previewing "import into new table").
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewColumn {
    pub name: String,
    pub data_type: String,
}

/// The target of `req`: from the catalog, or from `new_columns` before the table exists.
pub async fn target(
    session: &Session,
    req: &ImportRequest,
    new_columns: Option<&[NewColumn]>,
) -> AppResult<Target> {
    let catalog = match new_columns {
        Some(cols) => cols.iter().map(new_column_info).collect(),
        None => meta::list_columns(&session.pool, &req.schema, &req.table).await?,
    };
    let names: Vec<String> = req.mapping.iter().map(|m| m.target.clone()).collect();
    Target::new(session.dialect(), &req.schema, &req.table, &names, &catalog)
}

fn new_column_info(c: &NewColumn) -> ColumnInfo {
    ColumnInfo {
        name: c.name.clone(),
        data_type: c.data_type.clone(),
        nullable: true,
        default: None,
        pk_position: None,
        auto_increment: false,
        comment: None,
    }
}

/// What the import will run, for the confirmation on production connections.
pub fn statements(target: &Target, req: &ImportRequest) -> Vec<String> {
    let per = target.rows_per_statement(req.batch_size);
    let scope = match req.transaction {
        TxScope::Run => "one transaction for the whole file".to_string(),
        TxScope::Batch => format!("a transaction per batch of {} rows", req.batch_size),
    };
    let on_error = match req.on_error {
        OnError::Stop => "stop and roll back",
        OnError::Skip => "skip the row",
    };
    vec![
        format!("-- Up to {per} rows per statement, {scope}; on error: {on_error}"),
        target.template(per),
    ]
}
