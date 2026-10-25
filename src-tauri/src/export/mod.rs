//! Streaming export (EXP-1..EXP-4): rows go from the database straight to the file,
//! so memory stays flat for millions of rows. Progress is reported every few thousand
//! rows and the job can be cancelled.

mod sink;
pub mod tables;

pub use sink::sql_literal;

use std::path::Path;

use futures::TryStreamExt;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::db::data::{select_sql, PageRequest};
use crate::db::value::{self, ColumnMeta};
use crate::db::{bind_all, DbPool};
use crate::error::{AppError, AppResult};
use crate::state::Session;
use sink::Sink;

const PROGRESS_EVERY: u64 = 2000;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// One SQL statement (a query result).
    Query { sql: String },
    /// A table, with the grid's filters/sort. `page_only` exports just the current page.
    Table {
        #[serde(flatten)]
        request: PageRequest,
        #[serde(default, rename = "pageOnly")]
        page_only: bool,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Csv,
    Json,
    Sql,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Options {
    pub delimiter: String,
    pub quote: String,
    pub header: bool,
    /// UTF-8 BOM so Excel detects the encoding.
    pub bom: bool,
    /// "empty" | "null": how NULL is written in CSV.
    pub null_as: String,
    /// Target table name for SQL INSERT statements.
    pub table_name: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delimiter: ",".into(),
            quote: "\"".into(),
            header: true,
            bom: false,
            null_as: "empty".into(),
            table_name: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub source: Source,
    pub format: Format,
    pub path: String,
    #[serde(default)]
    pub options: Options,
}

/// The query an export streams.
struct Plan {
    sql: String,
    binds: Vec<String>,
    /// Known up front for tables; read from the first row for queries.
    columns: Option<Vec<ColumnMeta>>,
    /// Table name for SQL INSERT statements when the options don't give one.
    default_table: String,
}

async fn plan(session: &Session, source: &Source) -> AppResult<Plan> {
    match source {
        Source::Query { sql } => {
            let stmts = crate::db::split::split(session.driver(), sql);
            if stmts.len() != 1 {
                return Err(AppError::validation("Export one statement at a time."));
            }
            Ok(Plan {
                sql: stmts[0].text.clone(),
                binds: Vec::new(),
                columns: None,
                default_table: "exported".into(),
            })
        }
        Source::Table { request, page_only } => {
            let cols = session.columns(&request.schema, &request.table).await?;
            let (sql, binds) = select_sql(session, &cols, request, *page_only)?;
            let metas = cols
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name.clone(),
                    type_name: c.data_type.clone(),
                })
                .collect();
            Ok(Plan {
                sql,
                binds,
                columns: Some(metas),
                default_table: request.table.clone(),
            })
        }
    }
}

/// Runs the export. `progress(rows)` is called periodically and once at the end.
pub async fn run<F: Fn(u64)>(
    session: &Session,
    req: &ExportRequest,
    cancel: &CancellationToken,
    progress: F,
) -> AppResult<u64> {
    let plan = plan(session, &req.source).await?;
    let path = Path::new(&req.path);
    let driver = session.driver();
    let mut sink = Sink::open(path, req.format, &req.options, driver, &plan.default_table)?;
    let mut writer = Writer {
        sink: &mut sink,
        header: req.options.header,
        wrote_header: false,
        count: 0,
        cancel,
        progress: &progress,
    };
    if let Some(cols) = &plan.columns {
        writer.header(cols)?;
    }
    let result = stream(session, &plan, &mut writer).await;
    let count = writer.count;
    if let Err(e) = result {
        drop(sink);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    sink.finish()?;
    progress(count);
    Ok(count)
}

/// Writes rows to the sink, counting them and reporting progress.
struct Writer<'a, F: Fn(u64)> {
    sink: &'a mut Sink,
    header: bool,
    wrote_header: bool,
    count: u64,
    cancel: &'a CancellationToken,
    progress: &'a F,
}

impl<F: Fn(u64)> Writer<'_, F> {
    fn header(&mut self, cols: &[ColumnMeta]) -> AppResult<()> {
        self.sink.header(cols, self.header)?;
        self.wrote_header = true;
        Ok(())
    }

    fn row<R: sqlx::Row>(
        &mut self,
        row: &R,
        decode: fn(&R, usize) -> Vec<serde_json::Value>,
    ) -> AppResult<()> {
        if self.cancel.is_cancelled() {
            return Err(AppError::cancelled());
        }
        if !self.wrote_header {
            self.header(&value::columns(row))?;
        }
        self.sink.row(&decode(row, value::FULL))?;
        self.count += 1;
        if self.count % PROGRESS_EVERY == 0 {
            (self.progress)(self.count);
        }
        Ok(())
    }
}

/// Streams the plan's rows into `writer`.
async fn stream<F: Fn(u64)>(
    session: &Session,
    plan: &Plan,
    writer: &mut Writer<'_, F>,
) -> AppResult<()> {
    // A user query may not be preparable (e.g. SHOW on MySQL): use the simple protocol.
    let simple = plan.binds.is_empty() && plan.columns.is_none();
    macro_rules! stream_rows {
        ($pool:expr, $decode:path) => {{
            let mut rows = if simple {
                Box::pin(sqlx::raw_sql(&plan.sql).fetch($pool))
                    as std::pin::Pin<Box<dyn futures::Stream<Item = _> + Send + '_>>
            } else {
                Box::pin(bind_all!(sqlx::query(&plan.sql), &plan.binds).fetch($pool))
            };
            while let Some(row) = rows.try_next().await? {
                writer.row(&row, $decode)?;
            }
        }};
    }
    match &session.pool {
        DbPool::Postgres(p) => stream_rows!(p, value::pg::row),
        DbPool::Mysql(p) => stream_rows!(p, value::mysql::row),
        DbPool::Sqlite(p) => stream_rows!(p, value::sqlite::row),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_deserializes_from_ui_shape() {
        let s: Source = serde_json::from_value(json!({
            "kind": "table", "pageOnly": true, "schema": "public", "table": "t",
            "page": 2, "pageSize": 50, "sort": [], "filters": []
        }))
        .unwrap();
        match s {
            Source::Table { request, page_only } => {
                assert!(page_only);
                assert_eq!(
                    (request.table.as_str(), request.page, request.page_size),
                    ("t", 2, 50)
                );
            }
            _ => panic!("expected table source"),
        }
        let q: Source =
            serde_json::from_value(json!({"kind": "query", "sql": "select 1"})).unwrap();
        assert!(matches!(q, Source::Query { .. }));
    }
}
