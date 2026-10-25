//! Keys of every type, read through the key view (with paging) and edited inside.

use libratable_lib::engines::redis::{KeyData, KeyDetail, KeyEdit, RedisEngine};
use libratable_lib::state::Session;
use serde_json::{json, Value};

use super::{edit, key, redis};

/// Strings for paging, one key of each type, and two keys with glob characters.
pub const STRINGS: usize = 25;

pub async fn create_all(s: &Session) {
    for i in 0..STRINGS {
        let name = key(&format!("s:{i:02}"));
        edit(
            s,
            json!({ "op": "create", "key": name, "type": "string", "value": i.to_string() }),
        )
        .await;
    }
    let items: Vec<String> = (0..25).map(|i| format!("item{i}")).collect();
    let creates = [
        (
            "hash",
            json!([{ "field": "name", "value": "Ada" }, { "field": "age", "value": "36" }]),
        ),
        ("list", json!(items)),
        ("set", json!(["a", "b", "c"])),
        (
            "zset",
            json!([{ "member": "low", "score": 1 }, { "member": "high", "score": 9.5 }]),
        ),
        ("stream", json!([{ "field": "event", "value": "signup" }])),
    ];
    for (t, value) in creates {
        edit(
            s,
            json!({ "op": "create", "key": key(t), "type": t, "value": value }),
        )
        .await;
    }
    for name in ["star*", "starX"] {
        edit(
            s,
            json!({ "op": "create", "key": key(name), "type": "string", "value": "x" }),
        )
        .await;
    }
    // Creating over an existing key is refused.
    let again: KeyEdit = serde_json::from_value(
        json!({ "op": "create", "key": key("hash"), "type": "string", "value": "v" }),
    )
    .unwrap();
    let err = redis(s)
        .run_plan(&RedisEngine::edit_plan(&again).unwrap())
        .await
        .unwrap_err();
    assert!(err.message.contains("already exists"), "{}", err.message);
}

async fn read(s: &Session, name: &str, cursor: &str, count: u32) -> KeyDetail {
    let req = json!({ "key": key(name), "cursor": cursor, "count": count });
    redis(s)
        .read_key(&serde_json::from_value(req).unwrap())
        .await
        .expect("read key")
}

fn hash_entries(d: &KeyDetail) -> Vec<Value> {
    match &d.value {
        KeyData::Hash { entries } => entries.clone(),
        other => panic!("not a hash: {other:?}"),
    }
}

pub async fn run(s: &Session) {
    // Hash: set a field, rename one, delete one.
    let h = read(s, "hash", "", 100).await;
    assert_eq!((h.key_type.as_str(), h.size), ("hash", Some(2)));
    edit(
        s,
        json!({ "op": "hash_set", "key": key("hash"), "field": "name", "value": "Grace" }),
    )
    .await;
    edit(
        s,
        json!({ "op": "hash_set", "key": key("hash"), "field": "years", "value": "36",
            "previous": "age" }),
    )
    .await;
    let mut entries = hash_entries(&read(s, "hash", "", 100).await);
    entries.sort_by_key(|e| e["field"].to_string());
    assert_eq!(
        entries,
        vec![
            json!({ "field": "name", "value": "Grace" }),
            json!({ "field": "years", "value": "36" })
        ]
    );

    list(s).await;
    sets(s).await;

    // Stream (read-only in the panel).
    let st = read(s, "stream", "", 10).await;
    match &st.value {
        KeyData::Stream { entries } => {
            assert_eq!(entries[0]["fields"], json!([["event", "signup"]]))
        }
        other => panic!("not a stream: {other:?}"),
    }

    // TTL, rename, missing keys.
    edit(
        s,
        json!({ "op": "expire", "key": key("set"), "seconds": 100 }),
    )
    .await;
    let ttl = read(s, "set", "", 10).await.ttl;
    assert!((1..=100).contains(&ttl), "{ttl}");
    edit(
        s,
        json!({ "op": "expire", "key": key("set"), "seconds": null }),
    )
    .await;
    assert_eq!(read(s, "set", "", 10).await.ttl, -1);
    edit(
        s,
        json!({ "op": "rename", "key": key("set"), "to": key("set2") }),
    )
    .await;
    assert!(matches!(
        read(s, "set", "", 10).await.value,
        KeyData::Missing
    ));
    edit(
        s,
        json!({ "op": "rename", "key": key("set2"), "to": key("set") }),
    )
    .await;
}

async fn list(s: &Session) {
    // Paging by index.
    let first = read(s, "list", "", 10).await;
    assert_eq!(first.next, "10");
    let third = read(s, "list", "20", 10).await;
    let KeyData::List { items } = &third.value else {
        panic!("not a list")
    };
    assert_eq!((items.len(), third.next.as_str()), (5, ""));
    assert_eq!(items[0], json!({ "index": 20, "value": "item20" }));
    // Set, remove (checked against the expected value), push.
    edit(
        s,
        json!({ "op": "list_set", "key": key("list"), "index": 0, "value": "first",
            "expected": "item0" }),
    )
    .await;
    edit(
        s,
        json!({ "op": "list_remove", "key": key("list"), "index": 1, "expected": "item1" }),
    )
    .await;
    edit(
        s,
        json!({ "op": "list_push", "key": key("list"), "values": ["head"], "head": true }),
    )
    .await;
    let l = read(s, "list", "", 3).await;
    let KeyData::List { items } = &l.value else {
        panic!("not a list")
    };
    let values: Vec<&Value> = items.iter().map(|i| &i["value"]).collect();
    assert_eq!(values, [&json!("head"), &json!("first"), &json!("item2")]);
    assert_eq!(l.size, Some(25));
    // A stale expected value is refused and changes nothing.
    let stale: KeyEdit = serde_json::from_value(
        json!({ "op": "list_remove", "key": key("list"), "index": 0, "expected": "item0" }),
    )
    .unwrap();
    let err = redis(s)
        .run_plan(&RedisEngine::edit_plan(&stale).unwrap())
        .await
        .unwrap_err();
    assert!(err.message.contains("changed"), "{}", err.message);
    assert_eq!(read(s, "list", "", 1).await.size, Some(25));
}

async fn sets(s: &Session) {
    edit(
        s,
        json!({ "op": "set_add", "key": key("set"), "members": ["d"] }),
    )
    .await;
    edit(
        s,
        json!({ "op": "set_remove", "key": key("set"), "members": ["a"] }),
    )
    .await;
    let KeyData::Set { mut members } = read(s, "set", "", 100).await.value else {
        panic!("not a set")
    };
    members.sort_by_key(|m| m.to_string());
    assert_eq!(members, [json!("b"), json!("c"), json!("d")]);

    edit(
        s,
        json!({ "op": "zset_add", "key": key("zset"), "member": "mid", "score": 5 }),
    )
    .await;
    edit(
        s,
        json!({ "op": "zset_remove", "key": key("zset"), "members": ["high"] }),
    )
    .await;
    let KeyData::Zset { members } = read(s, "zset", "", 100).await.value else {
        panic!("not a zset")
    };
    assert_eq!(
        members,
        [
            json!({ "member": "low", "score": 1.0 }),
            json!({ "member": "mid", "score": 5.0 })
        ]
    );
    // Type checks: a set edit on a hash is refused.
    let wrong: KeyEdit =
        serde_json::from_value(json!({ "op": "set_add", "key": key("hash"), "members": ["x"] }))
            .unwrap();
    let err = redis(s)
        .run_plan(&RedisEngine::edit_plan(&wrong).unwrap())
        .await
        .unwrap_err();
    assert!(err.message.contains("holds a hash"), "{}", err.message);
}
