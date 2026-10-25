//! Grid data: one page of a table with filters and sort applied server-side
//! (GRD-2, GRD-3, FLT-*), and a cheap row count.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::filter::{inline_preview, order_by, FilterBuilder, FilterRow, FilterSql, SortSpec};
use crate::db::meta::{primary_key, ColumnInfo};
use crate::db::sql::{qualified, quote_ident};
use crate::db::value;
use crate::db::{bind_all, DbPool};
use crate::error::AppResult;
use crate::models::Driver;
use crate::state::Session;

pub const MAX_PAGE_SIZE: u32 = 5000;
/// Counts stop here; above it the total is reported as "more than".
pub const COUNT_CAP: i64 = 1_000_000;
/// Below this estimated size we count exactly.
const EXACT_COUNT_BELOW: i64 = 200_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest {
    pub schema: String,
    pub table: String,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub page_size: u32,
    #[serde(default)]
    pub sort: Vec<SortSpec>,
    #[serde(default)]
    pub filters: Vec<FilterRow>,
    /// Apply only this filter row (per-row "Apply"); otherwise all enabled rows.
    #[serde(default)]
    pub apply_only: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Value>>,
    pub primary_key: Vec<String>,
    pub page: u32,
    pub page_size: u32,
    /// True when the page is full, i.e. there may be another page.
    pub has_more: bool,
    /// The query that produced this page, with values shown inline (preview only).
    pub sql: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowCount {
    pub total: i64,
    /// False for an estimate or a capped count ("more than").
    pub exact: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterPreview {
    /// WHERE fragment for this one row ("this filter").
    pub row: Option<String>,
    /// WHERE fragment for all enabled rows ("all checked filters").
    pub all: String,
}

fn col_types(cols: &[ColumnInfo]) -> Vec<(String, String)> {
    cols.iter()
        .map(|c| (c.name.clone(), c.data_type.clone()))
        .collect()
}

pub(crate) fn select_list(driver: Driver, cols: &[ColumnInfo]) -> String {
    cols.iter()
        .map(|c| {
            let q = quote_ident(driver, &c.name);
            if driver == Driver::Postgres && !value::pg::natively_decoded(&c.data_type) {
                format!("{q}::text AS {q}")
            } else {
                q
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn build_filter(
    driver: Driver,
    cols: &[ColumnInfo],
    rows: &[FilterRow],
    only: Option<usize>,
) -> AppResult<FilterSql> {
    let types = col_types(cols);
    FilterBuilder::new(driver, &types, 1).build(rows, only)
}

fn page_size(session: &Session, req: &PageRequest) -> u32 {
    match req.page_size {
        0 => session.connection.options.page_size.clamp(1, MAX_PAGE_SIZE),
        n => n.clamp(1, MAX_PAGE_SIZE),
    }
}

/// `SELECT … FROM … [WHERE …] [ORDER BY …] [LIMIT … OFFSET …]` plus its binds.
/// Shared by the grid and table exports so both see exactly the same rows.
pub fn select_sql(
    session: &Session,
    cols: &[ColumnInfo],
    req: &PageRequest,
    paged: bool,
) -> AppResult<(String, Vec<String>)> {
    let driver = session.driver();
    let filter = build_filter(driver, cols, &req.filters, req.apply_only)?;
    let types = col_types(cols);
    let order = order_by(driver, &types, &req.sort)?;
    let mut sql = format!(
        "SELECT {} FROM {}",
        select_list(driver, cols),
        qualified(driver, &req.schema, &req.table)
    );
    if !filter.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&filter.sql);
    }
    if !order.is_empty() {
        sql.push_str(" ORDER BY ");
        sql.push_str(&order);
    }
    if paged {
        let size = page_size(session, req);
        let offset = req.page as u64 * size as u64;
        sql.push_str(&format!(" LIMIT {size} OFFSET {offset}"));
    }
    Ok((sql, filter.binds))
}

pub async fn fetch_page(session: &Session, req: &PageRequest) -> AppResult<PageResult> {
    let cols = session.columns(&req.schema, &req.table).await?;
    let page_size = page_size(session, req);
    let query = FilterSql::from(select_sql(session, &cols, req, true)?);
    let started = Instant::now();
    let rows = fetch_rows(&session.pool, &query).await?;
    Ok(PageResult {
        primary_key: primary_key(&cols),
        has_more: rows.len() as u32 == page_size,
        columns: cols.as_ref().clone(),
        rows,
        page: req.page,
        page_size,
        sql: inline_preview(session.driver(), &query),
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// Runs a SELECT with its binds and decodes the rows for the grid.
async fn fetch_rows(pool: &DbPool, query: &FilterSql) -> AppResult<Vec<Vec<Value>>> {
    macro_rules! fetch {
        ($p:expr, $decode:path) => {
            bind_all!(sqlx::query(&query.sql), &query.binds)
                .fetch_all($p)
                .await?
                .iter()
                .map(|r| $decode(r, value::PREVIEW))
                .collect()
        };
    }
    Ok(match pool {
        DbPool::Postgres(p) => fetch!(p, value::pg::row),
        DbPool::Mysql(p) => fetch!(p, value::mysql::row),
        DbPool::Sqlite(p) => fetch!(p, value::sqlite::row),
    })
}

pub async fn count_rows(session: &Session, req: &PageRequest) -> AppResult<RowCount> {
    let driver = session.driver();
    let cols = session.columns(&req.schema, &req.table).await?;
    let filter = build_filter(driver, &cols, &req.filters, req.apply_only)?;
    if filter.is_empty() {
        if let Some(est) = estimate_rows(&session.pool, &req.schema, &req.table).await? {
            if est >= EXACT_COUNT_BELOW {
                return Ok(RowCount {
                    total: est,
                    exact: false,
                });
            }
        }
    }
    let table = qualified(driver, &req.schema, &req.table);
    let n = capped_count(&session.pool, &table, &filter).await?;
    Ok(if n > COUNT_CAP {
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

const PG_ESTIMATE: &str = "SELECT c.reltuples::int8 FROM pg_class c \
    JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1 AND c.relname = $2";

const MYSQL_ESTIMATE: &str = "SELECT CAST(TABLE_ROWS AS SIGNED) FROM information_schema.TABLES \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?";

/// The planner's row estimate (Postgres, MySQL); SQLite has none.
async fn estimate_rows(pool: &DbPool, schema: &str, table: &str) -> AppResult<Option<i64>> {
    Ok(match pool {
        DbPool::Postgres(p) => {
            sqlx::query_scalar(PG_ESTIMATE)
                .bind(schema)
                .bind(table)
                .fetch_optional(p)
                .await?
        }
        DbPool::Mysql(p) => sqlx::query_scalar::<_, Option<i64>>(MYSQL_ESTIMATE)
            .bind(schema)
            .bind(table)
            .fetch_optional(p)
            .await?
            .flatten(),
        DbPool::Sqlite(_) => None,
    })
}

/// Counts matching rows, stopping just past `COUNT_CAP`.
async fn capped_count(pool: &DbPool, table: &str, filter: &FilterSql) -> AppResult<i64> {
    let mut inner = format!("SELECT 1 FROM {table}");
    if !filter.is_empty() {
        inner.push_str(" WHERE ");
        inner.push_str(&filter.sql);
    }
    let sql = format!(
        "SELECT COUNT(*) FROM ({inner} LIMIT {}) AS counted",
        COUNT_CAP + 1
    );
    macro_rules! count {
        ($p:expr) => {
            bind_all!(sqlx::query_scalar::<_, i64>(&sql), &filter.binds)
                .fetch_one($p)
                .await?
        };
    }
    Ok(match pool {
        DbPool::Postgres(p) => count!(p),
        DbPool::Mysql(p) => count!(p),
        DbPool::Sqlite(p) => count!(p),
    })
}

/// SQL preview for the filter bar: this row, and all checked rows (FLT-5).
pub async fn preview_filter(session: &Session, req: &PageRequest) -> AppResult<FilterPreview> {
    let driver = session.driver();
    let cols = session.columns(&req.schema, &req.table).await?;
    let all = build_filter(driver, &cols, &req.filters, None)?;
    let row = match req.apply_only {
        Some(i) => Some(inline_preview(
            driver,
            &build_filter(driver, &cols, &req.filters, Some(i))?,
        )),
        None => None,
    };
    Ok(FilterPreview {
        row,
        all: inline_preview(driver, &all),
    })
}
