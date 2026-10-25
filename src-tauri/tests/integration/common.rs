//! Helpers shared by the end-to-end scenarios.

use std::sync::Arc;

use libratable_lib::db::data::{self, PageRequest};
use libratable_lib::db::ddl::{self, SchemaChange};
use libratable_lib::db::filter::FilterRow;
use libratable_lib::db::{meta, open_pool, DbPool};
use libratable_lib::error::AppError;
use libratable_lib::models::{Connection, Driver};
use libratable_lib::state::Session;
use serde_json::{json, Value};

/// The only databases these tests create (and drop).
pub const TEST_DB: &str = "libratable_test";
pub const OTHER_DB: &str = "libratable_test_other";

pub fn connection(driver: Driver, settings: Value) -> Connection {
    let mut v = json!({ "id": "test-conn", "name": "Test", "driver": driver });
    v["db"] = settings;
    serde_json::from_value(v).unwrap()
}

/// A session on `conn`, as `connect` would open it.
pub async fn session_for(
    conn: Connection,
    database: &str,
    password: Option<String>,
) -> Arc<Session> {
    let pool = open_pool(&conn, None, password.as_deref(), None, 4)
        .await
        .expect("connect");
    let version = pool.server_version().await.unwrap();
    Arc::new(Session::new(
        conn,
        pool,
        None,
        database.to_string(),
        version,
        password,
    ))
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

pub fn page(schema: &str, filters: Vec<FilterRow>) -> PageRequest {
    table_page(schema, "people", 100, filters)
}

pub fn table_page(
    schema: &str,
    table: &str,
    page_size: u32,
    filters: Vec<FilterRow>,
) -> PageRequest {
    PageRequest {
        schema: schema.into(),
        table: table.into(),
        page: 0,
        page_size,
        sort: vec![],
        filters,
        apply_only: None,
    }
}

pub async fn exec_sql(pool: &DbPool, sql: &str) {
    match pool {
        DbPool::Postgres(p) => {
            sqlx::raw_sql(sql).execute(p).await.unwrap();
        }
        DbPool::Mysql(p) => {
            sqlx::raw_sql(sql).execute(p).await.unwrap();
        }
        DbPool::Sqlite(p) => {
            sqlx::raw_sql(sql).execute(p).await.unwrap();
        }
    }
}

/// The `name` column of every row the request returns.
pub async fn names(session: &Session, req: &PageRequest) -> Vec<String> {
    let page = data::fetch_page(session, req)
        .await
        .unwrap_or_else(|e| panic!("fetch_page failed: {e:?}"));
    let idx = page.columns.iter().position(|c| c.name == "name").unwrap();
    page.rows
        .iter()
        .map(|r| r[idx].as_str().unwrap().to_string())
        .collect()
}

/// Rows in a table (up to 1,000).
pub async fn count(session: &Session, schema: &str, table: &str) -> u64 {
    let req = table_page(schema, table, 1000, vec![]);
    data::fetch_page(session, &req)
        .await
        .unwrap_or_else(|e| panic!("fetch {table}: {e:?}"))
        .rows
        .len() as u64
}

/// Builds, previews and runs a schema change the way `apply_schema_change` does.
pub async fn ddl_apply(s: &Session, change: Value) -> Result<Vec<String>, AppError> {
    let change: SchemaChange = serde_json::from_value(change).expect("schema change JSON");
    let ctx = ddl::context(&s.pool, &change).await?;
    let plan = ddl::build(s.driver(), &change, &ctx)?;
    let preview = ddl::preview(&s.pool, &plan, &change, &ctx).await?;
    ddl::apply(&s.pool, &plan, &change, &ctx).await?;
    s.clear_cache();
    Ok(preview)
}

/// A loaded column as the structure editor sends it back.
pub fn spec(c: &meta::ColumnInfo) -> Value {
    json!({
        "name": c.name, "dataType": c.data_type, "nullable": c.nullable,
        "default": c.default, "primaryKey": c.pk_position.is_some(),
        "autoIncrement": c.auto_increment, "comment": c.comment,
    })
}

/// Object names of a schema.
pub async fn object_names(session: &Session, schema: &str) -> Vec<String> {
    meta::list_objects(&session.pool, schema)
        .await
        .unwrap()
        .into_iter()
        .map(|o| o.name)
        .collect()
}
