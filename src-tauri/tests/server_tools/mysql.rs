//! MySQL: process list (KILL QUERY, KILL), accounts and grants, mysqldump → mysql round
//! trip as a password user. Only `libratable_srv_*` databases and users are touched.

use std::sync::Arc;
use std::time::Duration;

use libratable_lib::models::Dialect;
use libratable_lib::server::process::{self, ProcessAction, ProcessInfo};
use libratable_lib::server::tools::args;
use libratable_lib::server::users::{self, Grantee};
use libratable_lib::state::Session;
use serde_json::json;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

use crate::common::{self, backup, change, exec, restore, scalar_i64, session, tool};

const DB: &str = "libratable_srv_db";
const RESTORE_DB: &str = "libratable_srv_restore";
const USER: &str = "libratable_srv_user";
/// A quote and a backslash: both need escaping in the literal.
const PASSWORD: &str = r"p'w\x secret";

pub async fn run(admin: String) {
    let admin_pool = MySqlPool::connect(&admin).await.unwrap();
    cleanup(&admin_pool).await;
    for db in [DB, RESTORE_DB] {
        sqlx::query(&format!("CREATE DATABASE {db}"))
            .execute(&admin_pool)
            .await
            .unwrap();
    }
    let scenario = async {
        let s = session(Dialect::Mysql, &admin, DB, None).await;
        exec(
            &s.pool,
            "CREATE TABLE items (id int PRIMARY KEY, name text); \
             INSERT INTO items VALUES (1, 'a'), (2, 'b''s\\\\x'), (3, NULL)",
        )
        .await;
        processes(&s, &admin).await;
        accounts(&s).await;
        backup_restore(&s, &admin).await;
        s.close().await;
    };
    common::with_cleanup(scenario, || cleanup(&admin_pool)).await;
    admin_pool.close().await;
}

async fn cleanup(admin: &MySqlPool) {
    for sql in [
        format!("DROP DATABASE IF EXISTS {DB}"),
        format!("DROP DATABASE IF EXISTS {RESTORE_DB}"),
        format!("DROP USER IF EXISTS '{USER}'@'%'"),
    ] {
        sqlx::query(&sql).execute(admin).await.unwrap();
    }
}

async fn find(s: &Session, matches: impl Fn(&ProcessInfo) -> bool) -> ProcessInfo {
    for _ in 0..50 {
        let list = process::list(&s.pool, &[]).await.unwrap();
        if let Some(p) = list.into_iter().find(&matches) {
            return p;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("process not found");
}

async fn processes(s: &Arc<Session>, admin: &str) {
    let list = process::list(&s.pool, &[]).await.unwrap();
    assert!(
        list.iter().any(|p| p.own),
        "the listing connection is LibraTable's"
    );

    let client = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(admin)
        .await
        .unwrap();
    let sleeper = client.clone();
    let sleep = tokio::spawn(async move {
        sqlx::query("SELECT SLEEP(30) AS libratable_srv_sleep")
            .execute(&sleeper)
            .await
    });
    let p = find(s, |p| p.query.contains("libratable_srv_sleep")).await;
    assert!(!p.own && p.database == "mysql", "{p:?}");
    process::run_action(&s.pool, ProcessAction::Cancel, p.id)
        .await
        .unwrap();
    // SLEEP() returns early when its query is killed (it doesn't fail).
    let r = tokio::time::timeout(Duration::from_secs(5), sleep).await;
    r.expect("query stopped in time").unwrap().ok();

    let mut idle = client.acquire().await.unwrap();
    let id: i64 = sqlx::query_scalar("SELECT CAST(CONNECTION_ID() AS SIGNED)")
        .fetch_one(&mut *idle)
        .await
        .unwrap();
    process::run_action(&s.pool, ProcessAction::Kill, id)
        .await
        .unwrap();
    assert!(sqlx::query("SELECT 1").execute(&mut *idle).await.is_err());
    drop(idle);
    client.close().await;
}

fn grantee() -> serde_json::Value {
    json!({ "name": USER, "host": "%" })
}

async fn account(s: &Session) -> users::MysqlAccount {
    let list = users::list(&s.pool).await.unwrap();
    let found = list
        .accounts
        .into_iter()
        .find(|a| a.user == USER && a.host == "%");
    found.expect("account listed")
}

async fn accounts(s: &Session) {
    let create = json!({ "kind": "create_user", "user": USER, "host": "%", "password": "tmp" });
    change(s, create, Some("tmp")).await;
    assert!(account(s).await.privileges.is_empty());
    let set = json!({ "kind": "set_password", "grantee": grantee(), "password": PASSWORD });
    change(s, set, Some("secret")).await;

    let global = json!({ "kind": "grant", "grantee": grantee(), "privileges": ["PROCESS"],
        "target": { "on": "global" } });
    change(s, global, None).await;
    assert_eq!(account(s).await.privileges, vec!["PROCESS"]);
    let table = json!({ "kind": "grant", "grantee": grantee(), "privileges": ["SELECT"],
        "target": { "on": "table", "schema": DB, "name": "items" }, "withGrantOption": true });
    change(s, table, None).await;
    for db in [DB, RESTORE_DB] {
        let all = json!({ "kind": "grant", "grantee": grantee(),
            "privileges": ["ALL PRIVILEGES"], "target": { "on": "database", "name": db } });
        change(s, all, None).await;
    }
    let g = Grantee {
        name: USER.into(),
        host: Some("%".into()),
    };
    let lines = users::grants(&s.pool, &g).await.unwrap();
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    assert!(
        has(&format!(
            "ON `{DB}`.`items` TO `{USER}`@`%` WITH GRANT OPTION"
        )),
        "{lines:?}"
    );
    assert!(
        has(&format!("GRANT ALL PRIVILEGES ON `{RESTORE_DB}`.*")),
        "{lines:?}"
    );

    let revoke = json!({ "kind": "revoke", "grantee": grantee(), "privileges": ["PROCESS"],
        "target": { "on": "global" } });
    change(s, revoke, None).await;
    assert!(account(s).await.privileges.is_empty());
}

/// Dumps and restores as the password user: the password reaches the tools through
/// MYSQL_PWD, and its quote and backslash survived the escaping.
async fn backup_restore(admin_session: &Session, admin: &str) {
    let login = Some((USER, PASSWORD));
    let s = session(Dialect::Mysql, admin, DB, login).await;
    let target = session(Dialect::Mysql, admin, RESTORE_DB, login).await;
    if let (Some(dump), Some(client)) = (tool(&s, "mysqldump").await, tool(&s, "mysql").await) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sql").display().to_string();
        let req = args::BackupRequest {
            path: path.clone(),
            single_transaction: true,
            triggers: true,
            ..Default::default()
        };
        let rep = backup(&s, &req, &dump).await;
        assert!(
            !rep.lines.is_empty(),
            "mysqldump --verbose reports progress"
        );
        let req = args::RestoreRequest {
            path,
            stop_on_error: true,
            ..Default::default()
        };
        let (r, _) = restore(&target, &req, &client).await;
        r.unwrap();
        let n = scalar_i64(&target.pool, "SELECT count(*) FROM items").await;
        assert_eq!(n, 3);
        let odd = "SELECT count(*) FROM items WHERE name = 'b''s\\\\x'";
        assert_eq!(scalar_i64(&target.pool, odd).await, 1);
        stop_on_error(&target, &client, dir.path()).await;
    }
    s.close().await;
    target.close().await;
    let drop = json!({ "kind": "drop_user", "user": USER, "host": "%" });
    change(admin_session, drop, None).await;
}

async fn stop_on_error(
    target: &Session,
    client: &libratable_lib::server::tools::ToolChoice,
    dir: &std::path::Path,
) {
    let bad = dir.join("bad.sql");
    std::fs::write(
        &bad,
        "CREATE TABLE before_error (a int);\nSELEC 1;\nCREATE TABLE after_error (a int);\n",
    )
    .unwrap();
    let mut req = args::RestoreRequest {
        path: bad.display().to_string(),
        stop_on_error: true,
        ..Default::default()
    };
    let (r, _) = restore(target, &req, client).await;
    assert!(r.unwrap_err().message.contains("ERROR 1064"));
    let exists = format!(
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema = '{RESTORE_DB}' AND table_name = 'after_error'"
    );
    assert_eq!(scalar_i64(&target.pool, &exists).await, 0);
    req.stop_on_error = false;
    exec(&target.pool, "DROP TABLE before_error").await;
    let _ = restore(target, &req, client).await;
    assert_eq!(scalar_i64(&target.pool, &exists).await, 1);
}
