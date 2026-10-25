//! A second database opened next to the first one (SCH-1, multi-database tabs),
//! created and dropped through `db::ddl`.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use libratable_lib::db::{exec, meta};
use libratable_lib::models::Driver;
use libratable_lib::state::Session;
use serde_json::json;

use crate::common::{ddl_apply, object_names, OTHER_DB};

pub async fn run(primary: Arc<Session>, schema_of: impl Fn(&str) -> String) {
    ddl_apply(
        &primary,
        json!({ "kind": "create_database", "name": OTHER_DB }),
    )
    .await
    .expect("create database");
    let dbs = meta::list_databases(&primary.pool, false).await.unwrap();
    assert!(dbs.iter().any(|d| d == OTHER_DB), "{dbs:?}");

    let other = primary
        .in_database(Some(OTHER_DB))
        .await
        .expect("open other database");
    assert_eq!(other.database, OTHER_DB);
    assert!(
        Arc::ptr_eq(&other, &primary.in_database(Some(OTHER_DB)).await.unwrap()),
        "the session is reused"
    );
    let own = primary.in_database(Some(&primary.database)).await.unwrap();
    assert!(Arc::ptr_eq(&primary, &own));

    let schema = schema_of(OTHER_DB);
    let table = json!({ "kind": "create_table", "schema": schema, "name": "only_here",
        "columns": [{ "name": "id", "dataType": "integer", "nullable": false, "primaryKey": true }]
    });
    ddl_apply(&other, table)
        .await
        .expect("create table in the other database");
    assert_eq!(object_names(&other, &schema).await, vec!["only_here"]);
    assert_eq!(
        current_database(&other).await,
        json!(OTHER_DB),
        "the editor runs in the other database"
    );

    // The open session to it would block DROP DATABASE on Postgres.
    primary.close_database(OTHER_DB).await;
    ddl_apply(
        &primary,
        json!({ "kind": "drop_database", "name": OTHER_DB }),
    )
    .await
    .expect("drop database");
    let dbs = meta::list_databases(&primary.pool, false).await.unwrap();
    assert!(!dbs.iter().any(|d| d == OTHER_DB));
    primary.close().await;
}

/// The database the session's editor connection is in.
async fn current_database(s: &Session) -> serde_json::Value {
    let mut slot = s.editor.lock().await;
    exec::prepare(s, &mut slot).await.unwrap();
    let sql = if s.driver() == Driver::Postgres {
        "SELECT current_database()::text"
    } else {
        "SELECT DATABASE()"
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let r = exec::run_locked(&mut slot, &[sql.to_string()], 10, &cancelled).await;
    r[0].rows[0][0].clone()
}
