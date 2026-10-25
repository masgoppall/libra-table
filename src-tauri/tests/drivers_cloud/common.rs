//! Helpers the cloud scenarios share.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use libratable_lib::db::changes::Change;
use libratable_lib::db::data::PageRequest;
use libratable_lib::db::engine::{SharedEngine, TableRef};
use libratable_lib::db::filter::{FilterRow, SortSpec};
use libratable_lib::db::meta::ColumnInfo;
use libratable_lib::db::{open_pool, DbPool};
use libratable_lib::error::{AppError, AppResult};
use libratable_lib::export::{self, ExportRequest, Format, Options, Source};
use libratable_lib::models::Connection;
use libratable_lib::state::Session;
use serde_json::{json, Map, Value};

pub fn connection(driver: &str, db: Value, read_only: bool) -> Connection {
    serde_json::from_value(json!({
        "id": format!("{driver}-test"), "name": "Cloud test", "driver": driver,
        "db": db, "options": { "readOnly": read_only, "pageSize": 100, "queryRowLimit": 1000,
            "showSystemSchemas": false, "safeMode": "silent", "startupQuery": "",
            "keepAliveSecs": 0 }
    }))
    .unwrap()
}

pub async fn try_session(conn: Connection, secret: Option<&str>) -> AppResult<Arc<Session>> {
    let pool = open_pool(&conn, None, secret, None, 4).await?;
    let version = pool.server_version().await?;
    let database = match &pool {
        DbPool::Engine(e) => e.current_database().await?,
        _ => String::new(),
    };
    Ok(Arc::new(Session::new(
        conn,
        pool,
        None,
        database,
        version,
        secret.map(str::to_string),
    )))
}

pub fn engine(s: &Session) -> SharedEngine {
    match &s.pool {
        DbPool::Engine(e) => e.clone(),
        _ => panic!("not an engine session"),
    }
}

pub fn filter(column: &str, operator: &str, value: &str) -> FilterRow {
    FilterRow {
        enabled: true,
        column: column.into(),
        operator: operator.into(),
        value: value.into(),
        value2: String::new(),
    }
}

pub fn page(schema: &str, table: &str, size: u32, filters: Vec<FilterRow>) -> PageRequest {
    PageRequest {
        schema: schema.into(),
        table: table.into(),
        page: 0,
        page_size: size,
        sort: vec![],
        filters,
        apply_only: None,
    }
}

pub fn sorted(mut req: PageRequest, column: &str, desc: bool) -> PageRequest {
    req.sort = vec![SortSpec {
        column: column.into(),
        desc,
    }];
    req
}

pub fn map(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap()
}

pub fn update(pk: Value, values: Value) -> Change {
    Change::Update {
        pk: map(pk),
        values: map(values),
    }
}

/// Grid edits the way `commit_changes` sends them to an engine.
pub async fn apply(
    s: &Session,
    schema: &str,
    table: &str,
    changes: &[Change],
) -> Result<u64, AppError> {
    let cols = s.columns(schema, table).await?;
    let t = table_ref(schema, table, &cols);
    engine(s).apply_changes(t, changes).await
}

pub fn table_ref<'a>(schema: &'a str, table: &'a str, cols: &'a [ColumnInfo]) -> TableRef<'a> {
    TableRef {
        schema,
        table,
        columns: cols,
    }
}

/// Runs editor statements as `run_query` does.
pub async fn run(s: &Session, sql: &[&str]) -> Vec<libratable_lib::db::exec::StatementResult> {
    let stmts: Vec<String> = sql.iter().map(|s| s.to_string()).collect();
    engine(s).run(&stmts, 1000, &AtomicBool::new(false)).await
}

/// Exports to a temporary CSV file and returns its lines.
pub async fn export_csv(s: &Session, source: Source) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.csv");
    let req = ExportRequest {
        source,
        format: Format::Csv,
        path: path.display().to_string(),
        options: Options::default(),
    };
    let token = tokio_util::sync::CancellationToken::new();
    export::run(s, &req, &token, |_| {})
        .await
        .unwrap_or_else(|e| panic!("export failed: {e:?}"));
    std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

pub fn table_source(req: PageRequest) -> Source {
    Source::Table {
        request: req,
        page_only: false,
    }
}
