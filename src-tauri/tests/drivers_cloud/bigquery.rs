//! BigQuery: the service-account JWT, token caching, job polling, result paging and
//! cancel against a mock (always), and the data paths against the emulator.

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use futures::FutureExt;
use libratable_lib::db::{data, meta};
use libratable_lib::error::ErrorKind;
use libratable_lib::safe_mode;
use ring::signature::{UnparsedPublicKey, RSA_PKCS1_2048_8192_SHA256};
use rsa::pkcs1::EncodeRsaPublicKey;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use serde_json::{json, Value};

use crate::common::*;
use crate::mock::{self, Mock, Reply, Request};

const PROJECT: &str = "libratable-test";
const DATASET: &str = "libratable_ds";
const TABLE: &str = "libratable_people";
const TOKEN: &str = "ya29.mock-token";

/// A throwaway service-account key file and the DER of its public key.
fn throwaway_key(token_uri: &str) -> (String, Vec<u8>) {
    let key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).unwrap();
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    let public = key
        .to_public_key()
        .to_pkcs1_der()
        .unwrap()
        .as_bytes()
        .to_vec();
    let file = json!({
        "type": "service_account", "project_id": PROJECT, "private_key_id": "kid-1",
        "private_key": pem, "client_email": "tester@libratable-test.iam.gserviceaccount.com",
        "token_uri": token_uri,
    });
    (file.to_string(), public)
}

/// Checks the JWT bearer grant the way Google's token endpoint would.
fn token_reply(req: &Request, public: &[u8], base: &str) -> Reply {
    let form: Vec<(String, String)> = url::form_urlencoded::parse(&req.body)
        .into_owned()
        .collect();
    let get = |k: &str| form.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    assert_eq!(
        get("grant_type").as_deref(),
        Some("urn:ietf:params:oauth:grant-type:jwt-bearer")
    );
    let jwt = get("assertion").unwrap();
    let parts: Vec<&str> = jwt.split('.').collect();
    let signed = format!("{}.{}", parts[0], parts[1]);
    let sig = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    let verified = UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, public)
        .verify(signed.as_bytes(), &sig)
        .is_ok();
    if !verified {
        return Reply::json(
            400,
            json!({"error": "invalid_grant", "error_description": "Invalid JWT Signature."}),
        );
    }
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(claims["aud"], format!("{base}/token"));
    assert_eq!(claims["scope"], "https://www.googleapis.com/auth/bigquery");
    assert_eq!(
        claims["iss"],
        "tester@libratable-test.iam.gserviceaccount.com"
    );
    Reply::json(
        200,
        json!({"access_token": TOKEN, "expires_in": 3600, "token_type": "Bearer"}),
    )
}

fn people_schema() -> Value {
    json!({"fields": [
        {"name": "id", "type": "INTEGER", "mode": "REQUIRED"},
        {"name": "name", "type": "STRING", "mode": "NULLABLE"},
        {"name": "tags", "type": "STRING", "mode": "REPEATED"}
    ]})
}

fn api_reply(req: &Request) -> Reply {
    let p = format!("/bigquery/v2/projects/{PROJECT}");
    let path = req.path().to_string();
    let job = |id: &str| json!({"projectId": PROJECT, "jobId": id, "location": "EU"});
    match (req.method.as_str(), path.as_str()) {
        ("GET", "/bigquery/v2/projects") => Reply::json(
            200,
            json!({"projects": [
            {"projectReference": {"projectId": PROJECT}},
            {"id": "other-proj", "projectReference": {"projectId": "other-proj"}}]}),
        ),
        ("GET", s) if s == format!("{p}/datasets") => Reply::json(
            200,
            json!({"datasets": [
            {"datasetReference": {"projectId": PROJECT, "datasetId": DATASET}}]}),
        ),
        ("GET", s) if s == format!("{p}/datasets/{DATASET}/tables") => {
            if req.query("pageToken").as_deref() == Some("t2") {
                Reply::json(
                    200,
                    json!({"tables": [{"tableReference": {"tableId": "libratable_v"},
                    "type": "VIEW"}]}),
                )
            } else {
                Reply::json(
                    200,
                    json!({"tables": [{"tableReference": {"tableId": TABLE},
                    "type": "TABLE"}], "nextPageToken": "t2"}),
                )
            }
        }
        ("GET", s) if s == format!("{p}/datasets/{DATASET}/tables/{TABLE}") => Reply::json(
            200,
            json!({"type": "TABLE", "numRows": "3", "schema": people_schema(),
                "tableConstraints": {"primaryKey": {"columns": ["id"]}}}),
        ),
        ("POST", s) if s == format!("{p}/queries") => {
            let q = req.json()["query"].as_str().unwrap_or("").to_string();
            if q == "SELECT SLOW" {
                Reply::json(
                    200,
                    json!({"jobComplete": false, "jobReference": job("job-slow")}),
                )
            } else if q.starts_with("DECLARE libratable_n") {
                Reply::json(
                    200,
                    json!({"jobComplete": true, "jobReference": job("job-dml")}),
                )
            } else {
                Reply::json(
                    200,
                    json!({"jobComplete": false, "jobReference": job("job-1")}),
                )
            }
        }
        ("GET", s) if s == format!("{p}/queries/job-slow") => Reply::json(
            200,
            json!({"jobComplete": false, "jobReference": job("job-slow")}),
        )
        .after(100),
        ("GET", s) if s == format!("{p}/queries/job-1") => {
            let row =
                |id: &str, name: &str| json!({"f": [{"v": id}, {"v": name}, {"v": [{"v": "t"}]}]});
            if req.query("pageToken").as_deref() == Some("pg2") {
                Reply::json(200, json!({"jobComplete": true, "rows": [row("2", "Bob")]}))
            } else {
                Reply::json(
                    200,
                    json!({"jobComplete": true, "schema": people_schema(),
                    "totalRows": "2", "rows": [row("1", "Ann")], "pageToken": "pg2",
                    "jobReference": job("job-1")}),
                )
            }
        }
        ("POST", s) if s == format!("{p}/jobs/job-slow/cancel") => Reply::json(200, json!({})),
        _ => Reply::json(404, json!({"error": {"code": 404, "message": "nope"}})),
    }
}

async fn mock_server(public: Vec<u8>) -> Mock {
    let base = Arc::new(Mutex::new(String::new()));
    let b2 = base.clone();
    let m = mock::start(Arc::new(move |r: &Request| {
        if r.path() == "/token" {
            return token_reply(r, &public, &b2.lock().unwrap());
        }
        if r.header("authorization") != Some(&format!("Bearer {TOKEN}")) {
            return Reply::json(401, json!({"error": {"code": 401, "message": "no token"}}));
        }
        api_reply(r)
    }))
    .await;
    *base.lock().unwrap() = m.url.clone();
    m
}

fn conn(endpoint: &str, read_only: bool) -> libratable_lib::models::Connection {
    connection(
        "bigquery",
        json!({ "database": DATASET, "passwordMode": "keychain", "extra": {
            "project": PROJECT, "location": "EU", "endpoint": endpoint } }),
        read_only,
    )
}

#[tokio::test]
async fn bigquery_jwt_and_jobs_against_mock() {
    // The mock trusts only the first key; the second one is refused at the token step.
    let (key, public) = throwaway_key("http://placeholder/token");
    let m = mock_server(public).await;
    let token_uri = format!("{}/token", m.url);
    let key = key.replace("http://placeholder/token", &token_uri);
    let (other, _) = throwaway_key(&token_uri);

    let bad = try_session(conn(&m.url, false), Some(&other))
        .await
        .err()
        .unwrap();
    assert_eq!(bad.kind, ErrorKind::DbAuth);
    assert!(bad.message.contains("Invalid JWT Signature"));

    let s = try_session(conn(&m.url, false), Some(&key)).await.unwrap();
    assert_eq!(s.database, PROJECT);
    assert_eq!(
        meta::list_databases(&s.pool, false).await.unwrap(),
        vec!["libratable-test", "other-proj"]
    );
    assert_eq!(
        meta::list_schemas(&s.pool, false).await.unwrap(),
        vec![DATASET]
    );
    let objects = meta::list_objects(&s.pool, DATASET).await.unwrap();
    assert_eq!(objects[1].name, "libratable_v");
    assert_eq!(objects[1].kind, "view");
    let cols = s.columns(DATASET, TABLE).await.unwrap();
    assert_eq!(cols[0].pk_position, Some(1));
    assert_eq!(cols[2].data_type, "ARRAY<STRING>");

    // A page: polled until complete, then the second result page fetched.
    let req = page(DATASET, TABLE, 10, vec![filter("id", "gte", "1")]);
    let res = data::fetch_page(&s, &req).await.unwrap();
    assert_eq!(res.rows.len(), 2);
    assert_eq!(res.rows[1], vec![json!(2), json!("Bob"), json!(["t"])]);
    let post = m
        .find(|r| r.method == "POST" && r.path().ends_with("/queries"))
        .remove(0);
    let body = post.json();
    assert_eq!(body["useLegacySql"], false);
    assert_eq!(body["location"], "EU");
    assert_eq!(body["parameterMode"], "NAMED");
    assert_eq!(
        body["queryParameters"],
        json!([{"name": "p1", "parameterType": {"type": "STRING"},
            "parameterValue": {"value": "1"}}])
    );
    assert!(body["query"]
        .as_str()
        .unwrap()
        .ends_with("WHERE (`id` >= CAST(@p1 AS INT64)) LIMIT 10 OFFSET 0"));
    let polls = m.find(|r| r.path().ends_with("/queries/job-1"));
    assert_eq!(polls.len(), 2);
    assert!(polls
        .iter()
        .all(|r| r.query("location").as_deref() == Some("EU")));

    // Unfiltered counts come from the table metadata, not a query job.
    let jobs_before = m.find(|r| r.method == "POST").len();
    let n = data::count_rows(&s, &page(DATASET, TABLE, 10, vec![]))
        .await
        .unwrap();
    assert_eq!((n.total, n.exact), (3, true));
    assert_eq!(m.find(|r| r.method == "POST").len(), jobs_before);

    // Edits: one transaction script with named parameters.
    let change = update(json!({"id": 1}), json!({"name": "Anna"}));
    assert_eq!(apply(&s, DATASET, TABLE, &[change]).await.unwrap(), 1);
    let script = m
        .find(|r| {
            r.json()["query"]
                .as_str()
                .is_some_and(|q| q.starts_with("DECLARE"))
        })
        .remove(0)
        .json();
    let q = script["query"].as_str().unwrap();
    assert!(q.contains("BEGIN TRANSACTION;") && q.contains("COMMIT TRANSACTION;"));
    assert_eq!(script["queryParameters"].as_array().unwrap().len(), 2);

    // Cancel: the running job gets jobs.cancel.
    let e = engine(&s);
    let flag = Arc::new(AtomicBool::new(false));
    let (e2, f2) = (e.clone(), flag.clone());
    let task = tokio::spawn(async move { e2.run(&["SELECT SLOW".to_string()], 10, &f2).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
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
    let cancel = m.find(|r| r.path().ends_with("/jobs/job-slow/cancel"));
    assert!(!cancel.is_empty());
    assert_eq!(cancel[0].query("location").as_deref(), Some("EU"));

    // One token for the whole session (plus the refused one).
    assert_eq!(m.find(|r| r.path() == "/token").len(), 2);
}

/// Raw REST calls to set up and clean up the emulator.
async fn emulator(method: reqwest::Method, url: String, body: Option<Value>) -> Value {
    let mut req = reqwest::Client::new().request(method, url);
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.send().await.unwrap();
    let text = resp.text().await.unwrap_or_default();
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

#[tokio::test]
async fn bigquery_against_emulator() {
    let Ok(endpoint) = std::env::var("LIBRATABLE_TEST_BIGQUERY_ENDPOINT") else {
        eprintln!("skipped: set LIBRATABLE_TEST_BIGQUERY_ENDPOINT to run");
        return;
    };
    let base = format!("{endpoint}/bigquery/v2/projects/{PROJECT}");
    let table_url = format!("{base}/datasets/{DATASET}/tables/{TABLE}");
    emulator(reqwest::Method::DELETE, table_url.clone(), None).await;
    let create = json!({
        "tableReference": {"projectId": PROJECT, "datasetId": DATASET, "tableId": TABLE},
        "schema": {"fields": [
            {"name": "id", "type": "INT64", "mode": "REQUIRED"},
            {"name": "name", "type": "STRING"},
            {"name": "score", "type": "FLOAT64"},
            {"name": "tags", "type": "STRING", "mode": "REPEATED"}]},
        "tableConstraints": {"primaryKey": {"columns": ["id"]}}
    });
    emulator(
        reqwest::Method::POST,
        format!("{base}/datasets/{DATASET}/tables"),
        Some(create),
    )
    .await;
    let insert = format!(
        "INSERT INTO {DATASET}.{TABLE} (id, name, score, tags) VALUES \
         (1, 'Ann', 10.5, ['a']), (2, 'Bob', 20, []), (3, 'Cy', NULL, ['b', 'c'])"
    );
    emulator(
        reqwest::Method::POST,
        format!("{base}/queries"),
        Some(json!({"query": insert, "useLegacySql": false})),
    )
    .await;

    let outcome = AssertUnwindSafe(emulator_scenario(&endpoint))
        .catch_unwind()
        .await;
    emulator(reqwest::Method::DELETE, table_url, None).await;
    if let Err(p) = outcome {
        std::panic::resume_unwind(p);
    }
}

async fn emulator_scenario(endpoint: &str) {
    let s = try_session(conn(endpoint, false), None).await.unwrap();
    assert!(meta::list_databases(&s.pool, false)
        .await
        .unwrap()
        .contains(&PROJECT.to_string()));
    assert_eq!(
        meta::list_schemas(&s.pool, false).await.unwrap()[0],
        DATASET
    );
    let objects = meta::list_objects(&s.pool, DATASET).await.unwrap();
    assert!(objects.iter().any(|o| o.name == TABLE));
    let cols = s.columns(DATASET, TABLE).await.unwrap();
    assert_eq!(cols[0].pk_position, Some(1));
    let st = meta::table_structure(&s.pool, DATASET, TABLE)
        .await
        .unwrap();
    assert!(
        st.ddl.contains("PRIMARY KEY (`id`) NOT ENFORCED"),
        "{}",
        st.ddl
    );

    // Filter (case-insensitive contains), sort, paging, count.
    let names = |rows: &[Vec<Value>]| -> Vec<String> {
        rows.iter()
            .map(|r| r[1].as_str().unwrap_or("").to_string())
            .collect()
    };
    let req = page(DATASET, TABLE, 10, vec![filter("name", "contains", "B")]);
    assert_eq!(
        names(&data::fetch_page(&s, &req).await.unwrap().rows),
        ["Bob"]
    );
    let mut req = sorted(page(DATASET, TABLE, 2, vec![]), "id", true);
    let first = data::fetch_page(&s, &req).await.unwrap();
    assert_eq!(names(&first.rows), ["Cy", "Bob"]);
    assert_eq!(first.rows[0][3], json!(["b", "c"]));
    req.page = 1;
    assert_eq!(
        names(&data::fetch_page(&s, &req).await.unwrap().rows),
        ["Ann"]
    );
    let scored = page(DATASET, TABLE, 10, vec![filter("score", "gt", "15")]);
    assert_eq!(data::count_rows(&s, &scored).await.unwrap().total, 1);
    assert_eq!(
        data::count_rows(&s, &page(DATASET, TABLE, 10, vec![]))
            .await
            .unwrap()
            .total,
        3
    );

    // Edits by the declared key. (The emulator ignores IF/RAISE in scripts, so the
    // one-row check itself is only covered by the unit tests.)
    let change = update(json!({"id": 2}), json!({"name": "Bobby"}));
    assert_eq!(apply(&s, DATASET, TABLE, &[change]).await.unwrap(), 1);
    let bob = page(DATASET, TABLE, 10, vec![filter("id", "eq", "2")]);
    assert_eq!(
        names(&data::fetch_page(&s, &bob).await.unwrap().rows),
        ["Bobby"]
    );

    // Editor: rows, then an error stops the run.
    let sql = format!("SELECT id, name FROM {DATASET}.{TABLE} ORDER BY id");
    let results = run(
        &s,
        &[&sql, "SELECT nope FROM libratable_missing", "SELECT 1"],
    )
    .await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].rows.len(), 3);
    assert_eq!(results[0].columns[0].type_name, "INT64");
    assert!(results[1].error.is_some());

    // Export the whole table.
    let lines = export_csv(&s, table_source(page(DATASET, TABLE, 2, vec![]))).await;
    assert_eq!(lines.len(), 4, "{lines:?}");

    // Read-only refuses writes app-side.
    let ro = conn(endpoint, true);
    let stmts = vec![
        format!("DELETE FROM {DATASET}.{TABLE} WHERE true"),
        sql.clone(),
    ];
    assert_eq!(safe_mode::read_only_violations(&ro, &stmts).len(), 1);
    let ro = try_session(ro, None).await.unwrap();
    let change = update(json!({"id": 1}), json!({"name": "x"}));
    let err = apply(&ro, DATASET, TABLE, &[change]).await.err().unwrap();
    assert_eq!(err.kind, ErrorKind::ReadOnly);
}
