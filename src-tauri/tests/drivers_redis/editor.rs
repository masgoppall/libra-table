//! The query editor: commands per line, result tables, errors, a session that lasts
//! between runs, cancelling a blocking command and read-only refusal.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use libratable_lib::db::{open_pool, DbPool};
use libratable_lib::error::ErrorKind;
use libratable_lib::safe_mode;
use libratable_lib::state::Session;
use serde_json::json;

use super::{key, run, Server};

pub async fn run_all(s: &Arc<Session>, server: &Server) {
    results(s).await;
    session_state(s).await;
    cancel(s).await;
    read_only(server);
    acl_user(s, server).await;
    startup_query(server).await;
}

async fn results(s: &Session) {
    let q = key("q");
    let script = format!(
        "SET {q} 1\nINCR {q}\n# a comment\nGET {q}\nHGETALL {h}\nGET {q}:none\nINFO server",
        h = key("hash")
    );
    let r = run(s, &script).await;
    assert_eq!(r.len(), 6);
    assert!(
        r.iter().all(|x| x.error.is_none()),
        "{:?}",
        r.iter().map(|x| &x.error).collect::<Vec<_>>()
    );
    assert_eq!(r[0].rows, vec![vec![json!("OK")]]);
    assert_eq!(r[1].rows, vec![vec![json!(2)]]);
    assert_eq!(r[2].rows, vec![vec![json!("2")]]);
    assert_eq!(r[3].columns[0].name, "field");
    assert_eq!(r[3].rows.len(), 2);
    assert_eq!(r[4].rows, vec![vec![serde_json::Value::Null]]);
    assert!(r[5].rows.iter().any(|row| row[1] == json!("redis_version")));

    // The first error stops the script.
    let r = run(s, &format!("HGET {q} f\nDEL {q}")).await;
    assert_eq!(r.len(), 1);
    let err = r[0].error.as_ref().expect("WRONGTYPE");
    assert!(err.message.starts_with("WRONGTYPE"), "{}", err.message);
    let r = run(s, "SET a \"open").await;
    assert_eq!(r[0].error.as_ref().unwrap().kind, ErrorKind::Validation);
    let r = run(s, "SUBSCRIBE news").await;
    assert_eq!(r[0].error.as_ref().unwrap().kind, ErrorKind::Unsupported);
    run(s, &format!("DEL {q}")).await;
}

async fn session_state(s: &Session) {
    // The editor keeps its connection: names and transactions span runs.
    run(s, "CLIENT SETNAME libratable-test").await;
    let r = run(s, "CLIENT GETNAME").await;
    assert_eq!(r[0].rows, vec![vec![json!("libratable-test")]]);
    let t = key("tx");
    run(s, &format!("MULTI\nSET {t} 1")).await;
    let r = run(s, &format!("INCR {t}\nEXEC")).await;
    assert_eq!(r[0].rows, vec![vec![json!("QUEUED")]]);
    assert_eq!(r[1].rows.len(), 2, "{:?}", r[1].rows);
    run(s, &format!("DEL {t}")).await;
    // SELECT changes only the editor's database.
    let r = run(s, "SELECT 14\nDBSIZE\nSELECT 15").await;
    assert!(r.iter().all(|x| x.error.is_none()));
}

async fn cancel(s: &Arc<Session>) {
    let DbPool::Engine(e) = &s.pool else {
        panic!("not an engine")
    };
    let e = e.clone();
    let started = Instant::now();
    let runner = {
        let e = e.clone();
        tokio::spawn(async move {
            let flag = AtomicBool::new(false);
            let stmts = [format!("BLPOP {} 20", key("never"))];
            e.run(&stmts, 10, &flag).await
        })
    };
    tokio::time::sleep(Duration::from_millis(500)).await;
    e.cancel().await.unwrap();
    let r = runner.await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "BLPOP was not unblocked"
    );
    assert!(r[0].error.is_some(), "{:?}", r[0].rows);
}

fn read_only(server: &Server) {
    let conn = server.connection(true);
    let stmts = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let refused = safe_mode::read_only_violations(&conn, &stmts(&["SET a 1", "FLUSHDB", "x"]));
    assert_eq!(refused.len(), 3);
    let allowed = safe_mode::read_only_violations(&conn, &stmts(&["GET a", "SELECT 1", "SCAN 0"]));
    assert!(allowed.is_empty());
}

/// An ACL user (`libratable_test`) logs in with its own password; a wrong one fails.
async fn acl_user(s: &Session, server: &Server) {
    let made = run(s, "ACL SETUSER libratable_test on >lt-secret ~* &* +@all").await;
    assert!(made[0].error.is_none(), "{:?}", made[0].error);
    let mut conn = server.connection(false);
    conn.db.user = "libratable_test".into();
    let who = match open_pool(&conn, None, Some("lt-secret"), None, 1).await {
        Ok(pool) => {
            let v = pool.server_version().await;
            pool.close().await;
            v.map_err(|e| e.message)
        }
        Err(e) => Err(e.message),
    };
    let wrong = open_pool(&conn, None, Some("nope"), None, 1).await;
    run(s, "ACL DELUSER libratable_test").await;
    assert!(who.is_ok(), "{who:?}");
    assert_eq!(wrong.err().map(|e| e.kind), Some(ErrorKind::DbAuth));
}

/// The connection's startup commands run on the editor connection when it opens.
async fn startup_query(server: &Server) {
    let mut conn = server.connection(false);
    conn.options.startup_query = "CLIENT SETNAME lt-startup\n# comment".into();
    let pool = open_pool(&conn, None, server.password.as_deref(), None, 1)
        .await
        .unwrap();
    let DbPool::Engine(e) = &pool else {
        panic!("not an engine")
    };
    let r = e
        .run(&["CLIENT GETNAME".to_string()], 10, &AtomicBool::new(false))
        .await;
    pool.close().await;
    assert_eq!(r[0].rows, vec![vec![json!("lt-startup")]]);
}
