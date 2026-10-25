//! DynamoDB against DynamoDB Local: catalog from DescribeTable + a sample scan, paged
//! scans and key queries, conditional edits and transactions, PartiQL, export,
//! read-only. (DynamoDB Local doesn't check signatures; the SigV4 unit tests use the
//! official test vectors.)

use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use libratable_lib::db::changes::Change;
use libratable_lib::db::{data, meta};
use libratable_lib::error::ErrorKind;
use libratable_lib::safe_mode;
use serde_json::{json, Value};

use crate::common::*;

const TABLE: &str = "libratable_people";
const REGION: &str = "us-east-1";

/// An unsigned-but-well-formed call for setup and cleanup (DynamoDB Local accepts it).
async fn raw(endpoint: &str, action: &str, body: Value) -> Value {
    let auth = "AWS4-HMAC-SHA256 Credential=libratabletest/20240101/us-east-1/dynamodb/\
                aws4_request, SignedHeaders=host, Signature=00";
    let resp = reqwest::Client::new()
        .post(endpoint)
        .header("Content-Type", "application/x-amz-json-1.0")
        .header("X-Amz-Target", format!("DynamoDB_20120810.{action}"))
        .header("Authorization", auth)
        .body(body.to_string())
        .send()
        .await
        .unwrap();
    serde_json::from_str(&resp.text().await.unwrap()).unwrap_or(Value::Null)
}

fn conn(endpoint: &str, key_id: &str, read_only: bool) -> libratable_lib::models::Connection {
    connection(
        "dynamodb",
        json!({ "passwordMode": "keychain", "extra": {
            "region": REGION, "accessKeyId": key_id, "endpoint": endpoint } }),
        read_only,
    )
}

fn item(pk: &str, sk: u32, name: &str, age: Value) -> Value {
    json!({ "PutRequest": { "Item": {
        "pk": {"S": pk}, "sk": {"N": sk.to_string()}, "name": {"S": name}, "age": age } } })
}

#[tokio::test]
async fn dynamodb_against_local() {
    let Ok(endpoint) = std::env::var("LIBRATABLE_TEST_DYNAMODB_ENDPOINT") else {
        eprintln!("skipped: set LIBRATABLE_TEST_DYNAMODB_ENDPOINT to run");
        return;
    };
    raw(&endpoint, "DeleteTable", json!({ "TableName": TABLE })).await;
    let created = raw(
        &endpoint,
        "CreateTable",
        json!({
            "TableName": TABLE, "BillingMode": "PAY_PER_REQUEST",
            "AttributeDefinitions": [
                {"AttributeName": "pk", "AttributeType": "S"},
                {"AttributeName": "sk", "AttributeType": "N"},
                {"AttributeName": "name", "AttributeType": "S"}],
            "KeySchema": [
                {"AttributeName": "pk", "KeyType": "HASH"},
                {"AttributeName": "sk", "KeyType": "RANGE"}],
            "GlobalSecondaryIndexes": [{
                "IndexName": "by_name",
                "KeySchema": [{"AttributeName": "name", "KeyType": "HASH"}],
                "Projection": {"ProjectionType": "ALL"}}]
        }),
    )
    .await;
    assert!(created.get("TableDescription").is_some(), "{created}");
    let mut items = vec![
        item("u1", 1, "Ann", json!({"N": "30"})),
        item("u1", 2, "Bob", json!({"N": "41"})),
        item("u2", 1, "Cy", json!({"N": "25"})),
        item("u3", 1, "Di", json!({"S": "unknown"})),
        item("u4", 1, "Ed", json!({"N": "50"})),
    ];
    items[0]["PutRequest"]["Item"]["tags"] = json!({"SS": ["a", "b"]});
    items[0]["PutRequest"]["Item"]["doc"] = json!({"M": {"x": {"N": "1"}}});
    raw(
        &endpoint,
        "BatchWriteItem",
        json!({ "RequestItems": { TABLE: items } }),
    )
    .await;

    let outcome = AssertUnwindSafe(scenario(&endpoint)).catch_unwind().await;
    raw(&endpoint, "DeleteTable", json!({ "TableName": TABLE })).await;
    if let Err(p) = outcome {
        std::panic::resume_unwind(p);
    }
}

fn names(rows: &[Vec<Value>], idx: usize) -> Vec<String> {
    rows.iter()
        .map(|r| r[idx].as_str().unwrap_or("").to_string())
        .collect()
}

async fn scenario(endpoint: &str) {
    let bad = try_session(conn(endpoint, "bad key!", false), Some("x"))
        .await
        .err()
        .unwrap();
    assert_eq!(bad.kind, ErrorKind::DbAuth);

    let s = try_session(conn(endpoint, "libratabletest", false), Some("secret"))
        .await
        .unwrap();
    assert_eq!(
        meta::list_schemas(&s.pool, false).await.unwrap(),
        vec![REGION]
    );
    let objects = meta::list_objects(&s.pool, REGION).await.unwrap();
    assert!(objects.iter().any(|o| o.name == TABLE));
    let cols = s.columns(REGION, TABLE).await.unwrap();
    let col_names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(col_names, ["pk", "sk", "age", "doc", "name", "tags"]);
    assert_eq!(
        (cols[0].pk_position, cols[1].pk_position),
        (Some(1), Some(2))
    );
    assert_eq!(cols[1].data_type, "N");
    assert_eq!(cols[2].data_type, "mixed");
    let st = meta::table_structure(&s.pool, REGION, TABLE).await.unwrap();
    assert!(st
        .indexes
        .iter()
        .any(|i| i.name == "by_name" && i.definition.starts_with("GSI")));

    let name = 4;
    // Paging: page N+1 continues from the cached key of page N.
    let mut req = page(REGION, TABLE, 2, vec![]);
    let mut seen = Vec::new();
    for p in 0..3 {
        req.page = p;
        let rows = data::fetch_page(&s, &req).await.unwrap().rows;
        seen.extend(names(&rows, name));
    }
    seen.sort();
    assert_eq!(seen, ["Ann", "Bob", "Cy", "Di", "Ed"]);
    req.page = 3;
    assert!(data::fetch_page(&s, &req).await.unwrap().rows.is_empty());

    // `=` on the partition key queries; other rows filter; sort within the page.
    let req = sorted(
        page(
            REGION,
            TABLE,
            10,
            vec![filter("pk", "eq", "u1"), filter("age", "gt", "35")],
        ),
        "sk",
        true,
    );
    let res = data::fetch_page(&s, &req).await.unwrap();
    assert!(res.sql.starts_with("Query "), "{}", res.sql);
    assert_eq!(names(&res.rows, name), ["Bob"]);
    let both = sorted(
        page(REGION, TABLE, 10, vec![filter("pk", "eq", "u1")]),
        "sk",
        true,
    );
    let rows = data::fetch_page(&s, &both).await.unwrap().rows;
    assert_eq!(names(&rows, name), ["Bob", "Ann"]);
    assert_eq!(rows[1][5], json!(["a", "b"]));
    assert_eq!(rows[1][3], json!({"x": 1}));
    let prefix = page(REGION, TABLE, 10, vec![filter("name", "has_prefix", "D")]);
    assert_eq!(
        names(&data::fetch_page(&s, &prefix).await.unwrap().rows, name),
        ["Di"]
    );
    let count = data::count_rows(
        &s,
        &page(REGION, TABLE, 10, vec![filter("age", "gte", "30")]),
    )
    .await
    .unwrap();
    assert_eq!((count.total, count.exact), (3, true));
    assert_eq!(
        data::count_rows(&s, &page(REGION, TABLE, 10, vec![]))
            .await
            .unwrap()
            .total,
        5
    );
    let preview = data::preview_filter(&s, &prefix).await.unwrap();
    assert!(
        preview.all.contains("begins_with(#n0, :v0)"),
        "{}",
        preview.all
    );

    edits(&s).await;

    // PartiQL: rows, then an error stops the run.
    let select = format!("SELECT * FROM \"{TABLE}\" WHERE pk = 'u2'");
    let results = run(
        &s,
        &[&select, "SELECT * FROM \"libratable_missing\"", "SELECT 1"],
    )
    .await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].rows.len(), 1);
    assert!(results[1].error.is_some());

    let lines = export_csv(&s, table_source(page(REGION, TABLE, 2, vec![]))).await;
    assert_eq!(lines.len(), 6, "{lines:?}");

    let ro = conn(endpoint, "libratabletest", true);
    let stmts = vec![
        format!("DELETE FROM \"{TABLE}\" WHERE pk = 'u1' AND sk = 1"),
        select,
    ];
    assert_eq!(safe_mode::read_only_violations(&ro, &stmts).len(), 1);
    let ro = try_session(ro, Some("secret")).await.unwrap();
    let change = update(json!({"pk": "u1", "sk": 1}), json!({"name": "x"}));
    let err = apply(&ro, REGION, TABLE, &[change]).await.err().unwrap();
    assert_eq!(err.kind, ErrorKind::ReadOnly);
}

async fn edits(s: &libratable_lib::state::Session) {
    let ann = || json!({"pk": "u1", "sk": 1});
    let ok = update(ann(), json!({"name": "Anna", "age": 31, "doc": null}));
    assert_eq!(apply(s, REGION, TABLE, &[ok]).await.unwrap(), 1);
    let cols = s.columns(REGION, TABLE).await.unwrap();
    let cell = engine(s)
        .fetch_cell(table_ref(REGION, TABLE, &cols), &map(ann()), "age")
        .await
        .unwrap();
    assert_eq!(
        cell,
        libratable_lib::db::cell::CellContent::Text { text: "31".into() }
    );

    // One-row rule: a missing item fails, and with it the whole transaction.
    let ghost = update(json!({"pk": "nobody", "sk": 9}), json!({"name": "Ghost"}));
    let err = apply(s, REGION, TABLE, std::slice::from_ref(&ghost))
        .await
        .err()
        .unwrap();
    assert!(err.message.contains("affected 0 rows"), "{}", err.message);
    let fine = update(ann(), json!({"name": "Annie"}));
    let err = apply(s, REGION, TABLE, &[fine, ghost]).await.err().unwrap();
    assert!(
        err.message.starts_with("Change 2 affected 0 rows"),
        "{}",
        err.message
    );
    let anna = page(
        REGION,
        TABLE,
        10,
        vec![filter("pk", "eq", "u1"), filter("sk", "eq", "1")],
    );
    let rows = data::fetch_page(s, &anna).await.unwrap().rows;
    assert_eq!(rows[0][4], json!("Anna"));
    assert_eq!(rows[0][2], json!(31));

    // Insert (never over an existing key) and delete in one transaction.
    let insert = Change::Insert {
        values: map(json!({"pk": "u9", "sk": "1", "name": "New"})),
    };
    let dup = Change::Insert {
        values: map(json!({"pk": "u1", "sk": "1", "name": "Dup"})),
    };
    let err = apply(s, REGION, TABLE, &[dup]).await.err().unwrap();
    assert!(err.message.contains("already exists"), "{}", err.message);
    let delete = Change::Delete {
        pk: map(json!({"pk": "u4", "sk": 1})),
    };
    assert_eq!(apply(s, REGION, TABLE, &[insert, delete]).await.unwrap(), 2);
    let all = data::fetch_page(s, &page(REGION, TABLE, 10, vec![]))
        .await
        .unwrap()
        .rows;
    let mut got = names(&all, 4);
    got.sort();
    assert_eq!(got, ["Anna", "Bob", "Cy", "Di", "New"]);
}
