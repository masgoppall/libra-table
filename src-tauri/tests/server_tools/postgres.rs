//! Postgres: process list (cancel, terminate), roles and grants, pg_dump → psql /
//! pg_restore round trips. Only `libratable_srv_*` databases and roles are touched.

use std::sync::Arc;
use std::time::Duration;

use libratable_lib::db::DbPool;
use libratable_lib::models::Dialect;
use libratable_lib::server::process::{self, ProcessAction, ProcessInfo};
use libratable_lib::server::tools::{self, args};
use libratable_lib::server::users::{self, Grantee};
use libratable_lib::state::Session;
use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::common::{self, backup, change, exec, restore, scalar_i64, session, tool};

const DB: &str = "libratable_srv_db";
const RESTORE_DB: &str = "libratable_srv_restore";
const ROLE: &str = "libratable_srv_role";
const GROUP: &str = "libratable_srv_group";
const PASSWORD: &str = r"it's\a secret";

pub async fn run(admin: String) {
    let admin_pool = PgPool::connect(&admin).await.unwrap();
    cleanup(&admin_pool).await;
    for db in [DB, RESTORE_DB] {
        sqlx::query(&format!("CREATE DATABASE {db}"))
            .execute(&admin_pool)
            .await
            .unwrap();
    }
    let scenario = async {
        let s = session(Dialect::Postgres, &admin, DB, None).await;
        exec(
            &s.pool,
            "CREATE TABLE items (id int PRIMARY KEY, name text); \
             INSERT INTO items VALUES (1, 'a'), (2, 'b''s'), (3, NULL)",
        )
        .await;
        processes(&s, &admin).await;
        roles(&s).await;
        backup_restore(&s, &admin).await;
        s.close().await;
    };
    common::with_cleanup(scenario, || cleanup(&admin_pool)).await;
    admin_pool.close().await;
}

async fn cleanup(admin: &PgPool) {
    for db in [DB, RESTORE_DB] {
        let sql = format!("DROP DATABASE IF EXISTS {db} WITH (FORCE)");
        sqlx::query(&sql).execute(admin).await.unwrap();
    }
    for role in [ROLE, GROUP] {
        let sql = format!("DROP ROLE IF EXISTS {role}");
        sqlx::query(&sql).execute(admin).await.unwrap();
    }
}

/// A plain pool on the test database that isn't LibraTable's.
async fn other_client(admin: &str) -> PgPool {
    let o: PgConnectOptions = admin.parse().unwrap();
    let o = o.database(DB).application_name("libratable_srv_client");
    PgPoolOptions::new()
        .max_connections(2)
        .connect_with(o)
        .await
        .unwrap()
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

    let client = other_client(admin).await;
    let sleeper = client.clone();
    let sleep =
        tokio::spawn(async move { sqlx::query("SELECT pg_sleep(30)").execute(&sleeper).await });
    let p = find(s, |p| p.query.contains("pg_sleep(30)")).await;
    assert!(!p.own && p.state == "active" && p.database == DB, "{p:?}");
    assert_eq!(p.application, "libratable_srv_client");
    process::run_action(&s.pool, ProcessAction::Cancel, p.id)
        .await
        .unwrap();
    let r = tokio::time::timeout(Duration::from_secs(5), sleep).await;
    let err = r.expect("cancelled in time").unwrap().unwrap_err();
    assert!(err.to_string().contains("cancel"), "{err}");

    let mut idle = client.acquire().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *idle)
        .await
        .unwrap();
    process::run_action(&s.pool, ProcessAction::Kill, pid as i64)
        .await
        .unwrap();
    assert!(sqlx::query("SELECT 1").execute(&mut *idle).await.is_err());
    drop(idle);
    let gone = process::run_action(&s.pool, ProcessAction::Kill, 2_000_000_000).await;
    assert!(gone.is_err(), "an unknown pid is reported");
    client.close().await;
}

async fn role(s: &Session) -> users::PgRole {
    let list = users::list(&s.pool).await.unwrap();
    list.roles
        .into_iter()
        .find(|r| r.name == ROLE)
        .expect("role listed")
}

async fn grants(s: &Session) -> Vec<String> {
    let g = Grantee {
        name: ROLE.into(),
        host: None,
    };
    users::grants(&s.pool, &g).await.unwrap()
}

async fn roles(s: &Session) {
    change(
        s,
        json!({ "kind": "create_role", "role": { "name": GROUP } }),
        None,
    )
    .await;
    let create = json!({ "kind": "create_role", "password": PASSWORD,
        "role": { "name": ROLE, "login": true, "connLimit": 2 } });
    change(s, create, Some("secret")).await;
    let r = role(s).await;
    assert!(
        r.login && !r.superuser && !r.create_db && r.conn_limit == 2,
        "{r:?}"
    );
    let has_pw: bool = sqlx_scalar_bool(&s.pool, ROLE).await;
    assert!(has_pw, "the password was set");

    let alter = json!({ "kind": "alter_role", "name": ROLE, "role": {
        "name": ROLE, "login": true, "createDb": true, "memberOf": [GROUP] } });
    let shown = change(s, alter, None).await;
    assert_eq!(
        shown,
        vec![
            format!("ALTER ROLE \"{ROLE}\" WITH CREATEDB CONNECTION LIMIT -1"),
            format!("GRANT \"{GROUP}\" TO \"{ROLE}\""),
        ]
    );
    let r = role(s).await;
    assert!(r.create_db && r.conn_limit == -1 && r.member_of == vec![GROUP.to_string()]);

    let grantee = json!({ "name": ROLE });
    let targets = [
        (
            json!({ "on": "table", "schema": "public", "name": "items" }),
            "SELECT",
        ),
        (json!({ "on": "schema", "name": "public" }), "USAGE"),
        (json!({ "on": "database", "name": DB }), "CONNECT"),
    ];
    for (target, privilege) in &targets {
        let grant = json!({ "kind": "grant", "grantee": grantee, "target": target,
            "privileges": [privilege] });
        change(s, grant, None).await;
    }
    let all = json!({ "kind": "grant", "grantee": grantee, "privileges": ["INSERT"],
        "target": { "on": "all_tables", "schema": "public" } });
    change(s, all, None).await;
    let g = grants(s).await;
    assert!(
        g.contains(&"INSERT, SELECT ON TABLE public.items".to_string()),
        "{g:?}"
    );
    assert!(g.contains(&format!("CONNECT ON DATABASE {DB}")), "{g:?}");
    assert!(g.contains(&"USAGE ON SCHEMA public".to_string()), "{g:?}");

    for (target, _) in &targets {
        let revoke = json!({ "kind": "revoke", "grantee": grantee, "target": target,
            "privileges": ["ALL PRIVILEGES"] });
        change(s, revoke, None).await;
    }
    assert!(grants(s).await.is_empty());
    let pw = json!({ "kind": "set_password", "grantee": grantee, "password": "new secret" });
    change(s, pw, Some("new secret")).await;
    for name in [ROLE, GROUP] {
        change(s, json!({ "kind": "drop_role", "name": name }), None).await;
    }
    let list = users::list(&s.pool).await.unwrap();
    assert!(!list.roles.iter().any(|r| r.name == ROLE || r.name == GROUP));
}

async fn sqlx_scalar_bool(pool: &DbPool, role: &str) -> bool {
    let DbPool::Postgres(p) = pool else {
        unreachable!()
    };
    sqlx::query_scalar("SELECT rolpassword IS NOT NULL FROM pg_authid WHERE rolname = $1")
        .bind(role)
        .fetch_one(p)
        .await
        .unwrap()
}

async fn backup_restore(s: &Session, admin: &str) {
    let Some(dump) = tool(s, "pg_dump").await else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let target = session(Dialect::Postgres, admin, RESTORE_DB, None).await;
    let count = "SELECT count(*) FROM items";

    // Plain SQL → psql.
    let plain = dir.path().join("plain.sql").display().to_string();
    let req = args::BackupRequest {
        path: plain.clone(),
        no_owner: true,
        ..Default::default()
    };
    let rep = backup(s, &req, &dump).await;
    assert!(!rep.lines.is_empty(), "pg_dump --verbose reports progress");
    let text = std::fs::read_to_string(&plain).unwrap();
    assert!(text.contains("CREATE TABLE public.items"));
    assert_eq!(
        tools::restore_tool(Dialect::Postgres, &plain).unwrap(),
        "psql"
    );
    let psql = tool(&target, "psql").await.expect("psql next to pg_dump");
    let req = args::RestoreRequest {
        path: plain.clone(),
        stop_on_error: true,
        ..Default::default()
    };
    let (r, rep) = restore(&target, &req, &psql).await;
    r.unwrap();
    let size = std::fs::metadata(&plain).unwrap().len();
    assert_eq!(rep.bytes, Some((size, Some(size))));
    assert_eq!(scalar_i64(&target.pool, count).await, 3);

    // Custom archive → pg_restore, dropping what is there first.
    let custom = dir.path().join("db.dump").display().to_string();
    let req = args::BackupRequest {
        path: custom.clone(),
        format: args::DumpFormat::Custom,
        ..Default::default()
    };
    backup(s, &req, &dump).await;
    assert_eq!(
        tools::restore_tool(Dialect::Postgres, &custom).unwrap(),
        "pg_restore"
    );
    let pg_restore = tool(&target, "pg_restore").await.expect("pg_restore");
    let req = args::RestoreRequest {
        path: custom,
        stop_on_error: true,
        clean: true,
        no_owner: true,
        ..Default::default()
    };
    restore(&target, &req, &pg_restore).await.0.unwrap();
    assert_eq!(scalar_i64(&target.pool, count).await, 3);

    stop_on_error(&target, &psql, dir.path()).await;
    target.close().await;
}

/// With "stop on error" nothing after the bad statement runs; without it, the rest does.
async fn stop_on_error(target: &Session, psql: &tools::ToolChoice, dir: &std::path::Path) {
    let bad = dir.join("bad.sql");
    std::fs::write(
        &bad,
        "CREATE TABLE before_error ();\nSELEC 1;\nCREATE TABLE after_error ();\n",
    )
    .unwrap();
    let mut req = args::RestoreRequest {
        path: bad.display().to_string(),
        stop_on_error: true,
        ..Default::default()
    };
    let (r, _) = restore(target, &req, psql).await;
    let e = r.unwrap_err();
    assert!(e.message.contains("syntax error"), "{}", e.message);
    let exists = "SELECT count(*) FROM pg_class WHERE relname = 'after_error'";
    assert_eq!(scalar_i64(&target.pool, exists).await, 0);
    req.stop_on_error = false;
    exec(&target.pool, "DROP TABLE before_error").await;
    let _ = restore(target, &req, psql).await;
    assert_eq!(scalar_i64(&target.pool, exists).await, 1);
}
