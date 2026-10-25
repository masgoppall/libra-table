//! End-to-end checks of the server tools: process list, users & privileges, backup and
//! restore with the native tools, and the SQLite backup.
//!
//! * SQLite always runs (temporary files).
//! * Postgres / MySQL run when an admin URL is given. They create and drop only
//!   databases, roles and users named `libratable_srv_*`:
//!   LIBRATABLE_TEST_PG_URL=postgres://postgres@127.0.0.1:5432/postgres
//!   LIBRATABLE_TEST_MYSQL_URL=mysql://root@127.0.0.1:3306/mysql
//! * Backup/restore steps are skipped (with a note) when the tools aren't installed.

mod common;
mod mysql;
mod postgres;

use libratable_lib::db::{open_pool, DbPool};
use libratable_lib::models::Driver;
use libratable_lib::server::tools::{self, args};
use libratable_lib::state::Session;
use serde_json::json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn postgres_server_tools() {
    match std::env::var("LIBRATABLE_TEST_PG_URL") {
        Ok(url) => postgres::run(url).await,
        Err(_) => eprintln!("skipped: set LIBRATABLE_TEST_PG_URL to run"),
    }
}

#[tokio::test]
async fn mysql_server_tools() {
    match std::env::var("LIBRATABLE_TEST_MYSQL_URL") {
        Ok(url) => mysql::run(url).await,
        Err(_) => eprintln!("skipped: set LIBRATABLE_TEST_MYSQL_URL to run"),
    }
}

#[tokio::test]
async fn sqlite_backup_is_a_copy_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("live.db");
    std::fs::File::create(&db).unwrap();
    let conn = common::connection(
        Driver::Sqlite,
        json!({ "filePath": db.display().to_string() }),
    );
    let pool = open_pool(&conn, None, None, None, 2).await.unwrap();
    let s = Arc::new(Session::new(
        conn,
        pool,
        None,
        "main".into(),
        "3".into(),
        None,
    ));
    common::exec(&s.pool, "CREATE TABLE t (a); INSERT INTO t VALUES (1), (2)").await;
    let token = CancellationToken::new();

    let copy = dir.path().join("copy.db");
    std::fs::write(&copy, "old").unwrap();
    let req = args::BackupRequest {
        path: copy.display().to_string(),
        ..Default::default()
    };
    tools::backup(&s, &req, None, &token, |_| {}).await.unwrap();
    let backup_conn = common::connection(
        Driver::Sqlite,
        json!({ "filePath": copy.display().to_string() }),
    );
    let b = open_pool(&backup_conn, None, None, None, 1).await.unwrap();
    assert_eq!(common::scalar_i64(&b, "SELECT count(*) FROM t").await, 2);
    b.close().await;

    // Never over the database itself.
    let same = args::BackupRequest {
        path: db.display().to_string(),
        ..Default::default()
    };
    assert!(tools::backup(&s, &same, None, &token, |_| {})
        .await
        .is_err());
    assert_eq!(
        common::scalar_i64(&s.pool, "SELECT count(*) FROM t").await,
        2
    );
    assert!(tools::restore_tool(Driver::Sqlite, &db.display().to_string()).is_err());
    let DbPool::Sqlite(p) = &s.pool else {
        unreachable!()
    };
    p.close().await;
}
