//! The DynamoDB engine: tables of one region (the only "schema"), item grids with
//! cached page keys, conditional edits, and PartiQL (`ExecuteStatement`) in the editor.
//! DynamoDB has no sessions and no server-side cancel: a run stops between pages when
//! cancelled. Read-only connections are enforced app-side (PartiQL classification, and
//! `apply_changes` refuses); IAM policies are the server-side way.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use super::client::Client;
use super::filter::build;
use super::values::{to_json, typed_text};
use super::{catalog, edits, grid, op_ids};
use crate::db::cell::CellContent;
use crate::db::changes::Change;
use crate::db::data::{FilterPreview, PageRequest, RowCount};
use crate::db::engine::{is_cancelled, Engine, PageRows, TableRef};
use crate::db::exec::StatementResult;
use crate::db::meta::{ColumnInfo, ObjectInfo, TableStructure};
use crate::db::value::{self, ColumnMeta};
use crate::engines::cloud_common::{cancelled, cell_content, read_only, CELL_BYTES};
use crate::error::{AppError, AppResult};
use crate::models::Driver;

/// Below this many items (DescribeTable's estimate) the grid counts exactly.
const EXACT_COUNT_BELOW: i64 = 200_000;

pub struct Dynamo {
    pub(super) client: Client,
    pub(super) read_only: bool,
    pub(super) local: bool,
    pub(super) pages: grid::PageCache,
}

fn filter_preview(
    cols: &[ColumnInfo],
    req: &PageRequest,
    only: Option<usize>,
) -> AppResult<String> {
    let e = build(cols, &req.filters, only, &op_ids(), false)?;
    if e.filter.is_empty() {
        return Ok(String::new());
    }
    let mut body = Map::new();
    e.apply(&mut body);
    Ok(Value::Object(body).to_string())
}

impl Dynamo {
    /// One PartiQL statement, paging with NextToken until `limit` items.
    async fn partiql(
        &self,
        sql: &str,
        limit: usize,
        flag: &AtomicBool,
    ) -> AppResult<StatementResult> {
        let started = Instant::now();
        let mut r = StatementResult::new(sql);
        let mut items: Vec<Value> = Vec::new();
        let mut token: Option<Value> = None;
        let mut more = false;
        loop {
            let mut body = json!({ "Statement": sql });
            if let Some(t) = token.take() {
                body["NextToken"] = t;
            }
            let page = self.client.call("ExecuteStatement", &body).await?;
            if let Some(got) = page.get("Items").and_then(Value::as_array) {
                items.extend(got.iter().cloned());
            }
            token = page.get("NextToken").filter(|t| !t.is_null()).cloned();
            if items.len() > limit {
                more = true;
                break;
            }
            if token.is_none() {
                break;
            }
            if items.len() == limit {
                more = true;
                break;
            }
            if is_cancelled(flag) {
                return Err(cancelled());
            }
        }
        let mut names: Vec<String> = Vec::new();
        for item in &items {
            for k in item.as_object().into_iter().flat_map(|m| m.keys()) {
                if !names.contains(k) {
                    names.push(k.clone());
                }
            }
        }
        r.total_rows = items.len() as u64;
        items.truncate(limit);
        r.truncated = more;
        r.rows = items
            .iter()
            .map(|it| {
                names
                    .iter()
                    .map(|n| {
                        it.get(n)
                            .map(|v| to_json(v, value::PREVIEW))
                            .unwrap_or(Value::Null)
                    })
                    .collect()
            })
            .collect();
        r.columns = names
            .into_iter()
            .map(|name| ColumnMeta {
                name,
                type_name: String::new(),
            })
            .collect();
        r.duration_ms = started.elapsed().as_millis() as u64;
        Ok(r)
    }
}

#[async_trait]
impl Engine for Dynamo {
    fn driver(&self) -> Driver {
        Driver::Dynamodb
    }

    async fn server_version(&self) -> AppResult<String> {
        Ok(if self.local {
            "DynamoDB API 2012-08-10 (custom endpoint)".into()
        } else {
            format!("DynamoDB API 2012-08-10 ({})", self.client.region)
        })
    }

    async fn close(&self) {}

    async fn schemas(&self, _show_system: bool) -> AppResult<Vec<String>> {
        Ok(vec![self.client.region.clone()])
    }

    async fn objects(&self, _schema: &str) -> AppResult<Vec<ObjectInfo>> {
        catalog::tables(&self.client).await
    }

    async fn columns(&self, _schema: &str, table: &str) -> AppResult<Vec<ColumnInfo>> {
        catalog::columns(&self.client, table).await
    }

    async fn structure(&self, t: TableRef<'_>) -> AppResult<TableStructure> {
        let desc = catalog::describe(&self.client, t.table).await?;
        Ok(TableStructure {
            kind: "table",
            columns: t.columns.to_vec(),
            indexes: catalog::indexes(&desc),
            foreign_keys: Vec::new(),
            ddl: serde_json::to_string_pretty(&desc).unwrap_or_default(),
        })
    }

    async fn fetch_page(
        &self,
        t: TableRef<'_>,
        req: &PageRequest,
        page_size: u32,
    ) -> AppResult<PageRows> {
        let p = grid::plan(t.table, t.columns, req, &op_ids())?;
        let items = grid::page(
            &self.client,
            &self.pages,
            &p,
            t.columns,
            req.page as usize,
            page_size as usize,
        )
        .await?;
        let mut rows = grid::rows(&items, t.columns, value::PREVIEW);
        grid::sort_page(&mut rows, t.columns, &req.sort)?;
        Ok(PageRows {
            rows,
            query: grid::describe(&p, page_size, !req.sort.is_empty()),
        })
    }

    async fn count_rows(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<RowCount> {
        let p = grid::plan(t.table, t.columns, req, &op_ids())?;
        if p.action == "Scan" && !p.body.contains_key("FilterExpression") {
            let desc = catalog::describe(&self.client, t.table).await?;
            let estimate = desc.get("ItemCount").and_then(Value::as_i64).unwrap_or(0);
            if estimate >= EXACT_COUNT_BELOW {
                return Ok(RowCount {
                    total: estimate,
                    exact: false,
                });
            }
        }
        let (total, exact) = grid::count(&self.client, &p).await?;
        Ok(RowCount { total, exact })
    }

    fn preview_filter(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<FilterPreview> {
        let row = match req.apply_only {
            Some(i) => Some(filter_preview(t.columns, req, Some(i))?),
            None => None,
        };
        Ok(FilterPreview {
            row,
            all: filter_preview(t.columns, req, None)?,
        })
    }

    fn preview_changes(&self, t: TableRef<'_>, changes: &[Change]) -> AppResult<Vec<String>> {
        Ok(edits::build(t.table, t.columns, changes)?
            .iter()
            .map(edits::preview)
            .collect())
    }

    async fn apply_changes(&self, t: TableRef<'_>, changes: &[Change]) -> AppResult<u64> {
        if self.read_only {
            return Err(read_only());
        }
        let writes = edits::build(t.table, t.columns, changes)?;
        let res = edits::apply(&self.client, &writes).await;
        self.pages.forget(t.table);
        res
    }

    async fn fetch_cell(
        &self,
        t: TableRef<'_>,
        pk: &Map<String, Value>,
        column: &str,
    ) -> AppResult<CellContent> {
        let mut key = Map::new();
        for c in t.columns.iter().filter(|c| c.pk_position.is_some()) {
            let v = pk.get(&c.name).ok_or_else(|| {
                AppError::validation(format!("Missing key value for \"{}\".", c.name))
            })?;
            let text = v
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string());
            key.insert(c.name.clone(), typed_text(&text, &c.data_type)?);
        }
        let body = json!({
            "TableName": t.table,
            "Key": key,
            "ProjectionExpression": "#c",
            "ExpressionAttributeNames": { "#c": column },
            "ConsistentRead": true,
        });
        let got = self.client.call("GetItem", &body).await?;
        let item = got.get("Item").ok_or_else(|| {
            AppError::validation("The item no longer exists.").hint("Refresh the table.")
        })?;
        Ok(cell_content(
            item.get(column)
                .map(|v| to_json(v, CELL_BYTES))
                .unwrap_or(Value::Null),
        ))
    }

    async fn run(
        &self,
        statements: &[String],
        limit: usize,
        flag: &AtomicBool,
    ) -> Vec<StatementResult> {
        let mut results = Vec::new();
        for sql in statements {
            if is_cancelled(flag) {
                break;
            }
            let started = Instant::now();
            match self.partiql(sql, limit, flag).await {
                Ok(r) => results.push(r),
                Err(e) => {
                    let mut r = StatementResult::new(sql);
                    r.duration_ms = started.elapsed().as_millis() as u64;
                    r.error = Some(e);
                    results.push(r);
                    break;
                }
            }
        }
        results
    }
}
