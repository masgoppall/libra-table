//! The BigQuery engine. Projects are the database level (jobs always run in the
//! connection's project; another project's tables are read by qualified name), datasets
//! are the schemas. The editor joins a BigQuery session, so temp tables, variables and
//! transactions survive between runs where the server supports sessions.
//!
//! Every grid page or count with filters is a query job, billed by the bytes of the
//! columns it reads (LIMIT doesn't lower that). Edits need a declared primary key and
//! run as one multi-statement transaction; DML jobs count against BigQuery's
//! concurrent-DML limits. Read-only: there is no read-only session switch, so the app
//! refuses writes before sending them and `apply_changes` refuses too.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{Map, Value};

use super::catalog;
use super::client::{Api, JobRef, QueryArgs, Results};
use super::sql::{edit_script, one_row_failure, Bq};
use super::{op_ids, values};
use crate::db::cell::CellContent;
use crate::db::changes::Change;
use crate::db::data::{FilterPreview, PageRequest, RowCount, COUNT_CAP};
use crate::db::engine::{is_cancelled, Engine, PageRows, TableRef};
use crate::db::exec::StatementResult;
use crate::db::meta::{primary_key, ColumnInfo, ObjectInfo, TableStructure};
use crate::db::value;
use crate::engines::cloud_common::edits::{cell_select, EditTable};
use crate::engines::cloud_common::sql::{self as gsql, Binds, Flavor};
use crate::engines::cloud_common::{cancelled, cell_content, not_one_row, read_only, CELL_BYTES};
use crate::error::{AppError, AppResult};
use crate::models::Driver;

pub struct BigQuery {
    pub(super) api: Api,
    /// The project whose datasets the catalog lists.
    pub(super) data_project: String,
    /// Dataset for unqualified names in the editor ("" for none).
    pub(super) default_dataset: String,
    pub(super) startup: String,
    pub(super) read_only: bool,
    /// The editor's session id; locked for a whole run.
    pub(super) session: tokio::sync::Mutex<Option<String>>,
    pub(super) running: Mutex<Option<JobRef>>,
}

fn no_key() -> AppError {
    AppError::validation("This BigQuery table declares no primary key, so rows can't be edited.")
        .hint("Declare one (ALTER TABLE … ADD PRIMARY KEY (…) NOT ENFORCED), or use a query.")
}

fn first_number(r: &Results) -> i64 {
    r.rows
        .first()
        .and_then(|row| row.pointer("/f/0/v"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// An error that means the editor's session is gone.
fn session_gone(e: &AppError) -> bool {
    let m = e.message.to_ascii_lowercase();
    m.contains("session") && (m.contains("expired") || m.contains("not found"))
}

impl BigQuery {
    fn bq(&self) -> Bq<'_> {
        Bq {
            project: &self.data_project,
        }
    }

    async fn query(&self, sql: &str, params: &[String], limit: usize) -> AppResult<Results> {
        let a = QueryArgs {
            sql,
            params,
            ..QueryArgs::default()
        };
        self.api.query(&a, limit, None, &|_| {}).await
    }

    fn default_dataset(&self) -> Option<(&str, &str)> {
        (!self.default_dataset.is_empty() && self.data_project == self.api.project)
            .then_some((self.data_project.as_str(), self.default_dataset.as_str()))
    }

    /// Runs one editor statement in the session (`slot`), starting it when needed.
    async fn editor_query(
        &self,
        slot: &mut Option<String>,
        sql: &str,
        limit: usize,
        flag: &AtomicBool,
    ) -> AppResult<Results> {
        let mut retried = false;
        loop {
            let a = QueryArgs {
                sql,
                default_dataset: self.default_dataset(),
                session: slot.as_deref(),
                create_session: slot.is_none(),
                ..QueryArgs::default()
            };
            let track = |job: Option<JobRef>| *self.running.lock().unwrap() = job;
            match self.api.query(&a, limit, Some(flag), &track).await {
                Ok(r) => {
                    if slot.is_none() && r.session.is_some() {
                        *slot = r.session.clone();
                    }
                    return Ok(r);
                }
                Err(e) if !retried && slot.is_some() && session_gone(&e) => {
                    *slot = None;
                    retried = true;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Starts the editor session and runs the startup query in it.
    async fn start_session(&self, slot: &mut Option<String>, flag: &AtomicBool) -> AppResult<()> {
        if self.startup.is_empty() || slot.is_some() {
            return Ok(());
        }
        for stmt in super::split::split(&self.startup) {
            self.editor_query(slot, &stmt.text, 0, flag)
                .await
                .map_err(|e| AppError::query(format!("The startup query failed: {}", e.message)))?;
        }
        Ok(())
    }
}

fn edit_table<'a>(t: TableRef<'a>) -> AppResult<EditTable<'a>> {
    if primary_key(t.columns).is_empty() {
        return Err(no_key());
    }
    Ok(EditTable {
        schema: t.schema,
        table: t.table,
        columns: t.columns,
    })
}

fn fill(r: &mut StatementResult, res: Results) {
    if res.fields.is_empty() {
        r.rows_affected = Some(res.dml_rows.unwrap_or(0));
        return;
    }
    r.columns = values::columns(&res.fields);
    r.rows = res
        .rows
        .iter()
        .map(|row| values::row(&res.fields, row, value::PREVIEW))
        .collect();
    r.total_rows = res.total.max(r.rows.len() as u64);
    r.truncated = r.total_rows > r.rows.len() as u64;
}

#[async_trait]
impl Engine for BigQuery {
    fn driver(&self) -> Driver {
        Driver::Bigquery
    }

    async fn server_version(&self) -> AppResult<String> {
        Ok("BigQuery API v2".into())
    }

    async fn current_database(&self) -> AppResult<String> {
        Ok(self.data_project.clone())
    }

    async fn close(&self) {}

    async fn databases(&self, _show_system: bool) -> AppResult<Vec<String>> {
        let mut list = catalog::projects(&self.api).await.unwrap_or_default();
        for p in [&self.api.project, &self.data_project] {
            if !list.contains(p) {
                list.push(p.clone());
            }
        }
        list.sort();
        Ok(list)
    }

    /// The default dataset comes first, so the sidebar opens it.
    async fn schemas(&self, show_system: bool) -> AppResult<Vec<String>> {
        let mut list = catalog::datasets(&self.api, &self.data_project, show_system).await?;
        if let Some(i) = list.iter().position(|d| *d == self.default_dataset) {
            let d = list.remove(i);
            list.insert(0, d);
        }
        Ok(list)
    }

    async fn objects(&self, schema: &str) -> AppResult<Vec<ObjectInfo>> {
        catalog::tables(&self.api, &self.data_project, schema).await
    }

    async fn columns(&self, schema: &str, table: &str) -> AppResult<Vec<ColumnInfo>> {
        Ok(catalog::table(&self.api, &self.data_project, schema, table)
            .await?
            .columns())
    }

    async fn structure(&self, t: TableRef<'_>) -> AppResult<TableStructure> {
        let info = catalog::table(&self.api, &self.data_project, t.schema, t.table).await?;
        Ok(TableStructure {
            kind: if info.is_view() { "view" } else { "table" },
            columns: t.columns.to_vec(),
            indexes: info.indexes(),
            foreign_keys: info.foreign_keys(),
            ddl: info.ddl(&self.bq().table(t.schema, t.table)),
        })
    }

    async fn fetch_page(
        &self,
        t: TableRef<'_>,
        req: &PageRequest,
        page_size: u32,
    ) -> AppResult<PageRows> {
        let (f, ops) = (self.bq(), op_ids());
        let page = Some((page_size, req.page as u64 * page_size as u64));
        let mut b = Binds::new();
        let sql = gsql::select(&f, t.columns, req, &ops, page, &mut b)?;
        let query = gsql::select(&f, t.columns, req, &ops, page, &mut Binds::inline())?;
        let res = self.query(&sql, &b.values, page_size as usize).await?;
        let rows = res
            .rows
            .iter()
            .map(|r| values::row(&res.fields, r, value::PREVIEW))
            .collect();
        Ok(PageRows { rows, query })
    }

    /// Without filters a table's count is its metadata (free); otherwise a capped query.
    async fn count_rows(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<RowCount> {
        let f = self.bq();
        let mut b = Binds::new();
        let capped = gsql::capped_count(&f, t.columns, req, &op_ids(), &mut b)?;
        let sql = match capped {
            Some(sql) => sql,
            None => {
                let info = catalog::table(&self.api, &self.data_project, t.schema, t.table).await?;
                if let (false, Some(n)) = (info.is_view(), info.num_rows) {
                    return Ok(RowCount {
                        total: n,
                        exact: true,
                    });
                }
                format!("SELECT COUNT(*) FROM {}", f.table(t.schema, t.table))
            }
        };
        let n = first_number(&self.query(&sql, &b.values, 1).await?);
        Ok(RowCount {
            total: n.min(COUNT_CAP),
            exact: n <= COUNT_CAP,
        })
    }

    fn preview_filter(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<FilterPreview> {
        let (f, ops) = (self.bq(), op_ids());
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
        let (f, et) = (self.bq(), edit_table(t)?);
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
        let (f, et) = (self.bq(), edit_table(t)?);
        // One parameter list for the whole script, so @pN stay unique.
        let mut b = Binds::new();
        let mut edits = Vec::new();
        for ch in changes {
            if let Some(e) = et.edit(&f, ch, &mut b)? {
                edits.push(e);
            }
        }
        if edits.is_empty() {
            return Ok(0);
        }
        let script = edit_script(&f, &edits, t.schema, t.table);
        match self.query(&script, &b.values, 1).await {
            Ok(_) => Ok(edits.len() as u64),
            Err(e) => Err(match one_row_failure(&e.message) {
                Some((change, rows)) => not_one_row(change, rows),
                None => e,
            }),
        }
    }

    async fn fetch_cell(
        &self,
        t: TableRef<'_>,
        pk: &Map<String, Value>,
        column: &str,
    ) -> AppResult<CellContent> {
        let mut b = Binds::new();
        let sql = cell_select(&self.bq(), &edit_table(t)?, pk, column, &mut b)?;
        let res = self.query(&sql, &b.values, 1).await?;
        let row = res.rows.first().ok_or_else(|| {
            AppError::validation("The row no longer exists.").hint("Refresh the table.")
        })?;
        let cell = values::row(&res.fields, row, CELL_BYTES).into_iter().next();
        Ok(cell_content(cell.unwrap_or(Value::Null)))
    }

    async fn run(
        &self,
        statements: &[String],
        limit: usize,
        flag: &AtomicBool,
    ) -> Vec<StatementResult> {
        let mut slot = self.session.lock().await;
        let mut results = Vec::new();
        if let Err(e) = self.start_session(&mut slot, flag).await {
            let mut r = StatementResult::new(statements.first().map_or("", |s| s));
            r.error = Some(e);
            return vec![r];
        }
        for sql in statements {
            if is_cancelled(flag) {
                break;
            }
            let started = Instant::now();
            let mut r = StatementResult::new(sql);
            let res = self.editor_query(&mut slot, sql, limit, flag).await;
            r.duration_ms = started.elapsed().as_millis() as u64;
            let failed = match res {
                Ok(res) => {
                    fill(&mut r, res);
                    false
                }
                Err(_) if is_cancelled(flag) => {
                    r.error = Some(cancelled());
                    true
                }
                Err(e) => {
                    r.error = Some(e);
                    true
                }
            };
            results.push(r);
            if failed {
                break;
            }
        }
        results
    }

    async fn cancel(&self) -> AppResult<()> {
        let job = self.running.lock().unwrap().clone();
        match job {
            Some(j) => self.api.cancel(&j).await,
            None => Ok(()),
        }
    }

    async fn completion_tables(&self, schema: &str) -> AppResult<BTreeMap<String, Vec<String>>> {
        let objects = self.objects(schema).await?;
        Ok(objects.into_iter().map(|o| (o.name, Vec::new())).collect())
    }
}
