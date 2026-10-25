//! Enum choices and whole-cell fetches (Quick Look) against real servers. Only the
//! `libratable_opt_cells` database is created, and dropped again.

use std::sync::Arc;

use libratable_lib::db::cell::{fetch_cell, CellContent};
use libratable_lib::db::choices::column_choices;
use libratable_lib::db::{open_pool, DbPool};
use libratable_lib::models::Connection;
use libratable_lib::state::Session;
use serde_json::{json, Map, Value};

const DB: &str = "libratable_opt_cells";

fn connection(driver: &str, url: &url::Url, database: &str) -> Connection {
    serde_json::from_value(json!({
        "id": "cells", "name": "Cells", "driver": driver,
        "db": {
            "host": url.host_str().unwrap_or("127.0.0.1"), "port": url.port(),
            "user": url.username(), "database": database, "passwordMode": "none"
        }
    }))
    .unwrap()
}

async fn exec(pool: &DbPool, sql: &str) {
    match pool {
        DbPool::Postgres(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
        DbPool::Mysql(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
        DbPool::Sqlite(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
    }
    .unwrap();
}

async fn session(conn: Connection, database: &str) -> Arc<Session> {
    let pool = open_pool(&conn, None, None, None, 2).await.unwrap();
    Arc::new(Session::new(
        conn,
        pool,
        None,
        database.into(),
        String::new(),
        None,
    ))
}

fn pk(id: i64) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("id".into(), json!(id));
    m
}

const PNG: &str = "89504e470d0a1a0a0000000d49484452";

#[tokio::test]
async fn postgres_enum_choices_and_whole_binary_cell() {
    let Ok(raw) = std::env::var("LIBRATABLE_TEST_PG_URL") else {
        eprintln!("skipped: set LIBRATABLE_TEST_PG_URL to run");
        return;
    };
    let url = url::Url::parse(&raw).unwrap();
    let admin = open_pool(
        &connection("postgres", &url, "postgres"),
        None,
        None,
        None,
        1,
    )
    .await
    .unwrap();
    exec(&admin, &format!("DROP DATABASE IF EXISTS {DB}")).await;
    exec(&admin, &format!("CREATE DATABASE {DB}")).await;
    let s = session(connection("postgres", &url, DB), DB).await;
    exec(
        &s.pool,
        &format!(
            "CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy'); \
             CREATE TABLE t (id int PRIMARY KEY, m mood, pic bytea); \
             INSERT INTO t VALUES (1, 'ok', decode('{PNG}' || repeat('00', 8000), 'hex'));"
        ),
    )
    .await;
    let choices = column_choices(&s.pool, "public", "t").await.unwrap();
    assert_eq!(choices.get("m").unwrap(), &vec!["sad", "ok", "happy"]);
    let cell = fetch_cell(&s, "public", "t", &pk(1), "pic").await.unwrap();
    s.pool.close().await;
    exec(&admin, &format!("DROP DATABASE {DB}")).await;
    let CellContent::Binary {
        size,
        truncated,
        mime,
        ..
    } = cell
    else {
        panic!("{cell:?}")
    };
    assert_eq!((size, truncated, mime), (8016, false, Some("image/png")));
}

#[tokio::test]
async fn mysql_enum_choices_and_text_cell() {
    let Ok(raw) = std::env::var("LIBRATABLE_TEST_MYSQL_URL") else {
        eprintln!("skipped: set LIBRATABLE_TEST_MYSQL_URL to run");
        return;
    };
    let url = url::Url::parse(&raw).unwrap();
    let admin = open_pool(&connection("mysql", &url, "mysql"), None, None, None, 1)
        .await
        .unwrap();
    exec(&admin, &format!("DROP DATABASE IF EXISTS {DB}")).await;
    exec(&admin, &format!("CREATE DATABASE {DB}")).await;
    let s = session(connection("mysql", &url, DB), DB).await;
    exec(
        &s.pool,
        "CREATE TABLE t (id INT PRIMARY KEY, status ENUM('new','it''s paid'), note TEXT); \
         INSERT INTO t VALUES (7, 'new', 'hello world');",
    )
    .await;
    let choices = column_choices(&s.pool, DB, "t").await.unwrap();
    let cell = fetch_cell(&s, DB, "t", &pk(7), "note").await.unwrap();
    s.pool.close().await;
    exec(&admin, &format!("DROP DATABASE {DB}")).await;
    assert_eq!(choices.get("status").unwrap(), &vec!["new", "it's paid"]);
    assert_eq!(
        cell,
        CellContent::Text {
            text: "hello world".into()
        }
    );
}
