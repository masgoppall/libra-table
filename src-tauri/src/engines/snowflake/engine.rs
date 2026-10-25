//! The Snowflake engine: one session for the catalog, grid and edits, and a second one
//! (logged in on first use) for the query editor, so `USE`, `ALTER SESSION` and open
//! transactions there survive between runs.
//!
//! Read-only connections: Snowflake has no read-only session switch, so the app refuses
//! writes before they are sent (safe mode classification) and `apply_changes` refuses too.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Map, Value};

use super::client::{new_request_id, Conn, Login, Output};
use super::editor::Running;
use super::sql::Sf;
use super::{catalog, op_ids, values};
use crate::db::cell::CellContent;
use crate::db::changes::Change;
use crate::db::data::{FilterPreview, PageRequest, RowCount, COUNT_CAP};
use crate::db::engine::{Engine, PageRows, TableRef};
use crate::db::exec::StatementResult;
use crate::db::meta::{ColumnInfo, ObjectInfo, TableStructure};
use crate::db::value;
use crate::engines::cloud_common::edits::{cell_select, Edit, EditTable};
use crate::engines::cloud_common::sql::{self as gsql, Binds};
use crate::engines::cloud_common::{cell_content, not_one_row, read_only, CELL_BYTES};
use crate::error::{AppError, AppResult};
use crate::models::Driver;

pub struct Snowflake {
    pub(super) http: reqwest::Client,
    pub(super) login: Login,
    pub(super) main: Conn,
    pub(super) database: String,
    pub(super) startup: String,
    pub(super) read_only: bool,
    pub(super) editor: tokio::sync::Mutex<Option<Arc<Conn>>>,
    pub(super) running: Mutex<Option<Running>>,
    edits: tokio::sync::Mutex<()>,
    closed: AtomicBool,
}

/// Adds a hint to errors with an obvious fix.
pub(super) fn hint(e: AppError) -> AppError {
    let m = e.message.to_ascii_lowercase();
    if m.contains("no active warehouse") {
        e.hint("Set a warehouse in the connection settings, or run USE WAREHOUSE first.")
    } else {
        e
    }
}

impl Snowflake {
    pub(super) fn new(
        http: reqwest::Client,
        login: Login,
        main: Conn,
        conn: &crate::models::Connection,
    ) -> Self {
        let database = if login.database.is_empty() {
            main.database.clone()
        } else {
            login.database.clone()
        };
        Self {
            http,
            login,
            main,
            database,
            startup: conn.options.startup_query.trim().to_string(),
            read_only: conn.options.read_only,
            editor: tokio::sync::Mutex::new(None),
            running: Mutex::new(None),
            edits: tokio::sync::Mutex::new(()),
            closed: AtomicBool::new(false),
        }
    }

    fn sf(&self) -> Sf<'_> {
        Sf {
            database: &self.database,
        }
    }

    async fn exec(&self, sql: &str, binds: &[String], limit: usize) -> AppResult<Output> {
        self.main
            .query(&new_request_id(), sql, binds, limit, None)
            .await
            .map_err(hint)
    }

    /// Runs the connection's startup query on a fresh session.
    pub(super) async fn startup(&self, conn: &Conn) -> AppResult<()> {
        if self.startup.is_empty() {
            return Ok(());
        }
        for stmt in crate::engines::split(Driver::Snowflake, &self.startup) {
            conn.query(&new_request_id(), &stmt.text, &[], 0, None)
                .await
                .map_err(|e| {
                    let m = format!("The startup query failed: {}", e.message);
                    AppError::query(m)
                })?;
        }
        Ok(())
    }

    async fn apply_locked(&self, stmts: &[(Edit, Vec<String>)]) -> AppResult<u64> {
        let mut total = 0;
        for (i, (e, binds)) in stmts.iter().enumerate() {
            let n = self.exec(&e.sql, binds, 1).await?.affected.unwrap_or(0);
            if e.key_where.is_some() && n != 1 {
                return Err(not_one_row(i + 1, n));
            }
            total += n;
        }
        Ok(total)
    }
}

fn first_number(out: &Output) -> i64 {
    match out.rows.first().and_then(|r| r.first()) {
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        Some(v) => v.as_i64().unwrap_or(0),
        None => 0,
    }
}

#[async_trait]
impl Engine for Snowflake {
    fn driver(&self) -> Driver {
        Driver::Snowflake
    }

    async fn server_version(&self) -> AppResult<String> {
        Ok(self.main.version.clone())
    }

    async fn current_database(&self) -> AppResult<String> {
        Ok(self.database.clone())
    }

    async fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(e) = self.editor.lock().await.take() {
            e.logout().await;
        }
        self.main.logout().await;
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    async fn databases(&self, _show_system: bool) -> AppResult<Vec<String>> {
        catalog::databases(&self.main).await
    }

    async fn schemas(&self, show_system: bool) -> AppResult<Vec<String>> {
        catalog::schemas(&self.main, &self.database, show_system).await
    }

    async fn objects(&self, schema: &str) -> AppResult<Vec<ObjectInfo>> {
        catalog::objects(&self.main, &self.database, schema).await
    }

    async fn columns(&self, schema: &str, table: &str) -> AppResult<Vec<ColumnInfo>> {
        catalog::columns(&self.main, &self.database, schema, table).await
    }

    async fn structure(&self, t: TableRef<'_>) -> AppResult<TableStructure> {
        let (ddl, view) = catalog::ddl(&self.main, &self.database, t.schema, t.table).await;
        let foreign_keys = if view {
            Vec::new()
        } else {
            catalog::foreign_keys(&self.main, &self.database, t.schema, t.table).await
        };
        Ok(TableStructure {
            kind: if view { "view" } else { "table" },
            columns: t.columns.to_vec(),
            indexes: catalog::primary_index(t.columns).into_iter().collect(),
            foreign_keys,
            ddl,
        })
    }

    async fn completion_tables(&self, schema: &str) -> AppResult<BTreeMap<String, Vec<String>>> {
        catalog::completion(&self.main, &self.database, schema).await
    }

    async fn fetch_page(
        &self,
        t: TableRef<'_>,
        req: &PageRequest,
        page_size: u32,
    ) -> AppResult<PageRows> {
        let (f, ops) = (self.sf(), op_ids());
        let page = Some((page_size, req.page as u64 * page_size as u64));
        let mut b = Binds::new();
        let sql = gsql::select(&f, t.columns, req, &ops, page, &mut b)?;
        let query = gsql::select(&f, t.columns, req, &ops, page, &mut Binds::inline())?;
        let out = self.exec(&sql, &b.values, page_size as usize).await?;
        let rows = out
            .rows
            .iter()
            .map(|r| values::row(&out.columns, r, value::PREVIEW))
            .collect();
        Ok(PageRows { rows, query })
    }

    async fn count_rows(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<RowCount> {
        let f = self.sf();
        let mut b = Binds::new();
        let sql = gsql::capped_count(&f, t.columns, req, &op_ids(), &mut b)?;
        let capped = sql.is_some();
        let sql = sql.unwrap_or_else(|| {
            use crate::engines::cloud_common::sql::Flavor;
            format!("SELECT COUNT(*) FROM {}", f.table(t.schema, t.table))
        });
        let n = first_number(&self.exec(&sql, &b.values, 1).await?);
        Ok(if capped && n > COUNT_CAP {
            RowCount {
                total: COUNT_CAP,
                exact: false,
            }
        } else {
            RowCount {
                total: n,
                exact: true,
            }
        })
    }

    fn preview_filter(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<FilterPreview> {
        let (f, ops) = (self.sf(), op_ids());
        let all = gsql::filter(
            &f,
            t.columns,
            &req.filters,
            None,
            &ops,
            &mut Binds::inline(),
        )?;
        let row = match req.apply_only {
            Some(i) => Some(gsql::filter(
                &f,
                t.columns,
                &req.filters,
                Some(i),
                &ops,
                &mut Binds::inline(),
            )?),
            None => None,
        };
        Ok(FilterPreview { row, all })
    }

    fn preview_changes(&self, t: TableRef<'_>, changes: &[Change]) -> AppResult<Vec<String>> {
        let f = self.sf();
        let et = edit_table(t);
        let mut out = Vec::new();
        for ch in changes {
            if let Some(e) = et.edit(&f, ch, &mut Binds::inline())? {
                out.push(e.sql);
            }
        }
        Ok(out)
    }

    async fn apply_changes(&self, t: TableRef<'_>, changes: &[Change]) -> AppResult<u64> {
        if self.read_only {
            return Err(read_only());
        }
        let (f, et) = (self.sf(), edit_table(t));
        let mut stmts = Vec::new();
        for ch in changes {
            let mut b = Binds::new();
            if let Some(e) = et.edit(&f, ch, &mut b)? {
                stmts.push((e, b.values));
            }
        }
        if stmts.is_empty() {
            return Ok(0);
        }
        let _one_at_a_time = self.edits.lock().await;
        self.exec("BEGIN", &[], 0).await?;
        match self.apply_locked(&stmts).await {
            Ok(n) => {
                self.exec("COMMIT", &[], 0).await?;
                Ok(n)
            }
            Err(e) => {
                let _ = self.exec("ROLLBACK", &[], 0).await;
                Err(e)
            }
        }
    }

    async fn fetch_cell(
        &self,
        t: TableRef<'_>,
        pk: &Map<String, Value>,
        column: &str,
    ) -> AppResult<CellContent> {
        let mut b = Binds::new();
        let sql = cell_select(&self.sf(), &edit_table(t), pk, column, &mut b)?;
        let out = self.exec(&sql, &b.values, 1).await?;
        let row = out.rows.first().ok_or_else(|| {
            AppError::validation("The row no longer exists.").hint("Refresh the table.")
        })?;
        let cell = values::row(&out.columns, row, CELL_BYTES)
            .into_iter()
            .next();
        Ok(cell_content(cell.unwrap_or(Value::Null)))
    }

    async fn run(
        &self,
        statements: &[String],
        limit: usize,
        cancelled_flag: &AtomicBool,
    ) -> Vec<StatementResult> {
        self.run_editor(statements, limit, cancelled_flag).await
    }

    async fn cancel(&self) -> AppResult<()> {
        self.cancel_editor().await
    }
}

fn edit_table<'a>(t: TableRef<'a>) -> EditTable<'a> {
    EditTable {
        schema: t.schema,
        table: t.table,
        columns: t.columns,
    }
}
