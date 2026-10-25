//! Schema changes end to end through `db::ddl`: create tables with foreign keys and
//! indexes, alter columns (an SQLite rebuild with a dependent view), foreign keys,
//! primary keys, rename / duplicate / truncate / drop.

use libratable_lib::db::meta::{self, ColumnInfo, TableStructure};
use libratable_lib::models::Driver;
use libratable_lib::state::Session;
use serde_json::{json, Value};

use crate::common::{count, ddl_apply, exec_sql, object_names, spec};

/// Quoted `schema.table` for raw SQL in the tests.
struct Names<'a> {
    driver: Driver,
    schema: &'a str,
}

impl Names<'_> {
    fn table(&self, name: &str) -> String {
        match self.driver {
            Driver::Mysql => format!("`{}`.`{name}`", self.schema),
            _ => format!("\"{}\".\"{name}\"", self.schema),
        }
    }
}

fn int_type(driver: Driver) -> &'static str {
    if driver == Driver::Sqlite {
        "INTEGER"
    } else {
        "integer"
    }
}

fn column<'a>(s: &'a TableStructure, name: &str) -> &'a ColumnInfo {
    s.columns.iter().find(|c| c.name == name).unwrap()
}

fn has_org_index(s: &TableStructure) -> bool {
    s.indexes
        .iter()
        .any(|i| i.columns == vec!["org_id".to_string()] && !i.primary)
}

async fn structure(session: &Session, schema: &str, table: &str) -> TableStructure {
    meta::table_structure(&session.pool, schema, table)
        .await
        .unwrap()
}

pub async fn run(session: &Session, schema: &str) {
    let names = Names {
        driver: session.driver(),
        schema,
    };
    create_tables(session, schema, &names).await;
    alter_members(session, schema).await;
    foreign_keys(session, schema, &names).await;
    table_operations(session, schema).await;
}

async fn create_tables(session: &Session, schema: &str, names: &Names<'_>) {
    let driver = session.driver();
    let int = int_type(driver);
    let id = json!({
        "name": "id", "dataType": int, "nullable": false,
        "primaryKey": true, "autoIncrement": true
    });
    ddl_apply(
        session,
        json!({ "kind": "create_table", "schema": schema, "name": "orgs", "columns": [
            id,
            { "name": "name", "dataType": "varchar(100)", "nullable": false, "unique": true },
        ]}),
    )
    .await
    .expect("create orgs");
    let comment = if driver == Driver::Sqlite {
        Value::Null
    } else {
        json!("People in orgs")
    };
    ddl_apply(
        session,
        json!({ "kind": "create_table", "schema": schema, "name": "members",
            "comment": comment,
            "columns": [
                id,
                { "name": "org_id", "dataType": int, "nullable": false },
                { "name": "email", "dataType": "varchar(200)", "nullable": true, "unique": true },
                { "name": "score", "dataType": int, "nullable": true, "default": "0" },
                { "name": "note", "dataType": "varchar(200)", "nullable": true },
            ],
            "indexes": [{ "columns": ["org_id"] }],
            "foreignKeys": [{
                "name": "members_org_fk", "columns": ["org_id"], "refTable": "orgs",
                "refColumns": ["id"], "onDelete": "CASCADE"
            }],
        }),
    )
    .await
    .expect("create members");
    let seed = format!(
        "INSERT INTO {} (name) VALUES ('acme'), ('globex');
         INSERT INTO {} (org_id, email, score, note) VALUES
             (1, 'a@x.io', 10, 'first'), (1, 'b@x.io', 20, NULL), (2, NULL, 30, 'third');",
        names.table("orgs"),
        names.table("members")
    );
    exec_sql(&session.pool, &seed).await;
    if driver != Driver::Mysql {
        let view = format!(
            "CREATE VIEW {} AS SELECT id, org_id FROM {}",
            names.table("member_orgs"),
            names.table("members")
        );
        exec_sql(&session.pool, &view).await;
    }

    let s = structure(session, schema, "members").await;
    assert_eq!(s.foreign_keys.len(), 1, "{:?}", s.foreign_keys);
    assert_eq!(s.foreign_keys[0].on_delete, "CASCADE");
    assert!(has_org_index(&s), "{:?}", s.indexes);
    let id = column(&s, "id");
    assert!(id.auto_increment, "{id:?}");
    let score = column(&s, "score");
    let default = score
        .default
        .as_deref()
        .map(|d| d.trim_matches(|c| c == '\'' || c == '(' || c == ')'));
    assert_eq!(default, Some("0"), "{score:?}");
}

/// Change a type, nullability and default, rename, add and drop columns in one go.
/// On SQLite this rebuilds the table (with the view and the FK depending on it).
async fn alter_members(session: &Session, schema: &str) {
    let driver = session.driver();
    let s = structure(session, schema, "members").await;
    let (score, note) = (column(&s, "score"), column(&s, "note"));
    let mut score_new = spec(score);
    score_new["dataType"] = json!("bigint");
    score_new["nullable"] = json!(false);
    score_new["default"] = json!("5");
    let mut note_new = spec(note);
    note_new["name"] = json!("remark");
    let joined = json!({
        "name": "joined", "dataType": "varchar(30)", "nullable": true, "default": "'today'"
    });
    let preview = ddl_apply(
        session,
        json!({ "kind": "alter_table", "schema": schema, "name": "members", "ops": [
            { "op": "modify_column", "original": spec(score), "column": score_new },
            { "op": "modify_column", "original": spec(note), "column": note_new },
            { "op": "add_column", "column": joined },
            { "op": "drop_column", "name": "email" },
            { "op": "add_index", "index": { "name": "members_remark_idx", "columns": ["remark"] } },
        ]}),
    )
    .await
    .unwrap_or_else(|e| panic!("alter members: {e:?}"));
    if driver == Driver::Sqlite {
        assert_eq!(preview[0], "PRAGMA foreign_keys = OFF", "{preview:#?}");
        let rebuild = "CREATE TABLE \"main\".\"_libratable_new_members\"";
        assert!(
            preview.iter().any(|p| p.starts_with(rebuild)),
            "{preview:#?}"
        );
    }
    let s = structure(session, schema, "members").await;
    let names: Vec<&str> = s.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["id", "org_id", "score", "remark", "joined"],
        "{:?}",
        s.columns
    );
    let score = column(&s, "score");
    assert!(
        score.data_type.to_lowercase().starts_with("bigint"),
        "{score:?}"
    );
    assert!(!score.nullable);
    assert!(
        column(&s, "id").auto_increment,
        "the rebuild keeps AUTOINCREMENT"
    );
    assert_eq!(
        s.foreign_keys.len(),
        1,
        "foreign key survives: {:?}",
        s.foreign_keys
    );
    assert!(
        s.indexes.iter().any(|i| i.name == "members_remark_idx"),
        "{:?}",
        s.indexes
    );
    assert!(has_org_index(&s), "index survives: {:?}", s.indexes);
    assert_eq!(count(session, schema, "members").await, 3, "rows survive");
    if driver != Driver::Mysql {
        let n = count(session, schema, "member_orgs").await;
        assert_eq!(n, 3, "the view still works");
    }
}

/// Drop the foreign key; re-adding it with a violating row fails and changes nothing.
async fn foreign_keys(session: &Session, schema: &str, names: &Names<'_>) {
    let fk_name = structure(session, schema, "members").await.foreign_keys[0]
        .name
        .clone();
    let drop = json!({ "kind": "alter_table", "schema": schema, "name": "members",
        "ops": [{ "op": "drop_foreign_key", "name": fk_name }] });
    ddl_apply(session, drop).await.expect("drop fk");
    let fks = |s: TableStructure| s.foreign_keys.len();
    assert_eq!(fks(structure(session, schema, "members").await), 0);
    let orphan = format!(
        "INSERT INTO {} (org_id, score) VALUES (999, 1)",
        names.table("members")
    );
    exec_sql(&session.pool, &orphan).await;
    let fk = json!({ "kind": "alter_table", "schema": schema, "name": "members", "ops": [{
        "op": "add_foreign_key",
        "foreignKey": { "columns": ["org_id"], "refTable": "orgs", "refColumns": ["id"] }
    }] });
    assert!(
        ddl_apply(session, fk.clone()).await.is_err(),
        "row 999 has no org"
    );
    assert_eq!(fks(structure(session, schema, "members").await), 0);
    assert_eq!(count(session, schema, "members").await, 4);
    let cleanup = format!("DELETE FROM {} WHERE org_id = 999", names.table("members"));
    exec_sql(&session.pool, &cleanup).await;
    ddl_apply(session, fk).await.expect("add fk");
    assert_eq!(fks(structure(session, schema, "members").await), 1);
}

/// Primary key on a table without one, then rename / duplicate / truncate / drop.
async fn table_operations(session: &Session, schema: &str) {
    ddl_apply(
        session,
        json!({ "kind": "create_table", "schema": schema, "name": "tags", "columns": [
            { "name": "code", "dataType": "varchar(20)", "nullable": false },
            { "name": "lang", "dataType": "varchar(5)", "nullable": false },
        ] }),
    )
    .await
    .expect("create tags");
    let set_pk = json!({ "kind": "alter_table", "schema": schema, "name": "tags",
        "ops": [{ "op": "set_primary_key", "columns": ["code", "lang"] }] });
    ddl_apply(session, set_pk).await.expect("set pk");
    let cols = meta::list_columns(&session.pool, schema, "tags")
        .await
        .unwrap();
    assert_eq!(meta::primary_key(&cols), vec!["code", "lang"]);
    ddl_apply(
        session,
        json!({ "kind": "rename_table", "schema": schema, "from": "tags", "to": "labels" }),
    )
    .await
    .expect("rename");
    let duplicate = json!({ "kind": "duplicate_table", "schema": schema,
        "from": "members", "to": "members_copy", "withData": true });
    ddl_apply(session, duplicate).await.expect("duplicate");
    assert_eq!(count(session, schema, "members_copy").await, 3);
    ddl_apply(
        session,
        json!({ "kind": "truncate_table", "schema": schema, "name": "members_copy" }),
    )
    .await
    .expect("truncate");
    assert_eq!(count(session, schema, "members_copy").await, 0);
    for name in ["members_copy", "labels"] {
        ddl_apply(
            session,
            json!({ "kind": "drop_table", "schema": schema, "name": name }),
        )
        .await
        .unwrap_or_else(|e| panic!("drop {name}: {e:?}"));
    }
    if session.driver() != Driver::Mysql {
        let p = ddl_apply(
            session,
            json!({ "kind": "drop_table", "schema": schema, "name": "member_orgs" }),
        )
        .await
        .expect("drop view");
        assert!(p[0].starts_with("DROP VIEW"), "{p:?}");
    }
    let left = object_names(session, schema).await;
    let dropped = ["labels", "members_copy", "member_orgs"];
    assert!(
        !left.iter().any(|n| dropped.contains(&n.as_str())),
        "{left:?}"
    );
}
