//! Exporting several tables at once (ExportTablesWindow): one file per table in a
//! folder, each streamed like a single-table export.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::db::data::PageRequest;
use crate::error::{AppError, AppResult};
use crate::state::Session;

use super::{ExportRequest, Format, Options, Source};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TablesRequest {
    pub schema: String,
    pub tables: Vec<String>,
    pub format: Format,
    pub folder: String,
    #[serde(default)]
    pub options: Options,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablesProgress {
    /// 0-based index of the table being written.
    pub index: usize,
    pub count: usize,
    pub table: String,
    /// Rows of the current table so far.
    pub rows: u64,
    /// Rows of all finished tables.
    pub total_rows: u64,
    /// Files written so far.
    pub files: Vec<String>,
}

fn extension(f: Format) -> &'static str {
    match f {
        Format::Csv => "csv",
        Format::Json => "json",
        Format::Sql => "sql",
    }
}

/// A file name for a table: characters that file systems reject become `_`.
pub fn file_name(table: &str, format: Format) -> String {
    let safe: String = table
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let safe = safe.trim_start_matches('.');
    let safe = if safe.is_empty() { "table" } else { safe };
    format!("{safe}.{}", extension(format))
}

fn table_request(req: &TablesRequest, table: &str, path: &Path) -> ExportRequest {
    let page = PageRequest {
        schema: req.schema.clone(),
        table: table.to_string(),
        page: 0,
        page_size: 0,
        sort: Vec::new(),
        filters: Vec::new(),
        apply_only: None,
    };
    let mut options = req.options.clone();
    // SQL INSERTs name the table they came from.
    options.table_name = table.to_string();
    ExportRequest {
        source: Source::Table {
            request: page,
            page_only: false,
        },
        format: req.format,
        path: path.display().to_string(),
        options,
    }
}

/// Exports every table in order; stops at the first failure or on cancel.
pub async fn run<F: Fn(&TablesProgress)>(
    session: &Session,
    req: &TablesRequest,
    cancel: &CancellationToken,
    progress: F,
) -> AppResult<TablesProgress> {
    if req.tables.is_empty() {
        return Err(AppError::validation("Choose at least one table to export."));
    }
    let folder = PathBuf::from(&req.folder);
    if !folder.is_dir() {
        return Err(AppError::validation("Choose an existing folder."));
    }
    let mut p = TablesProgress {
        count: req.tables.len(),
        ..Default::default()
    };
    for (index, table) in req.tables.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(AppError::cancelled());
        }
        let path = folder.join(file_name(table, req.format));
        (p.index, p.table, p.rows) = (index, table.clone(), 0);
        progress(&p);
        let one = table_request(req, table, &path);
        let base = p.clone();
        let rows = super::run(session, &one, cancel, |rows| {
            progress(&TablesProgress {
                rows,
                ..base.clone()
            })
        })
        .await?;
        p.total_rows += rows;
        p.files.push(path.display().to_string());
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("people", Format::Csv), "people.csv");
        assert_eq!(file_name("a/b:c", Format::Json), "a_b_c.json");
        assert_eq!(file_name("..", Format::Sql), "table.sql");
    }
}
