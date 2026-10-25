//! Snowflake against an in-test mock of its REST API: the requests the driver sends
//! (login body, token header, bindings, abort, logout) and how it reads the answers
//! (rowtype decoding, result chunks, DML counts, long-query polling).

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use libratable_lib::db::data;
use libratable_lib::db::meta;
use libratable_lib::error::ErrorKind;
use libratable_lib::export::Source;
use libratable_lib::safe_mode;
use serde_json::{json, Value};

use crate::common::*;
use crate::mock::{self, Mock, Reply, Request};

const PASSWORD: &str = "s3cret-pass";
const DB: &str = "LIBRATABLE_DB";
const TABLE: &str = "LIBRATABLE_PEOPLE";

#[derive(Default)]
struct State {
    logins: u32,
    aborted: bool,
}

fn ok(data: Value) -> Reply {
    Reply::json(
        200,
        json!({ "data": data, "code": null, "message": null, "success": true }),
    )
}

fn fail(code: &str, message: &str) -> Reply {
    Reply::json(
        200,
        json!({ "data": { "sqlState": "42000", "queryId": "q-err" }, "code": code,
            "message": message, "success": false }),
    )
}

fn col(name: &str, kind: &str, scale: Option<i64>) -> Value {
    json!({ "name": name, "type": kind, "precision": 38, "scale": scale, "nullable": true })
}

fn result(cols: Vec<Value>, rows: Value) -> Reply {
    let total = rows.as_array().map_or(0, Vec::len);
    ok(
        json!({ "rowtype": cols, "rowset": rows, "total": total, "queryId": "q-1",
        "statementTypeId": 4096 }),
    )
}

fn text_cols(names: &[&str]) -> Vec<Value> {
    names.iter().map(|n| col(n, "text", None)).collect()
}

fn gzip(text: &str) -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(text.as_bytes()).unwrap();
    gz.finish().unwrap()
}

fn people_cols() -> Vec<Value> {
    vec![
        col("ID", "fixed", Some(0)),
        col("NAME", "text", None),
        col("BORN", "date", None),
    ]
}

fn query(sql: &str, binds: &Value, base: &str, st: &mut State) -> Reply {
    let table = format!("\"{DB}\".\"PUBLIC\".\"{TABLE}\"");
    match sql {
        "SHOW DATABASES" => result(text_cols(&["created_on", "name"]), json!([["0", DB]])),
        s if s == format!("SHOW SCHEMAS IN DATABASE \"{DB}\"") => result(
            text_cols(&["created_on", "name"]),
            json!([["0", "INFORMATION_SCHEMA"], ["0", "PUBLIC"]]),
        ),
        s if s.starts_with("SHOW TABLES IN SCHEMA") => {
            result(text_cols(&["created_on", "name"]), json!([["0", TABLE]]))
        }
        s if s.starts_with("SHOW VIEWS IN SCHEMA") => result(
            text_cols(&["created_on", "name"]),
            json!([["0", "LIBRATABLE_V"]]),
        ),
        s if s == format!("DESCRIBE TABLE {table}") => result(
            text_cols(&[
                "name",
                "type",
                "kind",
                "null?",
                "default",
                "primary key",
                "comment",
            ]),
            json!([
                [
                    "ID",
                    "NUMBER(38,0)",
                    "COLUMN",
                    "N",
                    "IDENTITY START 1 INCREMENT 1",
                    "Y",
                    null
                ],
                [
                    "NAME",
                    "VARCHAR(100)",
                    "COLUMN",
                    "Y",
                    null,
                    "N",
                    "full name"
                ],
                ["BORN", "DATE", "COLUMN", "Y", null, "N", null]
            ]),
        ),
        s if s == format!("SHOW PRIMARY KEYS IN TABLE {table}") => result(
            text_cols(&["column_name", "key_sequence"]),
            json!([["ID", "1"]]),
        ),
        s if s.starts_with("SHOW IMPORTED KEYS") => result(text_cols(&["fk_name"]), json!([])),
        s if s.starts_with("SELECT GET_DDL('TABLE'") => result(
            text_cols(&["GET_DDL"]),
            json!([["create or replace TABLE LIBRATABLE_PEOPLE (ID NUMBER(38,0));"]]),
        ),
        s if s.starts_with("SELECT \"ID\", \"NAME\", \"BORN\" FROM") => result(
            people_cols(),
            json!([["2", "Bob", "19725"], ["1", "Ann", null]]),
        ),
        s if s.starts_with("SELECT COUNT(*)") => {
            result(vec![col("COUNT(*)", "fixed", Some(0))], json!([["2"]]))
        }
        "BEGIN" | "COMMIT" | "ROLLBACK" => result(text_cols(&["status"]), json!([["ok"]])),
        s if s.starts_with("UPDATE") || s.starts_with("DELETE") => {
            let stale = binds.get("2").and_then(|b| b.get("value")) == Some(&json!("999"));
            let n = if stale { "0" } else { "1" };
            ok(
                json!({ "rowtype": [col("number of rows updated", "fixed", Some(0)),
                col("number of multi-joined rows updated", "fixed", Some(0))],
                "rowset": [[n, "0"]], "total": 1, "statementTypeId": 0x3200 }),
            )
        }
        "SELECT * FROM BIG" => ok(json!({
            "rowtype": people_cols(), "rowset": [["1", "Ann", null], ["2", "Bob", "19725"]],
            "total": 4, "statementTypeId": 4096, "qrmk": "chunk-key",
            "chunks": [{ "url": format!("{base}/chunk/1"), "rowCount": 2 }]
        })),
        "SELECT SYSTEM$WAIT(10)" => Reply::json(
            200,
            json!({ "data": { "getResultUrl": "/queries/q-long/result", "queryId": "q-long" },
                "code": "333334", "message": "running", "success": true }),
        ),
        _ if st.aborted => fail("000604", "SQL execution canceled"),
        _ => fail("000904", "SQL compilation error: invalid identifier 'NOPE'"),
    }
}

fn handle(req: &Request, base: &str, st: &Mutex<State>) -> Reply {
    let mut st = st.lock().unwrap();
    match (req.method.as_str(), req.path()) {
        ("POST", "/session/v1/login-request") => {
            let body = req.json();
            if body["data"]["PASSWORD"] != PASSWORD {
                return fail("390100", "Incorrect username or password was specified.");
            }
            st.logins += 1;
            ok(
                json!({ "token": format!("tok-{}", st.logins), "masterToken": "master",
                "serverVersion": "8.40.1", "sessionInfo": { "databaseName": DB } }),
            )
        }
        ("POST", "/queries/v1/query-request") => {
            let body = req.json();
            let sql = body["sqlText"].as_str().unwrap_or("");
            query(sql, &body["bindings"], base, &mut st)
        }
        ("GET", "/queries/q-long/result") if st.aborted => fail("000604", "SQL execution canceled"),
        ("GET", "/queries/q-long/result") => Reply::json(
            200,
            json!({ "data": { "getResultUrl": "/queries/q-long/result" }, "code": "333333",
                "success": true }),
        )
        .after(100),
        ("GET", "/chunk/1") => Reply::bytes(gzip(r#"["3","Cy","0"],["4","Di",null]"#)),
        ("POST", "/queries/v1/abort-request") => {
            st.aborted = true;
            ok(json!({}))
        }
        ("POST", "/session") => ok(json!({})),
        _ => Reply::json(404, json!({})),
    }
}

async fn server() -> Mock {
    let state = Arc::new(Mutex::new(State::default()));
    let base = Arc::new(Mutex::new(String::new()));
    let b2 = base.clone();
    let m = mock::start(Arc::new(move |r: &Request| {
        let base = b2.lock().unwrap().clone();
        handle(r, &base, &state)
    }))
    .await;
    *base.lock().unwrap() = m.url.clone();
    m
}

fn conn(m: &Mock, read_only: bool) -> libratable_lib::models::Connection {
    connection(
        "snowflake",
        json!({ "user": "TESTER", "database": DB, "passwordMode": "keychain", "extra": {
            "account": "xy12345.eu-central-1", "warehouse": "WH", "role": "ANALYST",
            "endpoint": m.url } }),
        read_only,
    )
}

fn sql_of(r: &Request) -> String {
    r.json()["sqlText"].as_str().unwrap_or("").to_string()
}

#[tokio::test]
async fn snowflake_against_mock() {
    let m = server().await;

    // Wrong password: an auth error that doesn't repeat the password.
    let bad = try_session(conn(&m, false), Some("wrong-pass"))
        .await
        .err()
        .unwrap();
    assert_eq!(bad.kind, ErrorKind::DbAuth);
    assert!(!bad.message.contains("wrong-pass"));

    let s = try_session(conn(&m, false), Some(PASSWORD)).await.unwrap();
    assert_eq!(s.server_version, "8.40.1");
    assert_eq!(s.database, DB);
    let login = &m.find(|r| r.path() == "/session/v1/login-request")[1];
    assert_eq!(login.query("databaseName").as_deref(), Some(DB));
    assert_eq!(login.query("warehouse").as_deref(), Some("WH"));
    assert_eq!(login.query("roleName").as_deref(), Some("ANALYST"));
    let body = login.json();
    assert_eq!(body["data"]["ACCOUNT_NAME"], "xy12345");
    assert_eq!(body["data"]["LOGIN_NAME"], "TESTER");
    assert_eq!(
        body["data"]["SESSION_PARAMETERS"]["QUERY_RESULT_FORMAT"],
        "JSON"
    );

    // Catalog.
    assert_eq!(
        meta::list_databases(&s.pool, false).await.unwrap(),
        vec![DB]
    );
    assert_eq!(
        meta::list_schemas(&s.pool, false).await.unwrap(),
        vec!["PUBLIC"]
    );
    let objects = meta::list_objects(&s.pool, "PUBLIC").await.unwrap();
    let kinds: Vec<(String, &str)> = objects.iter().map(|o| (o.name.clone(), o.kind)).collect();
    assert_eq!(
        kinds,
        vec![(TABLE.into(), "table"), ("LIBRATABLE_V".into(), "view")]
    );
    let cols = s.columns("PUBLIC", TABLE).await.unwrap();
    assert_eq!(cols[0].pk_position, Some(1));
    assert!(cols[0].auto_increment && !cols[0].nullable);
    assert_eq!(cols[1].comment.as_deref(), Some("full name"));
    let st = meta::table_structure(&s.pool, "PUBLIC", TABLE)
        .await
        .unwrap();
    assert!(st.ddl.starts_with("create or replace TABLE"));
    assert!(st.indexes[0].primary);

    // Every query carries the session token and asks for Snowflake's JSON format.
    let q = &m.find(|r| r.path() == "/queries/v1/query-request")[0];
    assert_eq!(q.header("authorization"), Some("Snowflake Token=\"tok-1\""));
    assert_eq!(q.header("accept"), Some("application/snowflake"));
    assert!(q.query("requestId").is_some());

    // Grid: filters are TEXT bindings, sort and paging in the SQL, values decoded.
    let req = sorted(
        page(
            "PUBLIC",
            TABLE,
            50,
            vec![filter("NAME", "contains", "o%"), filter("ID", "gt", "0")],
        ),
        "NAME",
        true,
    );
    let res = data::fetch_page(&s, &req).await.unwrap();
    assert_eq!(
        res.rows[0],
        vec![json!(2), json!("Bob"), json!("2024-01-03")]
    );
    assert!(res.sql.contains("LIKE '%o!%%' ESCAPE '!'"), "{}", res.sql);
    let sent = m
        .find(|r| sql_of(r).starts_with("SELECT \"ID\""))
        .pop()
        .unwrap()
        .json();
    assert_eq!(
        sent["sqlText"],
        concat!(
            r#"SELECT "ID", "NAME", "BORN" FROM "LIBRATABLE_DB"."PUBLIC"."LIBRATABLE_PEOPLE" "#,
            r#"WHERE (TO_VARCHAR("NAME") LIKE ? ESCAPE '!') AND ("ID" > CAST(? AS NUMBER(38,0))) "#,
            r#"ORDER BY "NAME" DESC LIMIT 50 OFFSET 0"#
        )
    );
    assert_eq!(
        sent["bindings"]["1"],
        json!({ "type": "TEXT", "value": "%o!%%" })
    );
    assert_eq!(
        sent["bindings"]["2"],
        json!({ "type": "TEXT", "value": "0" })
    );
    let count = data::count_rows(&s, &req).await.unwrap();
    assert_eq!((count.total, count.exact), (2, true));
    let preview = data::preview_filter(&s, &req).await.unwrap();
    assert!(preview
        .all
        .contains(r#"("ID" > CAST('0' AS NUMBER(38,0)))"#));

    // Edits: one transaction, and a stale row rolls everything back.
    let ok_change = update(json!({"ID": 1}), json!({"NAME": "Anna"}));
    assert_eq!(
        apply(&s, "PUBLIC", TABLE, std::slice::from_ref(&ok_change))
            .await
            .unwrap(),
        1
    );
    let stale = update(json!({"ID": 999}), json!({"NAME": "Ghost"}));
    let err = apply(&s, "PUBLIC", TABLE, &[ok_change, stale])
        .await
        .err()
        .unwrap();
    assert!(
        err.message.contains("Change 2 affected 0 rows"),
        "{}",
        err.message
    );
    let tx: Vec<String> = m
        .find(|r| {
            let q = sql_of(r);
            ["BEGIN", "COMMIT", "ROLLBACK"].contains(&q.as_str()) || q.starts_with("UPDATE")
        })
        .iter()
        .map(|r| sql_of(r).split(' ').next().unwrap().to_string())
        .collect();
    assert_eq!(
        tx,
        ["BEGIN", "UPDATE", "COMMIT", "BEGIN", "UPDATE", "UPDATE", "ROLLBACK"]
    );

    // Editor: its own login, result chunks, stop at the first error.
    let results = run(&s, &["SELECT * FROM BIG", "SELECT nope", "SELECT 1"]).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].rows.len(), 4);
    assert_eq!(
        results[0].rows[2],
        vec![json!(3), json!("Cy"), json!("1970-01-01")]
    );
    assert!(results[1]
        .error
        .as_ref()
        .unwrap()
        .message
        .contains("invalid identifier"));
    let chunk = &m.find(|r| r.path() == "/chunk/1")[0];
    assert_eq!(
        chunk.header("x-amz-server-side-encryption-customer-key"),
        Some("chunk-key")
    );
    let editor_q = m.find(|r| sql_of(r) == "SELECT * FROM BIG");
    assert_eq!(
        editor_q[0].header("authorization"),
        Some("Snowflake Token=\"tok-2\"")
    );

    // Export of a query (chunks included).
    let lines = export_csv(
        &s,
        Source::Query {
            sql: "SELECT * FROM BIG".into(),
        },
    )
    .await;
    assert_eq!(lines.len(), 5);

    cancel_long_query(&s, &m).await;
    read_only(&m).await;

    s.close().await;
    assert!(!m
        .find(|r| r.path() == "/session" && r.query("delete").as_deref() == Some("true"))
        .is_empty());
    let leaked = m
        .requests()
        .iter()
        .filter(|r| r.path() != "/session/v1/login-request")
        .any(|r| String::from_utf8_lossy(&r.body).contains(PASSWORD));
    assert!(!leaked, "the password went somewhere other than the login");
}

/// A long query polls getResultUrl; cancelling sends abort-request for its request id.
async fn cancel_long_query(s: &Arc<libratable_lib::state::Session>, m: &Mock) {
    let e = engine(s);
    let flag = Arc::new(AtomicBool::new(false));
    let (e2, f2) = (e.clone(), flag.clone());
    let task = tokio::spawn(async move {
        e2.run(&["SELECT SYSTEM$WAIT(10)".to_string()], 100, &f2)
            .await
    });
    tokio::time::sleep(Duration::from_millis(400)).await;
    flag.store(true, Ordering::SeqCst);
    e.cancel().await.unwrap();
    let results = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        results[0].error.as_ref().unwrap().kind,
        ErrorKind::Cancelled
    );
    let sent = m
        .find(|r| r.path() == "/queries/v1/query-request" && sql_of(r) == "SELECT SYSTEM$WAIT(10)")
        .pop()
        .unwrap();
    let abort = m
        .find(|r| r.path() == "/queries/v1/abort-request")
        .pop()
        .unwrap();
    assert_eq!(
        abort.json()["requestId"].as_str(),
        sent.query("requestId").as_deref()
    );
    assert!(!m.find(|r| r.path() == "/queries/q-long/result").is_empty());
}

/// Read-only: writes are refused app-side (there is no session switch to set).
async fn read_only(m: &Mock) {
    let c = conn(m, true);
    let stmts = vec!["UPDATE T SET A = 1".to_string(), "SHOW TABLES".to_string()];
    let refused = safe_mode::read_only_violations(&c, &stmts);
    assert_eq!(refused.len(), 1);
    let s = try_session(c, Some(PASSWORD)).await.unwrap();
    let change = update(json!({"ID": 1}), json!({"NAME": "x"}));
    let err = apply(&s, "PUBLIC", TABLE, &[change]).await.err().unwrap();
    assert_eq!(err.kind, ErrorKind::ReadOnly);
    s.close().await;
}
