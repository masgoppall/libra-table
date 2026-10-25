//! Helpers for the server tools scenarios: sessions, SQL, tool jobs, cleanup on panic.

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures::FutureExt;
use libratable_lib::db::{open_pool, DbPool};
use libratable_lib::error::AppResult;
use libratable_lib::models::{Connection, Driver};
use libratable_lib::server::tools::run::Progress;
use libratable_lib::server::tools::{self, args, ToolChoice};
use libratable_lib::server::users::{self, UserChange};
use libratable_lib::state::Session;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

/// (host, port, user, password) of a database URL.
pub fn url_parts(url: &str) -> (String, u16, String, Option<String>) {
    let u = url::Url::parse(url).unwrap();
    (
        u.host_str().unwrap().to_string(),
        u.port().unwrap(),
        u.username().to_string(),
        u.password().map(str::to_string),
    )
}

pub fn connection(driver: Driver, settings: Value) -> Connection {
    let mut v = json!({ "id": "srv-test", "name": "Server tools test", "driver": driver });
    v["db"] = settings;
    serde_json::from_value(v).unwrap()
}

/// A session on `database` of the server behind `admin`, as `user` when given.
pub async fn session(
    driver: Driver,
    admin: &str,
    database: &str,
    login: Option<(&str, &str)>,
) -> Arc<Session> {
    let (host, port, admin_user, admin_pw) = url_parts(admin);
    let (user, password) = match login {
        Some((u, p)) => (u.to_string(), Some(p.to_string())),
        None => (admin_user, admin_pw),
    };
    let conn = connection(
        driver,
        json!({ "host": host, "port": port, "user": user, "database": database,
                "passwordMode": if password.is_some() { "keychain" } else { "none" } }),
    );
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

pub async fn exec(pool: &DbPool, sql: &str) {
    let r = match pool {
        DbPool::Postgres(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
        DbPool::Mysql(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
        DbPool::Sqlite(p) => sqlx::raw_sql(sql).execute(p).await.map(|_| ()),
    };
    r.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

pub async fn scalar_i64(pool: &DbPool, sql: &str) -> i64 {
    match pool {
        DbPool::Postgres(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Mysql(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Sqlite(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
    }
}

/// Plans a user change, checks the preview holds no `secret`, and applies it.
pub async fn change(s: &Session, change: Value, secret: Option<&str>) -> Vec<String> {
    let change: UserChange = serde_json::from_value(change).expect("user change JSON");
    let stmts = users::plan(&s.pool, &change).await.expect("plan");
    let shown: Vec<String> = stmts.iter().map(|x| x.shown.clone()).collect();
    if let Some(secret) = secret {
        assert!(shown.iter().all(|x| !x.contains(secret)), "{shown:?}");
        assert!(shown.iter().any(|x| x.contains(users::MASK)), "{shown:?}");
    }
    users::apply(&s.pool, &stmts)
        .await
        .unwrap_or_else(|e| panic!("{shown:?}: {e:?}"));
    shown
}

/// The tool, or None (and a note) when it isn't installed here.
pub async fn tool(s: &Session, name: &str) -> Option<ToolChoice> {
    let choice = tools::resolve(name, s.driver(), &s.server_version, None).await;
    if choice.path.is_none() {
        eprintln!("skipped: {name} not found ({:?})", choice.warning);
        return None;
    }
    Some(choice)
}

/// What a job reported: stderr lines and the last byte count.
#[derive(Default, Debug)]
pub struct Report {
    pub lines: Vec<String>,
    pub bytes: Option<(u64, Option<u64>)>,
}

pub async fn backup(s: &Session, r: &args::BackupRequest, c: &ToolChoice) -> Report {
    let mut rep = Report::default();
    let token = CancellationToken::new();
    tools::backup(s, r, Some(c), &token, |p| record(&mut rep, p))
        .await
        .unwrap_or_else(|e| panic!("backup: {e:?}"));
    rep
}

pub async fn restore(
    s: &Session,
    r: &args::RestoreRequest,
    c: &ToolChoice,
) -> (AppResult<()>, Report) {
    let mut rep = Report::default();
    let token = CancellationToken::new();
    let result = tools::restore(s, r, c, &token, |p| record(&mut rep, p)).await;
    (result, rep)
}

fn record(rep: &mut Report, p: Progress) {
    match p {
        Progress::Line(l) => rep.lines.push(l),
        Progress::Bytes { done, total } => rep.bytes = Some((done, total)),
    }
}

/// Runs `scenario`, then `cleanup` even when the scenario panicked.
pub async fn with_cleanup<C, CF>(scenario: impl Future<Output = ()>, cleanup: C)
where
    C: FnOnce() -> CF,
    CF: Future<Output = ()>,
{
    let outcome = AssertUnwindSafe(scenario).catch_unwind().await;
    cleanup().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
