//! MySQL: functions, procedures, triggers and events.

use super::*;

const MYSQL_SETUP: &[&str] = &[
    "CREATE TABLE people (id int PRIMARY KEY AUTO_INCREMENT, name varchar(50))",
    "CREATE FUNCTION twice(x INT) RETURNS INT DETERMINISTIC RETURN x * 2",
    "CREATE PROCEDURE tidy(IN keep INT, OUT removed INT)
     BEGIN DELETE FROM people WHERE id > keep; SET removed = ROW_COUNT(); END",
    "CREATE TRIGGER bi BEFORE INSERT ON people FOR EACH ROW SET NEW.name = UPPER(NEW.name)",
    "CREATE EVENT nightly ON SCHEDULE EVERY 1 DAY DO DELETE FROM people WHERE id < 0",
];

pub(super) async fn mysql_objects(s: &Session) {
    for sql in MYSQL_SETUP {
        run_script(s, sql).await;
    }
    let list = listed(s, DB).await;
    let f = find(&list, ObjectKind::Function, "twice");
    assert_eq!(
        (f.arguments.as_deref(), f.detail.as_deref()),
        (Some("x int"), Some("returns int"))
    );
    let p = find(&list, ObjectKind::Procedure, "tidy");
    assert_eq!(p.arguments.as_deref(), Some("IN keep int, OUT removed int"));
    let t = find(&list, ObjectKind::Trigger, "bi");
    assert_eq!(t.detail.as_deref(), Some("BEFORE INSERT · row"));
    let e = find(&list, ObjectKind::Event, "nightly");
    assert_eq!(e.detail.as_deref(), Some("every 1 day · enabled"));

    for (kind, name) in [
        (ObjectKind::Function, "twice"),
        (ObjectKind::Procedure, "tidy"),
        (ObjectKind::Trigger, "bi"),
        (ObjectKind::Event, "nightly"),
    ] {
        let def = definition(s, DB, &object(kind, name)).await;
        assert!(def.ddl.starts_with("CREATE"), "{}", def.ddl);
        assert!(def.edit_script.starts_with("DROP "));
        run_script(s, &def.edit_script).await;
    }
    run_script(s, "INSERT INTO people (name) VALUES ('ann')").await;
    assert_eq!(scalar_i64(s, "SELECT twice(21)").await, 42);

    for (kind, name) in [
        ("function", "twice"),
        ("procedure", "tidy"),
        ("trigger", "bi"),
        ("event", "nightly"),
    ] {
        change(s, drop(DB, kind, name, json!({}))).await.unwrap();
    }
    assert!(listed(s, DB).await.is_empty());
}
