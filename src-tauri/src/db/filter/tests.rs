//! Filter building, end to end per driver.

use super::*;

fn cols() -> Vec<(String, String)> {
    vec![
        ("id".into(), "integer".into()),
        ("name".into(), "text".into()),
        ("note".into(), "text".into()),
    ]
}

fn row(column: &str, operator: &str, value: &str) -> FilterRow {
    FilterRow {
        enabled: true,
        column: column.into(),
        operator: operator.into(),
        value: value.into(),
        value2: String::new(),
    }
}

#[test]
fn postgres_casts_binds_to_column_type() {
    let c = cols();
    let f = FilterBuilder::new(Driver::Postgres, &c, 1)
        .build(&[row("id", "eq", "5")], None)
        .unwrap();
    assert_eq!(f.sql, r#"("id" = $1::integer)"#);
    assert_eq!(f.binds, vec!["5"]);
}

#[test]
fn contains_escapes_wildcards_and_binds_value() {
    let c = cols();
    let f = FilterBuilder::new(Driver::Mysql, &c, 1)
        .build(&[row("name", "contains", "50%_off!")], None)
        .unwrap();
    assert_eq!(f.sql, "(`name` LIKE ? ESCAPE '!')");
    assert_eq!(f.binds, vec!["%50!%!_off!!%"]);
}

#[test]
fn injection_attempt_stays_a_value() {
    let c = cols();
    let evil = "'; DROP TABLE x; --";
    let f = FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row("name", "eq", evil)], None)
        .unwrap();
    assert_eq!(f.sql, r#"("name" = ?)"#);
    assert_eq!(f.binds, vec![evil]);
}

#[test]
fn rows_combine_with_and_and_skip_disabled() {
    let c = cols();
    let mut disabled = row("note", "is_null", "");
    disabled.enabled = false;
    let mut between = row("id", "between", "1");
    between.value2 = "10".into();
    let f = FilterBuilder::new(Driver::Postgres, &c, 1)
        .build(&[row("name", "has_prefix", "ab"), disabled, between], None)
        .unwrap();
    assert_eq!(
        f.sql,
        r#"("name"::text LIKE $1 ESCAPE '!') AND ("id" BETWEEN $2::integer AND $3::integer)"#
    );
    assert_eq!(f.binds, vec!["ab%", "1", "10"]);
}

#[test]
fn single_row_apply_ignores_enabled_flag() {
    let c = cols();
    let mut r = row("id", "gt", "3");
    r.enabled = false;
    let f = FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row("name", "eq", "x"), r], Some(1))
        .unwrap();
    assert_eq!(f.sql, r#"("id" > ?)"#);
}

#[test]
fn in_list_becomes_one_param_per_item() {
    let c = cols();
    let f = FilterBuilder::new(Driver::Postgres, &c, 3)
        .build(&[row("name", "not_in", "a, 'b, c', \"d\"")], None)
        .unwrap();
    assert_eq!(f.sql, r#"("name" NOT IN ($3::text, $4::text, $5::text))"#);
    assert_eq!(f.binds, vec!["a", "b, c", "d"]);
}

#[test]
fn any_column_positive_and_negative() {
    let c = cols();
    let f = FilterBuilder::new(Driver::Postgres, &c, 1)
        .build(&[row(ANY_COLUMN, "contains_ci", "x")], None)
        .unwrap();
    assert_eq!(
        f.sql,
        concat!(
            r#"("id"::text ILIKE $1 ESCAPE '!' OR "name"::text ILIKE $2 ESCAPE '!' "#,
            r#"OR "note"::text ILIKE $3 ESCAPE '!')"#
        )
    );
    let f = FilterBuilder::new(Driver::Mysql, &c, 1)
        .build(&[row(ANY_COLUMN, "eq", "abc")], None)
        .unwrap();
    assert_eq!(
        f.sql,
        "(CAST(`id` AS CHAR) = ? OR CAST(`name` AS CHAR) = ? OR CAST(`note` AS CHAR) = ?)"
    );
    let f = FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row(ANY_COLUMN, "is_not_null", "")], None)
        .unwrap();
    assert_eq!(
        f.sql,
        concat!(
            r#"(NOT (COALESCE("id" IS NULL, FALSE) OR COALESCE("name" IS NULL, FALSE) "#,
            r#"OR COALESCE("note" IS NULL, FALSE)))"#
        )
    );
}

#[test]
fn mysql_case_sensitive_and_postgres_only_ops() {
    let c = cols();
    let f = FilterBuilder::new(Driver::Mysql, &c, 1)
        .build(&[row("name", "not_contains_cs", "A")], None)
        .unwrap();
    assert_eq!(f.sql, "(CAST(`name` AS BINARY) NOT LIKE ? ESCAPE '!')");
    assert!(FilterBuilder::new(Driver::Mysql, &c, 1)
        .build(&[row("name", "ilike", "a")], None)
        .is_err());
}

#[test]
fn raw_sql_is_wrapped_and_rejects_semicolons() {
    let c = cols();
    let f = FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row(RAW_SQL, "", "id = 1 OR id = 2")], None)
        .unwrap();
    assert_eq!(f.sql, "(id = 1 OR id = 2)");
    assert!(FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row(RAW_SQL, "", "1=1; drop table x")], None)
        .is_err());
}

#[test]
fn validation_errors() {
    let c = cols();
    assert!(FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row("missing", "eq", "1")], None)
        .is_err());
    assert!(FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row("id", "between", "1")], None)
        .is_err());
    assert!(FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row("id", "in", " , ")], None)
        .is_err());
    assert!(FilterBuilder::new(Driver::Sqlite, &c, 1)
        .build(&[row("id", "bogus", "1")], None)
        .is_err());
}

#[test]
fn preview_inlines_literals() {
    let c = cols();
    let f = FilterBuilder::new(Driver::Postgres, &c, 1)
        .build(&[row("name", "eq", "O'Brien")], None)
        .unwrap();
    assert_eq!(
        inline_preview(Driver::Postgres, &f),
        r#"("name" = 'O''Brien'::text)"#
    );
    let f = FilterBuilder::new(Driver::Mysql, &c, 1)
        .build(&[row("name", "eq", "a?b")], None)
        .unwrap();
    assert_eq!(inline_preview(Driver::Mysql, &f), "(`name` = 'a?b')");
}

#[test]
fn order_by_validates_columns() {
    let c = cols();
    let s = vec![SortSpec {
        column: "name".into(),
        desc: true,
    }];
    assert_eq!(order_by(Driver::Mysql, &c, &s).unwrap(), "`name` DESC");
    assert!(order_by(
        Driver::Mysql,
        &c,
        &[SortSpec {
            column: "x; drop".into(),
            desc: false
        }]
    )
    .is_err());
}

#[test]
fn operator_lists_follow_driver_capabilities() {
    let pg: Vec<_> = operators(Driver::Postgres).iter().map(|o| o.id).collect();
    let my: Vec<_> = operators(Driver::Mysql).iter().map(|o| o.id).collect();
    let lite: Vec<_> = operators(Driver::Sqlite).iter().map(|o| o.id).collect();
    assert!(pg.contains(&"ilike") && pg.contains(&"contains_ci") && !pg.contains(&"contains_cs"));
    assert!(my.contains(&"contains_cs") && !my.contains(&"ilike"));
    assert!(
        !lite.contains(&"contains_ci") && !lite.contains(&"contains_cs") && lite.contains(&"like")
    );
    for id in pg.iter().chain(&my).chain(&lite) {
        assert!(Op::parse(id).is_some(), "{id} must parse");
    }
}
