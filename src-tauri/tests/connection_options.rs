//! Read-only connections and startup queries, against real servers: the server itself
//! must refuse writes (not only the app's classifier). Postgres/MySQL run when
//! LIBRATABLE_TEST_PG_URL / LIBRATABLE_TEST_MYSQL_URL are set; SQLite always runs.
//! Only `libratable_opt_*` objects are created, and they are dropped again.

use libratable_lib::db::{open_pool, DbPool};
use libratable_lib::models::Connection;
use serde_json::{json, Value};

const DB: &str = "libratable_opt_db";

fn connection(driver: &str, db: Value, options: Value) -> Connection {
    serde_json::from_value(json!({
        "id": "opt", "name": "Options", "driver": driver, "db": db, "options": options
    }))
    .unwrap()
}

fn url_part(url: &url::Url) -> Value {
    json!({
        "host": url.host_str().unwrap_or("127.0.0.1"),
        "port": url.port(),
        "user": url.username(),
        "database": url.path().trim_start_matches('/'),
        "passwordMode": "none",
    })
}

fn read_only_with(startup: &str) -> Value {
    json!({ "readOnly": true, "startupQuery": startup })
}

async fn exec(pool: &DbPool, sql: &str) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Postgres(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
        DbPool::Mysql(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
        DbPool::Sqlite(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
    }
}

#[tokio::test]
async fn postgres_read_only_and_startup_query() {
    let Ok(raw) = std::env::var("LIBRATABLE_TEST_PG_URL") else {
        eprintln!("skipped: set LIBRATABLE_TEST_PG_URL to run");
        return;
    };
    let url = url::Url::parse(&raw).unwrap();
    let conn = connection(
        "postgres",
        url_part(&url),
        read_only_with("SET search_path TO pg_catalog"),
    );
    let pool = open_pool(&conn, None, None, None, 2).await.unwrap();
    let DbPool::Postgres(p) = &pool else {
        unreachable!()
    };
    let path: String = sqlx::query_scalar("SHOW search_path")
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(path, "pg_catalog");
    let err = exec(&pool, "CREATE TABLE public.libratable_opt_t (id int)")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("read-only"), "{err}");
    pool.close().await;
    // In case the guard ever fails, leave nothing behind.
    let admin = connection("postgres", url_part(&url), json!({}));
    let admin = open_pool(&admin, None, None, None, 1).await.unwrap();
    exec(&admin, "DROP TABLE IF EXISTS public.libratable_opt_t")
        .await
        .unwrap();
    admin.close().await;
}

#[tokio::test]
async fn mysql_read_only_and_startup_query() {
    let Ok(raw) = std::env::var("LIBRATABLE_TEST_MYSQL_URL") else {
        eprintln!("skipped: set LIBRATABLE_TEST_MYSQL_URL to run");
        return;
    };
    let url = url::Url::parse(&raw).unwrap();
    let admin = connection("mysql", url_part(&url), json!({}));
    let admin = open_pool(&admin, None, None, None, 1).await.unwrap();
    exec(&admin, &format!("DROP DATABASE IF EXISTS {DB}"))
        .await
        .unwrap();
    exec(&admin, &format!("CREATE DATABASE {DB}"))
        .await
        .unwrap();
    exec(&admin, &format!("CREATE TABLE {DB}.t (id INT)"))
        .await
        .unwrap();

    let mut db = url_part(&url);
    db["database"] = json!(DB);
    let conn = connection("mysql", db, read_only_with("SET @libratable_opt = 42"));
    let pool = open_pool(&conn, None, None, None, 1).await.unwrap();
    let DbPool::Mysql(p) = &pool else {
        unreachable!()
    };
    let v: i64 = sqlx::query_scalar("SELECT @libratable_opt")
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(v, 42);
    let refused = exec(&pool, "INSERT INTO t VALUES (1)").await;
    pool.close().await;
    exec(&admin, &format!("DROP DATABASE {DB}")).await.unwrap();
    admin.close().await;
    let err = refused.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("read only"),
        "{err}"
    );
}

#[tokio::test]
async fn sqlite_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opt.sqlite");
    let rw = connection("sqlite", json!({ "filePath": path }), json!({}));
    let rw_pool = open_pool(&rw, None, None, None, 1).await;
    // The file must exist first: create it through a writable connection.
    assert!(rw_pool.is_err());
    std::fs::write(&path, b"").unwrap();
    let rw_pool = open_pool(&rw, None, None, None, 1).await.unwrap();
    exec(&rw_pool, "CREATE TABLE t (id INTEGER)").await.unwrap();
    rw_pool.close().await;

    let ro = connection("sqlite", json!({ "filePath": path }), read_only_with(""));
    let pool = open_pool(&ro, None, None, None, 1).await.unwrap();
    let err = exec(&pool, "INSERT INTO t VALUES (1)").await.unwrap_err();
    assert!(err.to_string().contains("readonly"), "{err}");
}
