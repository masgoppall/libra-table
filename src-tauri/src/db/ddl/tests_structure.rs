//! What the structure tab adds to ALTER TABLE: MySQL character sets, collations, column
//! attributes and positions, index algorithms and renaming the table.

use super::spec::ColumnPosition;
use super::*;
use crate::models::Dialect;

fn col(name: &str, ty: &str) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        data_type: ty.into(),
        nullable: true,
        default: None,
        primary_key: false,
        auto_increment: false,
        unique: false,
        comment: None,
        charset: None,
        collation: None,
        extra: None,
    }
}

fn sqls(driver: Dialect, ops: Vec<AlterOp>, rename_to: Option<&str>) -> AppResultSqls {
    let change = SchemaChange::AlterTable {
        schema: "s".into(),
        name: "t".into(),
        ops,
        rename_to: rename_to.map(str::to_string),
    };
    build(driver, &change, &Ctx::default()).map(|p| {
        p.steps
            .iter()
            .map(|s| match s {
                Step::Sql(s) => s.clone(),
                Step::Rebuild(_) => "<rebuild>".into(),
            })
            .collect()
    })
}

type AppResultSqls = crate::error::AppResult<Vec<String>>;

fn modify(original: ColumnSpec, column: ColumnSpec, position: Option<ColumnPosition>) -> AlterOp {
    AlterOp::ModifyColumn {
        original,
        column,
        position,
    }
}

#[test]
fn mysql_change_column_keeps_charset_collation_and_on_update() {
    let mut slogan = col("app_slogan", "text");
    slogan.nullable = false;
    slogan.charset = Some("latin1".into());
    slogan.collation = Some("latin1_swedish_ci".into());
    let mut renamed = slogan.clone();
    renamed.name = "slogan".into();
    let mut stamp = col("updated_at", "timestamp");
    stamp.default = Some("CURRENT_TIMESTAMP".into());
    stamp.extra = Some("DEFAULT_GENERATED on update CURRENT_TIMESTAMP".into());
    let mut commented = stamp.clone();
    commented.comment = Some("last change".into());
    let out = sqls(
        Dialect::Mysql,
        vec![
            modify(slogan, renamed, None),
            modify(stamp, commented, None),
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        out,
        vec![
            "ALTER TABLE `s`.`t` CHANGE COLUMN `app_slogan` `slogan` text CHARACTER SET latin1 \
             COLLATE latin1_swedish_ci NOT NULL",
            "ALTER TABLE `s`.`t` CHANGE COLUMN `updated_at` `updated_at` timestamp NULL \
             DEFAULT CURRENT_TIMESTAMP on update CURRENT_TIMESTAMP COMMENT 'last change'",
        ]
    );
}

#[test]
fn mysql_charset_only_on_text_types_and_checked() {
    let mut id = col("id", "bigint");
    id.charset = Some("latin1".into());
    let mut big = id.clone();
    big.data_type = "bigint unsigned".into();
    let out = sqls(Dialect::Mysql, vec![modify(id, big, None)], None).unwrap();
    assert_eq!(
        out,
        vec!["ALTER TABLE `s`.`t` CHANGE COLUMN `id` `id` bigint unsigned NULL"]
    );

    let mut bad = col("name", "varchar(20)");
    bad.collation = Some("utf8mb4_bin; DROP TABLE x".into());
    let add = AlterOp::AddColumn {
        column: bad,
        position: None,
    };
    assert!(sqls(Dialect::Mysql, vec![add], None).is_err());

    let mut extra = col("n", "int");
    extra.extra = Some("SERIAL DEFAULT VALUE; DROP TABLE t".into());
    let add = AlterOp::AddColumn {
        column: extra,
        position: None,
    };
    assert!(sqls(Dialect::Mysql, vec![add], None).is_err());
}

#[test]
fn mysql_positions_move_and_place_columns() {
    let a = col("a", "int");
    let out = sqls(
        Dialect::Mysql,
        vec![
            modify(
                a.clone(),
                a.clone(),
                Some(ColumnPosition::After("c".into())),
            ),
            AlterOp::AddColumn {
                column: col("z", "int"),
                position: Some(ColumnPosition::First),
            },
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        out,
        vec![
            "ALTER TABLE `s`.`t` CHANGE COLUMN `a` `a` int NULL AFTER `c`",
            "ALTER TABLE `s`.`t` ADD COLUMN `z` int NULL FIRST",
        ]
    );
    // An unchanged column without a position writes nothing.
    assert!(sqls(Dialect::Mysql, vec![modify(a.clone(), a, None)], None)
        .unwrap()
        .is_empty());
}

#[test]
fn positions_are_refused_where_columns_cannot_move() {
    for driver in [Dialect::Postgres, Dialect::Sqlite] {
        let a = col("a", "integer");
        let err = sqls(
            driver,
            vec![modify(a.clone(), a, Some(ColumnPosition::First))],
            None,
        )
        .unwrap_err();
        assert!(err.message.contains("ordinal position"), "{}", err.message);
    }
}

#[test]
fn index_algorithms() {
    let index = |algorithm: &str| AlterOp::AddIndex {
        index: IndexSpec {
            name: "t_a".into(),
            columns: vec!["a".into()],
            unique: false,
            algorithm: algorithm.into(),
        },
    };
    assert_eq!(
        sqls(Dialect::Mysql, vec![index("hash")], None).unwrap(),
        vec!["CREATE INDEX `t_a` ON `s`.`t` (`a`) USING HASH"]
    );
    assert_eq!(
        sqls(Dialect::Postgres, vec![index("GIN")], None).unwrap(),
        vec!["CREATE INDEX \"t_a\" ON \"s\".\"t\" USING gin (\"a\")"]
    );
    assert_eq!(
        sqls(Dialect::Postgres, vec![index("")], None).unwrap(),
        vec!["CREATE INDEX \"t_a\" ON \"s\".\"t\" (\"a\")"]
    );
    assert!(sqls(Dialect::Mysql, vec![index("btree (a); --")], None).is_err());
}

#[test]
fn renaming_comes_after_the_changes() {
    let out = sqls(
        Dialect::Mysql,
        vec![AlterOp::DropColumn { name: "a".into() }],
        Some("contents_v2"),
    )
    .unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[1], "RENAME TABLE `s`.`t` TO `s`.`contents_v2`");
    // The same name, or only spaces, renames nothing.
    assert!(sqls(Dialect::Postgres, vec![], Some(" t "))
        .unwrap()
        .is_empty());
    // SQLite can't rename inside a table rebuild.
    let pk = AlterOp::SetPrimaryKey {
        columns: vec!["a".into()],
    };
    assert!(sqls(Dialect::Sqlite, vec![pk], Some("u")).is_err());
}
