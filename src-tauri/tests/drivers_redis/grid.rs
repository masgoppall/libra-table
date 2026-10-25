//! The key space as a grid: catalog, SCAN paging, filters, counts, edits, Quick Look
//! and export.

use std::collections::HashSet;
use std::sync::Arc;

use libratable_lib::db::cell::CellContent;
use libratable_lib::db::changes::Change;
use libratable_lib::db::data::{self, PageRequest};
use libratable_lib::db::engine::{RowSink, TableRef};
use libratable_lib::db::filter::FilterRow;
use libratable_lib::db::meta;
use libratable_lib::db::value::ColumnMeta;
use libratable_lib::error::AppResult;
use libratable_lib::state::Session;
use serde_json::{json, Value};

use super::keys::STRINGS;
use super::{key, PREFIX};

/// Test keys: the strings, five other types and the two glob-character keys.
const ALL_KEYS: usize = STRINGS + 5 + 2;

fn filter(column: &str, operator: &str, value: &str) -> FilterRow {
    FilterRow {
        enabled: true,
        column: column.into(),
        operator: operator.into(),
        value: value.into(),
        value2: String::new(),
    }
}

fn request(page: u32, size: u32, filters: Vec<FilterRow>) -> PageRequest {
    PageRequest {
        schema: String::new(),
        table: "keys".into(),
        page,
        page_size: size,
        sort: vec![],
        filters,
        apply_only: None,
    }
}

fn keys_of(rows: &[Vec<Value>]) -> Vec<String> {
    rows.iter()
        .map(|r| r[0].as_str().unwrap().to_string())
        .collect()
}

async fn all_pages(s: &Session, size: u32, filters: Vec<FilterRow>) -> Vec<String> {
    let mut out = Vec::new();
    for page in 0.. {
        let r = data::fetch_page(s, &request(page, size, filters.clone()))
            .await
            .unwrap();
        assert!(r.rows.len() <= size as usize);
        out.extend(keys_of(&r.rows));
        if !r.has_more {
            break;
        }
        assert!(page < 100, "paging never ends");
    }
    out
}

fn engine(s: &Session) -> &Arc<dyn libratable_lib::db::engine::Engine> {
    s.pool.engine().expect("engine")
}

async fn commit(s: &Session, changes: Value) -> AppResult<u64> {
    let changes: Vec<Change> = serde_json::from_value(changes).unwrap();
    let cols = s.columns("", "keys").await.unwrap();
    let t = TableRef {
        schema: "",
        table: "keys",
        columns: &cols,
    };
    engine(s).preview_changes(t, &changes)?;
    engine(s).apply_changes(t, &changes).await
}

pub async fn run(s: &Arc<Session>) {
    catalog(s).await;
    paging_and_filters(s).await;
    edits(s).await;
    quick_look_and_export(s).await;
    bulk_paging(s).await;
    // Another database through a child session ("db14").
    let other = s.in_database(Some("db14")).await.unwrap();
    let page = data::fetch_page(
        &other,
        &request(0, 10, vec![filter("key", "has_prefix", PREFIX)]),
    )
    .await
    .unwrap();
    assert!(page.rows.is_empty());
}

async fn catalog(s: &Session) {
    let dbs = meta::list_databases(&s.pool, false).await.unwrap();
    assert!(dbs.contains(&"db15".to_string()), "{dbs:?}");
    let all = meta::list_databases(&s.pool, true).await.unwrap();
    assert!(all.len() >= 16);
    let objects = meta::list_objects(&s.pool, "").await.unwrap();
    assert_eq!(objects[0].name, "keys");
    let cols = s.columns("", "keys").await.unwrap();
    assert_eq!(cols[0].name, "key");
    assert_eq!(cols[0].pk_position, Some(1));
}

async fn paging_and_filters(s: &Session) {
    // Pages of 7 cover every key once.
    let mine = vec![filter("key", "has_prefix", PREFIX)];
    let keys = all_pages(s, 7, mine.clone()).await;
    let unique: HashSet<&String> = keys.iter().collect();
    assert_eq!((keys.len(), unique.len()), (ALL_KEYS, ALL_KEYS), "{keys:?}");
    let unfiltered = all_pages(s, 4, vec![]).await;
    assert_eq!(unfiltered.len(), ALL_KEYS);

    // Page 2 again (cached cursor) gives the same keys.
    let p2 = data::fetch_page(s, &request(2, 7, mine.clone()))
        .await
        .unwrap();
    let again = data::fetch_page(s, &request(2, 7, mine.clone()))
        .await
        .unwrap();
    assert_eq!(keys_of(&p2.rows), keys_of(&again.rows));
    assert!(
        p2.sql.starts_with("SCAN 0 MATCH libratable:test:* COUNT 7"),
        "{}",
        p2.sql
    );

    // Operators, with glob characters taken literally.
    let only = |f: FilterRow| async move { all_pages(s, 50, vec![f]).await };
    assert_eq!(
        only(filter("key", "eq", &key("star*"))).await,
        [key("star*")]
    );
    assert_eq!(only(filter("key", "has_suffix", "*")).await, [key("star*")]);
    assert_eq!(only(filter("key", "contains", ":s:0")).await.len(), 10);
    assert_eq!(only(filter("type", "eq", "hash")).await, [key("hash")]);
    let mut prefixed = only(filter("key", "has_prefix", &key("star"))).await;
    prefixed.sort();
    assert_eq!(prefixed, [key("star*"), key("starX")]);

    // Counts: DBSIZE without a filter, a scan with one.
    let n = data::count_rows(s, &request(0, 10, vec![])).await.unwrap();
    assert_eq!((n.total, n.exact), (ALL_KEYS as i64, true));
    let hashes = vec![filter("type", "eq", "hash")];
    let n = data::count_rows(s, &request(0, 10, hashes.clone()))
        .await
        .unwrap();
    assert_eq!((n.total, n.exact), (1, true));
    let preview = data::preview_filter(s, &request(0, 10, hashes))
        .await
        .unwrap();
    assert_eq!(preview.all, "TYPE hash");

    // Sort applies within the page.
    let mut req = request(0, 100, mine);
    req.sort = vec![serde_json::from_value(json!({ "column": "key", "desc": true })).unwrap()];
    let sorted = keys_of(&data::fetch_page(s, &req).await.unwrap().rows);
    let mut expected = sorted.clone();
    expected.sort_by(|a, b| b.cmp(a));
    assert_eq!(sorted, expected);
}

async fn row_of(s: &Session, name: &str) -> Option<Vec<Value>> {
    let page = data::fetch_page(s, &request(0, 10, vec![filter("key", "eq", name)]))
        .await
        .unwrap();
    page.rows.into_iter().next()
}

async fn edits(s: &Session) {
    let a = key("s:00");
    // TTL, string value and rename in one commit; the rename runs last.
    let n = commit(
        s,
        json!([{ "kind": "update", "pk": { "key": a },
            "values": { "value": "changed", "ttl": 300, "key": key("renamed") } }]),
    )
    .await
    .unwrap();
    assert_eq!(n, 1);
    assert!(row_of(s, &a).await.is_none());
    let row = row_of(s, &key("renamed")).await.unwrap();
    assert_eq!(row[1], json!("string"));
    assert!(row[2].as_i64().unwrap() > 0);
    assert_eq!(row[4], json!("changed"));

    // Insert a string key with a TTL, then delete it.
    commit(
        s,
        json!([{ "kind": "insert", "values": { "key": key("new"), "value": "v" } }]),
    )
    .await
    .unwrap();
    assert_eq!(row_of(s, &key("new")).await.unwrap()[4], json!("v"));

    // One-row rule: deleting a key that doesn't exist fails, and nothing else changes.
    let err = commit(
        s,
        json!([
            { "kind": "delete", "pk": { "key": key("new") } },
            { "kind": "delete", "pk": { "key": key("missing") } },
        ]),
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("no longer exists"), "{}", err.message);
    assert!(row_of(s, &key("new")).await.is_some());
    commit(
        s,
        json!([{ "kind": "delete", "pk": { "key": key("new") } }]),
    )
    .await
    .unwrap();
    assert!(row_of(s, &key("new")).await.is_none());

    // A rename onto an existing key is refused; strings only for `value`.
    let clash = json!([{ "kind": "update", "pk": { "key": key("renamed") },
        "values": { "key": key("hash") } }]);
    assert!(commit(s, clash).await.is_err());
    let not_string = json!([{ "kind": "update", "pk": { "key": key("hash") },
        "values": { "value": "x" } }]);
    let err = commit(s, not_string).await.unwrap_err();
    assert!(err.message.contains("holds a hash"), "{}", err.message);
    commit(
        s,
        json!([{ "kind": "update", "pk": { "key": key("renamed") },
            "values": { "key": a, "ttl": null } }]),
    )
    .await
    .unwrap();
    assert_eq!(row_of(s, &a).await.unwrap()[2], json!(-1));
}

struct Collect(Vec<Vec<Value>>);

impl RowSink for Collect {
    fn columns(&mut self, _columns: &[ColumnMeta]) -> AppResult<()> {
        Ok(())
    }
    fn row(&mut self, values: Vec<Value>) -> AppResult<()> {
        self.0.push(values);
        Ok(())
    }
}

async fn quick_look_and_export(s: &Session) {
    let cols = s.columns("", "keys").await.unwrap();
    let t = TableRef {
        schema: "",
        table: "keys",
        columns: &cols,
    };
    let pk = json!({ "key": key("hash") });
    let cell = engine(s)
        .fetch_cell(t, pk.as_object().unwrap(), "value")
        .await
        .unwrap();
    let CellContent::Text { text } = cell else {
        panic!("not text")
    };
    let doc: Value = serde_json::from_str(&text).unwrap();
    assert!(doc.get("name").is_some(), "{text}");

    let mut sink = Collect(Vec::new());
    let req = request(0, 0, vec![filter("key", "has_prefix", PREFIX)]);
    let n = engine(s)
        .export_table(t, &req, false, &mut sink)
        .await
        .unwrap();
    assert_eq!(n as usize, ALL_KEYS);
    let out = engine(s)
        .export_query(&format!("LRANGE {} 0 -1", key("list")), &mut sink)
        .await
        .unwrap();
    assert_eq!(out, 25);
}

/// Many SCAN batches: 600 keys in pages of 50, then a sparse pattern.
async fn bulk_paging(s: &Session) {
    let args: Vec<String> = (0..600)
        .map(|i| format!("{} {i}", key(&format!("bulk:{i:03}"))))
        .collect();
    let r = super::run(s, &format!("MSET {}", args.join(" "))).await;
    assert!(r[0].error.is_none(), "{:?}", r[0].error);
    let keys = all_pages(s, 50, vec![filter("key", "has_prefix", &key("bulk:"))]).await;
    let unique: HashSet<&String> = keys.iter().collect();
    assert_eq!((keys.len(), unique.len()), (600, 600));
    let sparse = all_pages(s, 50, vec![filter("key", "has_suffix", ":599")]).await;
    assert_eq!(sparse, [key("bulk:599")]);
    let n = data::count_rows(s, &request(0, 10, vec![filter("key", "contains", "bulk:")]))
        .await
        .unwrap();
    assert_eq!((n.total, n.exact), (600, true));
}
