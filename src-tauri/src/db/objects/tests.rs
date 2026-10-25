//! Catalog rows → sidebar objects and DDL, for all three drivers.

use std::collections::HashMap;

use super::*;

fn seq() -> SequenceInfo {
    SequenceInfo {
        data_type: "bigint".into(),
        start_value: 1,
        min_value: 1,
        max_value: 9_223_372_036_854_775_807,
        increment: 5,
        cycle: false,
        cache: 1,
        last_value: Some(11),
    }
}

#[test]
fn postgres_trigger_types() {
    // ROW | BEFORE | INSERT | UPDATE
    assert_eq!(
        postgres::trigger_detail(1 | 2 | 4 | 16),
        "BEFORE INSERT OR UPDATE · row"
    );
    // Statement-level AFTER TRUNCATE; INSTEAD OF DELETE on a view.
    assert_eq!(postgres::trigger_detail(32), "AFTER TRUNCATE · statement");
    assert_eq!(
        postgres::trigger_detail(1 | 8 | 64),
        "INSTEAD OF DELETE · row"
    );
}

#[test]
fn postgres_rows_map_to_objects() {
    let f = postgres::function_object((
        "add".into(),
        "f".into(),
        "a integer, b integer".into(),
        Some("integer".into()),
    ));
    assert_eq!(f.kind, ObjectKind::Function);
    assert_eq!(f.arguments.as_deref(), Some("a integer, b integer"));
    assert_eq!(f.detail.as_deref(), Some("returns integer"));
    let p = postgres::function_object(("tidy".into(), "p".into(), String::new(), None));
    assert_eq!((p.kind, p.detail), (ObjectKind::Procedure, None));

    let m = postgres::relation_object(("totals".into(), "m".into(), false));
    assert_eq!(m.kind, ObjectKind::MaterializedView);
    assert_eq!(m.detail.as_deref(), Some("not populated"));
    assert_eq!(
        postgres::relation_object(("ids".into(), "S".into(), true)).kind,
        ObjectKind::Sequence
    );

    let t = postgres::trigger_object(("audit".into(), "people".into(), 1 | 16, "D".into()));
    assert_eq!(t.table.as_deref(), Some("people"));
    assert_eq!(t.detail.as_deref(), Some("AFTER UPDATE · row (disabled)"));

    let e = postgres::type_object((
        "mood".into(),
        "e".into(),
        vec!["sad".into(), "ok".into()],
        None,
    ));
    assert_eq!((e.detail.as_deref(), e.labels.len()), (Some("enum"), 2));
    let d = postgres::type_detail("d", Some("integer"));
    assert_eq!(d, "domain over integer");
}

#[test]
fn serializes_for_the_ui() {
    let o = postgres::relation_object(("totals".into(), "m".into(), true));
    let v = serde_json::to_value(&o).unwrap();
    assert_eq!(v["kind"], "materialized_view");
    assert_eq!(v["labels"], serde_json::json!([]));
    let r: ObjectRef =
        serde_json::from_value(serde_json::json!({"kind": "function", "name": "f"})).unwrap();
    assert_eq!((r.kind, r.arguments), (ObjectKind::Function, None));
}

#[test]
fn postgres_type_ddl() {
    let t = r#""app"."mood""#;
    let row = |typtype: &str| -> postgres::TypeRow {
        (
            typtype.into(),
            Some("integer".into()),
            true,
            Some("0".into()),
            vec!["sad".into(), "it's ok".into()],
            Some("numeric".into()),
            vec!["x integer".into(), "\"Label\" text".into()],
            vec!["CONSTRAINT positive CHECK (VALUE >= 0)".into()],
        )
    };
    let e = postgres::type_ddl(t, &row("e"));
    assert_eq!(
        e.ddl,
        r#"CREATE TYPE "app"."mood" AS ENUM ('sad', 'it''s ok');"#
    );
    assert!(e
        .edit_script
        .contains("DROP TYPE \"app\".\"mood\";\nCREATE TYPE"));
    let c = postgres::type_ddl(t, &row("c"));
    assert_eq!(
        c.ddl,
        "CREATE TYPE \"app\".\"mood\" AS (\n    x integer,\n    \"Label\" text\n);"
    );
    let d = postgres::type_ddl(t, &row("d"));
    assert_eq!(
        d.ddl,
        "CREATE DOMAIN \"app\".\"mood\" AS integer DEFAULT 0 NOT NULL\n    \
         CONSTRAINT positive CHECK (VALUE >= 0);"
    );
    assert!(d.edit_script.contains("DROP DOMAIN"));
    let r = postgres::type_ddl(t, &row("r"));
    assert_eq!(
        r.ddl,
        "CREATE TYPE \"app\".\"mood\" AS RANGE (SUBTYPE = numeric);"
    );
}

#[test]
fn postgres_sequence_ddl() {
    let create = postgres::sequence_ddl("CREATE", "\"s\"", &seq());
    assert!(create.starts_with("CREATE SEQUENCE \"s\" AS bigint\n    INCREMENT BY 5"));
    assert!(create.ends_with("CACHE 1\n    NO CYCLE;"));
    let alter = postgres::sequence_ddl("ALTER", "\"s\"", &seq());
    assert!(alter.starts_with("ALTER SEQUENCE \"s\"\n    INCREMENT BY 5"));
}

#[test]
fn mysql_rows_map_to_objects() {
    let params = vec![
        (
            "move".into(),
            "PROCEDURE".into(),
            Some("IN".into()),
            Some("a".into()),
            "int".into(),
        ),
        (
            "move".into(),
            "PROCEDURE".into(),
            Some("OUT".into()),
            Some("b".into()),
            "varchar(10)".into(),
        ),
        (
            "twice".into(),
            "FUNCTION".into(),
            Some("IN".into()),
            Some("x".into()),
            "int".into(),
        ),
    ];
    let sig: HashMap<(String, String), String> = mysql::signatures(params);
    let p = mysql::routine_object(("move".into(), "PROCEDURE".into(), None), &sig);
    assert_eq!(p.kind, ObjectKind::Procedure);
    assert_eq!(p.arguments.as_deref(), Some("IN a int, OUT b varchar(10)"));
    let f = mysql::routine_object(
        ("twice".into(), "FUNCTION".into(), Some("int".into())),
        &sig,
    );
    assert_eq!(f.arguments.as_deref(), Some("x int"));
    assert_eq!(f.detail.as_deref(), Some("returns int"));

    let t = mysql::trigger_object((
        "bi".into(),
        "people".into(),
        "BEFORE".into(),
        "INSERT".into(),
    ));
    assert_eq!(t.detail.as_deref(), Some("BEFORE INSERT · row"));
    let e = mysql::event_object((
        "nightly".into(),
        "RECURRING".into(),
        Some("1".into()),
        Some("DAY".into()),
        "ENABLED".into(),
    ));
    assert_eq!(e.detail.as_deref(), Some("every 1 day · enabled"));
    let once = mysql::event_object((
        "once".into(),
        "ONE TIME".into(),
        None,
        None,
        "DISABLED".into(),
    ));
    assert_eq!(once.detail.as_deref(), Some("one time · disabled"));
}

#[test]
fn sqlite_trigger_details() {
    let t = sqlite::trigger_object((
        "trg".into(),
        "people".into(),
        Some("CREATE TRIGGER trg BEFORE UPDATE OF name ON people BEGIN SELECT 1; END".into()),
    ));
    assert_eq!(t.detail.as_deref(), Some("BEFORE UPDATE · row"));
    assert_eq!(
        sqlite::trigger_detail("create trigger t insert on x begin select 1; end"),
        "AFTER INSERT · row"
    );
    assert_eq!(
        sqlite::trigger_detail("CREATE TRIGGER t INSTEAD OF DELETE ON v BEGIN SELECT 1; END"),
        "INSTEAD OF DELETE · row"
    );
}

#[test]
fn statements_end_with_one_semicolon() {
    assert_eq!(terminated("CREATE x\n"), "CREATE x;");
    assert_eq!(terminated("CREATE x;  "), "CREATE x;");
}

#[test]
fn sequence_numbers_go_out_as_text() {
    let v = serde_json::to_value(seq()).unwrap();
    assert_eq!(v["maxValue"], "9223372036854775807");
    assert_eq!(v["lastValue"], "11");
    let unused = SequenceInfo {
        last_value: None,
        ..seq()
    };
    assert!(serde_json::to_value(unused).unwrap()["lastValue"].is_null());
}
