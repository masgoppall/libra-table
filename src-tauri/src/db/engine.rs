//! Drivers that don't run on sqlx (SQL Server, ClickHouse, MongoDB, Redis…) implement
//! `Engine`; a session holds one as `DbPool::Engine`. Commands call it where the built-in
//! drivers run their own SQL. A method a driver can't do answers `unsupported`, and the
//! driver's `Features` (src/drivers.rs) keep the UI from offering it.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Map, Value};

use crate::db::cell::CellContent;
use crate::db::changes::Change;
use crate::db::data::{FilterPreview, PageRequest, RowCount};
use crate::db::exec::StatementResult;
use crate::db::meta::{ColumnInfo, ObjectInfo, TableStructure};
use crate::db::value::ColumnMeta;
use crate::db::Target;
use crate::error::{AppError, AppResult};
use crate::models::{Connection, Driver};

/// Page size used when an export reads a table page by page.
const EXPORT_PAGE: u32 = 5000;
/// Rows an exported query may return when the engine reads it in one go.
const EXPORT_QUERY_LIMIT: usize = 1_000_000;

/// What opening an engine needs: the saved settings, the tunnel's local end (if any),
/// the secret from the keychain or a prompt, and the database to work in.
pub struct OpenArgs<'a> {
    pub conn: &'a Connection,
    pub target: Option<&'a Target>,
    /// Password, API token, secret access key or service-account key, per driver.
    pub secret: Option<&'a str>,
    /// Database to open instead of the saved one (tabs of another database).
    pub database: Option<&'a str>,
    pub max_connections: u32,
}

impl OpenArgs<'_> {
    /// Host and port to dial: the tunnel's local end, or the saved host and port.
    pub fn host_port(&self) -> (String, u16) {
        match self.target {
            Some(t) => (t.host.clone(), t.port),
            None => (self.conn.db_host(), self.conn.db_port()),
        }
    }

    pub fn through_tunnel(&self) -> bool {
        self.target.map(|t| t.through_tunnel).unwrap_or(false)
    }

    /// The database to open: the requested one, else the saved one ("" for none).
    pub fn database(&self) -> String {
        self.database
            .map(str::to_string)
            .unwrap_or_else(|| self.conn.db.database.trim().to_string())
    }
}

/// A table a grid call works on, with its columns (cached by the session).
#[derive(Clone, Copy)]
pub struct TableRef<'a> {
    pub schema: &'a str,
    pub table: &'a str,
    pub columns: &'a [ColumnInfo],
}

/// One page of grid rows and the query that produced it (shown under the grid).
pub struct PageRows {
    pub rows: Vec<Vec<Value>>,
    pub query: String,
}

/// Where exported rows go. An error stops the export (cancelled, disk full…).
pub trait RowSink: Send {
    fn columns(&mut self, columns: &[ColumnMeta]) -> AppResult<()>;
    fn row(&mut self, values: Vec<Value>) -> AppResult<()>;
}

/// For code paths only the built-in sqlx drivers have (DDL, server tools, imports…);
/// the UI hides them for engine drivers, so reaching one is a bug or an old UI.
pub fn sqlx_only() -> AppError {
    AppError::unsupported("This is only available for the built-in SQL drivers.")
}

pub fn unsupported(driver: Driver, what: &str) -> AppError {
    AppError::unsupported(format!(
        "{} doesn't support {what} in LibraTable.",
        driver.label()
    ))
}

#[async_trait]
pub trait Engine: Send + Sync {
    fn driver(&self) -> Driver;
    /// The concrete engine, for driver-specific commands (`commands::redis`).
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
    async fn server_version(&self) -> AppResult<String>;
    /// The database the engine works in ("" when the driver has none).
    async fn current_database(&self) -> AppResult<String> {
        Ok(String::new())
    }
    async fn close(&self);
    fn is_closed(&self) -> bool {
        false
    }

    // Catalog. For drivers without schemas, `schema` is the database (MySQL style) or "".
    async fn databases(&self, _show_system: bool) -> AppResult<Vec<String>> {
        Ok(Vec::new())
    }
    async fn schemas(&self, show_system: bool) -> AppResult<Vec<String>>;
    async fn objects(&self, schema: &str) -> AppResult<Vec<ObjectInfo>>;
    async fn columns(&self, schema: &str, table: &str) -> AppResult<Vec<ColumnInfo>>;
    async fn structure(&self, t: TableRef<'_>) -> AppResult<TableStructure> {
        Ok(TableStructure {
            kind: "table",
            columns: t.columns.to_vec(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
            ddl: String::new(),
        })
    }
    /// Table → columns for autocomplete. The default lists tables without columns.
    async fn completion_tables(&self, schema: &str) -> AppResult<BTreeMap<String, Vec<String>>> {
        let objects = self.objects(schema).await?;
        Ok(objects.into_iter().map(|o| (o.name, Vec::new())).collect())
    }

    // Data grid.
    async fn fetch_page(
        &self,
        t: TableRef<'_>,
        req: &PageRequest,
        page_size: u32,
    ) -> AppResult<PageRows>;
    async fn count_rows(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<RowCount>;
    fn preview_filter(&self, _t: TableRef<'_>, _req: &PageRequest) -> AppResult<FilterPreview> {
        Err(unsupported(self.driver(), "filter previews"))
    }
    fn preview_changes(&self, _t: TableRef<'_>, _changes: &[Change]) -> AppResult<Vec<String>> {
        Err(unsupported(self.driver(), "editing rows"))
    }
    /// Applies grid edits atomically where the database allows it; returns rows changed.
    async fn apply_changes(&self, _t: TableRef<'_>, _changes: &[Change]) -> AppResult<u64> {
        Err(unsupported(self.driver(), "editing rows"))
    }
    async fn fetch_cell(
        &self,
        _t: TableRef<'_>,
        _pk: &Map<String, Value>,
        _column: &str,
    ) -> AppResult<CellContent> {
        Err(unsupported(self.driver(), "Quick Look"))
    }
    async fn column_choices(&self, _t: TableRef<'_>) -> AppResult<BTreeMap<String, Vec<String>>> {
        Ok(BTreeMap::new())
    }

    // Query editor (scripts are split by `engines::split`).
    /// Runs statements in order on the engine's editor session, stopping at the first
    /// error or when `cancelled` is set. Rows past `limit` are counted, not kept.
    async fn run(
        &self,
        statements: &[String],
        limit: usize,
        cancelled: &AtomicBool,
    ) -> Vec<StatementResult>;
    /// Stops the running statement on the server, where the driver can.
    async fn cancel(&self) -> AppResult<()> {
        Ok(())
    }

    // Export. The defaults read through `run` and `fetch_page`; drivers can stream.
    async fn export_query(&self, sql: &str, sink: &mut dyn RowSink) -> AppResult<u64> {
        let flag = AtomicBool::new(false);
        let stmts = [sql.to_string()];
        let mut results = self.run(&stmts, EXPORT_QUERY_LIMIT, &flag).await;
        let r = results
            .pop()
            .ok_or_else(|| AppError::validation("There is no query to export."))?;
        if let Some(e) = r.error {
            return Err(e);
        }
        if r.truncated {
            return Err(AppError::validation(format!(
                "The query returns more than {EXPORT_QUERY_LIMIT} rows; export the table instead."
            )));
        }
        sink.columns(&r.columns)?;
        let n = r.rows.len() as u64;
        for row in r.rows {
            sink.row(row)?;
        }
        Ok(n)
    }

    async fn export_table(
        &self,
        t: TableRef<'_>,
        req: &PageRequest,
        page_only: bool,
        sink: &mut dyn RowSink,
    ) -> AppResult<u64> {
        let metas: Vec<ColumnMeta> = t
            .columns
            .iter()
            .map(|c| ColumnMeta {
                name: c.name.clone(),
                type_name: c.data_type.clone(),
            })
            .collect();
        sink.columns(&metas)?;
        let size = if page_only {
            req.page_size.max(1)
        } else {
            EXPORT_PAGE
        };
        let mut req = req.clone();
        let mut total = 0u64;
        loop {
            let page = self.fetch_page(t, &req, size).await?;
            let n = page.rows.len() as u32;
            for row in page.rows {
                sink.row(row)?;
                total += 1;
            }
            if page_only || n < size {
                return Ok(total);
            }
            req.page += 1;
        }
    }
}

/// A cancel flag check engines use between statements or batches.
pub fn is_cancelled(flag: &AtomicBool) -> bool {
    flag.load(Ordering::SeqCst)
}

pub type SharedEngine = Arc<dyn Engine>;
