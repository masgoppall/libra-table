//! End-to-end checks of the Redis driver against a real server:
//!   LIBRATABLE_TEST_REDIS_URL=redis://:password@127.0.0.1:16390
//! The suite works in database 15 (it must be empty), creates only keys named
//! `libratable:test:*` and deletes them at the end, even when a check fails.
//! TLS runs against a server started with `--tls-port` (see `tls.rs`).

mod editor;
mod grid;
mod keys;
mod tls;

use std::sync::Arc;

use futures::FutureExt;
use libratable_lib::db::{open_pool, DbPool};
use libratable_lib::engines::redis::{KeyEdit, RedisEngine};
use libratable_lib::error::ErrorKind;
use libratable_lib::models::Connection;
use libratable_lib::state::Session;
use serde_json::{json, Value};

pub const PREFIX: &str = "libratable:test:";
pub const DB: &str = "15";

pub struct Server {
    pub host: String,
    pub port: u16,
    user: String,
    pub password: Option<String>,
}

impl Server {
    fn from_env() -> Option<Self> {
        Self::from_var("LIBRATABLE_TEST_REDIS_URL")
    }

    pub fn from_var(name: &str) -> Option<Self> {
        let raw = std::env::var(name).ok()?;
        let url = url::Url::parse(&raw).expect("a redis:// URL");
        Some(Self {
            host: url.host_str().unwrap_or("127.0.0.1").to_string(),
            port: url.port().unwrap_or(6379),
            user: url.username().to_string(),
            password: url.password().map(str::to_string),
        })
    }

    pub fn connection(&self, read_only: bool) -> Connection {
        serde_json::from_value(json!({
            "id": "redis-test",
            "name": "Redis test",
            "driver": "redis",
            "db": {
                "host": self.host,
                "port": self.port,
                "user": self.user,
                "database": DB,
            },
            "options": { "readOnly": read_only },
        }))
        .unwrap()
    }

    pub async fn session(&self) -> Arc<Session> {
        let conn = self.connection(false);
        let pool = open_pool(&conn, None, self.password.as_deref(), None, 4)
            .await
            .expect("connect");
        let version = pool.server_version().await.unwrap();
        Arc::new(Session::new(
            conn,
            pool,
            None,
            format!("db{DB}"),
            version,
            self.password.clone(),
        ))
    }
}

pub fn redis(s: &Session) -> &RedisEngine {
    s.pool
        .engine()
        .and_then(|e| e.as_any())
        .and_then(|a| a.downcast_ref::<RedisEngine>())
        .expect("a Redis engine")
}

pub fn key(name: &str) -> String {
    format!("{PREFIX}{name}")
}

/// Runs a value-panel edit and expects it to work.
pub async fn edit(s: &Session, edit: Value) {
    let edit: KeyEdit = serde_json::from_value(edit).unwrap();
    let plan = RedisEngine::edit_plan(&edit).unwrap();
    redis(s).run_plan(&plan).await.expect("edit");
}

pub async fn run(s: &Session, script: &str) -> Vec<libratable_lib::db::exec::StatementResult> {
    let stmts: Vec<String> = libratable_lib::engines::split(s.connection.driver, script)
        .into_iter()
        .map(|st| st.text)
        .collect();
    let flag = std::sync::atomic::AtomicBool::new(false);
    let DbPool::Engine(e) = &s.pool else {
        panic!("not an engine")
    };
    e.run(&stmts, 1000, &flag).await
}

/// Deletes every test key of the database.
async fn cleanup(s: &Session) {
    let page = redis(s)
        .scan(
            &serde_json::from_value(json!({ "pattern": format!("{PREFIX}*"), "pageSize": 5000 }))
                .unwrap(),
        )
        .await;
    let Ok(page) = page else { return };
    let keys: Vec<Value> = page.keys.iter().map(|k| k.key.clone()).collect();
    if !keys.is_empty() {
        edit(s, json!({ "op": "delete", "keys": keys })).await;
    }
}

#[tokio::test]
async fn redis_end_to_end() {
    let Some(server) = Server::from_env() else {
        eprintln!("skipped: set LIBRATABLE_TEST_REDIS_URL to run");
        return;
    };
    wrong_password(&server).await;
    let s = server.session().await;
    let size = run(&s, "DBSIZE").await;
    assert_eq!(
        size[0].rows[0][0],
        json!(0),
        "database {DB} of the test server must be empty"
    );
    let outcome = std::panic::AssertUnwindSafe(async {
        keys::create_all(&s).await;
        grid::run(&s).await;
        keys::run(&s).await;
        editor::run_all(&s, &server).await;
    })
    .catch_unwind();
    let result = outcome.await;
    cleanup(&s).await;
    assert_eq!(run(&s, "DBSIZE").await[0].rows[0][0], json!(0));
    s.pool.close().await;
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

async fn wrong_password(server: &Server) {
    let conn = server.connection(false);
    let err = open_pool(&conn, None, Some("not-the-password"), None, 1)
        .await
        .err()
        .expect("a wrong password fails");
    assert_eq!(err.kind, ErrorKind::DbAuth, "{err:?}");
    assert!(!err.message.contains("not-the-password"));
    let bad_db = serde_json::from_value::<Connection>(json!({
        "id": "x", "name": "x", "driver": "redis",
        "db": { "host": server.host, "port": server.port, "database": "999" },
    }))
    .unwrap();
    let err = open_pool(&bad_db, None, server.password.as_deref(), None, 1)
        .await
        .err()
        .expect("a missing database index fails");
    assert_eq!(err.kind, ErrorKind::Validation, "{err:?}");
}
