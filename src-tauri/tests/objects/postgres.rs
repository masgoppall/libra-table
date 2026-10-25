//! Postgres: functions (with overloads), procedures, sequences, materialized views,
//! triggers, enum/composite/domain types.

use super::*;

pub(super) const PG_SETUP: &str = "
CREATE TABLE people (id serial PRIMARY KEY, name text);
CREATE FUNCTION add(a integer, b integer) RETURNS integer LANGUAGE sql AS $$ SELECT a + b $$;
CREATE FUNCTION add(a text) RETURNS text LANGUAGE sql AS $$ SELECT a $$;
CREATE PROCEDURE tidy() LANGUAGE sql AS $$ DELETE FROM people WHERE name IS NULL $$;
CREATE SEQUENCE ids INCREMENT BY 5 START WITH 10;
CREATE MATERIALIZED VIEW totals AS SELECT count(*) AS n FROM people;
CREATE FUNCTION touch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;
CREATE TRIGGER audit BEFORE INSERT OR UPDATE ON people
    FOR EACH ROW EXECUTE FUNCTION touch();
CREATE TYPE mood AS ENUM ('sad', 'ok', 'it''s fine');
CREATE TYPE pair AS (x integer, \"Label\" text);
CREATE DOMAIN positive AS integer CHECK (VALUE > 0);
";

pub(super) async fn pg_listing(s: &Session) {
    let list = listed(s, "public").await;
    let add = list
        .iter()
        .filter(|o| o.kind == ObjectKind::Function && o.name == "add")
        .count();
    assert_eq!(add, 2, "both overloads are listed");
    let p = find(&list, ObjectKind::Procedure, "tidy");
    assert_eq!(p.arguments.as_deref(), Some(""));
    find(&list, ObjectKind::Sequence, "ids");
    find(&list, ObjectKind::MaterializedView, "totals");
    let t = find(&list, ObjectKind::Trigger, "audit");
    assert_eq!(t.table.as_deref(), Some("people"));
    assert_eq!(t.detail.as_deref(), Some("BEFORE INSERT OR UPDATE · row"));
    let mood = find(&list, ObjectKind::Type, "mood");
    assert_eq!(mood.labels, vec!["sad", "ok", "it's fine"]);
    assert_eq!(
        find(&list, ObjectKind::Type, "positive").detail.as_deref(),
        Some("domain over integer")
    );
    assert_eq!(
        find(&list, ObjectKind::Type, "pair").detail.as_deref(),
        Some("composite")
    );
    // The table's own row type and the serial's sequence: only the latter is listed.
    assert!(!list
        .iter()
        .any(|o| o.kind == ObjectKind::Type && o.name == "people"));
    find(&list, ObjectKind::Sequence, "people_id_seq");
}

pub(super) async fn pg_definitions(s: &Session) {
    let mut add = object(ObjectKind::Function, "add");
    add.arguments = Some("a integer, b integer".into());
    let def = definition(s, "public", &add).await;
    assert!(def
        .ddl
        .contains("CREATE OR REPLACE FUNCTION public.add(a integer, b integer)"));
    run_script(s, &def.edit_script).await;

    let seq = definition(s, "public", &object(ObjectKind::Sequence, "ids")).await;
    assert!(seq.ddl.contains("INCREMENT BY 5"));
    run_script(s, &seq.edit_script).await;

    let mut trg = object(ObjectKind::Trigger, "audit");
    trg.table = Some("people".into());
    for o in [
        object(ObjectKind::MaterializedView, "totals"),
        trg,
        object(ObjectKind::Type, "mood"),
        object(ObjectKind::Type, "pair"),
        object(ObjectKind::Type, "positive"),
    ] {
        // Drop-and-create scripts must recreate the object as it was.
        let before = definition(s, "public", &o).await;
        run_script(s, &before.edit_script).await;
        assert_eq!(definition(s, "public", &o).await.ddl, before.ddl);
    }
    let mood = definition(s, "public", &object(ObjectKind::Type, "mood")).await;
    assert_eq!(
        mood.ddl,
        r#"CREATE TYPE "public"."mood" AS ENUM ('sad', 'ok', 'it''s fine');"#
    );
}

pub(super) async fn pg_changes(s: &Session) {
    let info = objects::sequence(&s.pool, "public", "ids").await.unwrap();
    assert_eq!(
        (info.increment, info.start_value, info.last_value),
        (5, 10, None)
    );
    assert_eq!(scalar_i64(s, "SELECT nextval('ids')").await, 10);
    let reset =
        json!({ "kind": "reset_sequence", "schema": "public", "name": "ids", "value": 100 });
    change(s, reset).await.unwrap();
    assert_eq!(scalar_i64(s, "SELECT nextval('ids')").await, 100);

    run_script(s, "INSERT INTO people (name) VALUES ('Ann')").await;
    let refresh = json!({
        "kind": "refresh_materialized_view", "schema": "public", "name": "totals"
    });
    change(s, refresh).await.unwrap();
    assert_eq!(scalar_i64(s, "SELECT n FROM totals").await, 1);

    // Two overloads: the arguments pick one.
    let err = change(s, drop("public", "function", "add", json!({}))).await;
    assert!(err.unwrap_err().message.contains("2 overloads"));
    let preview = change(
        s,
        drop(
            "public",
            "function",
            "add",
            json!({ "arguments": "a integer, b integer" }),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        preview,
        vec![r#"DROP FUNCTION "public"."add"(a integer, b integer)"#]
    );
    let drops = [
        drop("public", "procedure", "tidy", json!({})),
        drop("public", "sequence", "ids", json!({})),
        drop("public", "materialized_view", "totals", json!({})),
        drop("public", "trigger", "audit", json!({ "table": "people" })),
        drop("public", "type", "mood", json!({})),
        drop("public", "type", "pair", json!({})),
        drop("public", "type", "positive", json!({})),
    ];
    for d in drops {
        change(s, d.clone())
            .await
            .unwrap_or_else(|e| panic!("{d}: {e:?}"));
    }
    let left: Vec<String> = listed(s, "public")
        .await
        .into_iter()
        .map(|o| o.name)
        .collect();
    assert_eq!(left, vec!["add", "touch", "people_id_seq"]);
}
