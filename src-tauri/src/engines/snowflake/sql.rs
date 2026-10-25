//! Snowflake SQL text: `"quoted"` identifiers, `?` binds sent as TEXT and cast to the
//! column type, LIKE with `ESCAPE '!'`.

use crate::db::filter::{escape_like, Case};
use crate::engines::cloud_common::sql::Flavor;

/// Grid SQL for tables of `database` (empty: the session's current database).
pub struct Sf<'a> {
    pub database: &'a str,
}

pub fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A string literal: `'` doubled and `\` escaped (Snowflake reads backslash escapes).
pub fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''"))
}

fn base(ty: &str) -> String {
    ty.split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase()
}

impl Flavor for Sf<'_> {
    fn quote(&self, name: &str) -> String {
        quote(name)
    }

    fn table(&self, schema: &str, table: &str) -> String {
        let mut parts = Vec::new();
        if !self.database.is_empty() {
            parts.push(quote(self.database));
        }
        if !schema.is_empty() {
            parts.push(quote(schema));
        }
        parts.push(quote(table));
        parts.join(".")
    }

    fn literal(&self, value: &str) -> String {
        literal(value)
    }

    fn marker(&self, _n: usize) -> String {
        "?".into()
    }

    fn typed(&self, text: String, ty: &str) -> String {
        match base(ty).as_str() {
            "" | "VARCHAR" | "TEXT" | "STRING" | "CHAR" | "CHARACTER" => text,
            "VARIANT" | "OBJECT" | "ARRAY" => format!("PARSE_JSON({text})"),
            "GEOGRAPHY" => format!("TO_GEOGRAPHY({text})"),
            "GEOMETRY" => format!("TO_GEOMETRY({text})"),
            "BINARY" | "VARBINARY" => format!("TO_BINARY({text}, 'HEX')"),
            _ => format!("CAST({text} AS {ty})"),
        }
    }

    fn as_text(&self, col: &str, _ty: &str) -> String {
        format!("TO_VARCHAR({col})")
    }

    fn like(&self, text: &str, pattern: String, not: bool, _case: Case) -> String {
        let not = if not { "NOT " } else { "" };
        format!("{text} {not}LIKE {pattern} ESCAPE '!'")
    }

    fn escape_like(&self, value: &str) -> String {
        escape_like(value)
    }

    fn scalar(&self, _ty: &str) -> bool {
        true
    }

    fn update_default(&self) -> bool {
        false
    }

    /// Snowflake refuses functions such as PARSE_JSON inside VALUES, so use SELECT.
    fn insert_values(&self, values: &[String]) -> String {
        format!("SELECT {}", values.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::changes::Change;
    use crate::db::data::PageRequest;
    use crate::db::filter::{FilterRow, SortSpec, ANY_COLUMN};
    use crate::db::meta::ColumnInfo;
    use crate::engines::cloud_common::edits::EditTable;
    use crate::engines::cloud_common::sql::{self, Binds};
    use serde_json::json;

    fn col(name: &str, ty: &str, pk: Option<i32>) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            data_type: ty.into(),
            nullable: pk.is_none(),
            default: None,
            pk_position: pk,
            auto_increment: false,
            comment: None,
        }
    }

    fn cols() -> Vec<ColumnInfo> {
        vec![
            col("ID", "NUMBER(38,0)", Some(1)),
            col("NAME", "VARCHAR(100)", None),
            col("DOC", "VARIANT", None),
        ]
    }

    fn row(column: &str, op: &str, value: &str) -> FilterRow {
        FilterRow {
            enabled: true,
            column: column.into(),
            operator: op.into(),
            value: value.into(),
            value2: "9".into(),
        }
    }

    const OPS: &[&str] = &["eq", "neq", "in", "between", "contains", "is_null"];

    #[test]
    fn page_with_filters_sort_and_binds() {
        let f = Sf { database: "DB" };
        let req = PageRequest {
            schema: "PUBLIC".into(),
            table: "PEOPLE".into(),
            page: 2,
            page_size: 10,
            sort: vec![SortSpec {
                column: "NAME".into(),
                desc: true,
            }],
            filters: vec![row("ID", "between", "1"), row("NAME", "contains", "50%_x")],
            apply_only: None,
        };
        let mut b = Binds::new();
        let s = sql::select(&f, &cols(), &req, OPS, Some((10, 20)), &mut b).unwrap();
        assert_eq!(
            s,
            concat!(
                r#"SELECT "ID", "NAME", "DOC" FROM "DB"."PUBLIC"."PEOPLE" "#,
                r#"WHERE ("ID" BETWEEN CAST(? AS NUMBER(38,0)) AND CAST(? AS NUMBER(38,0))) "#,
                r#"AND (TO_VARCHAR("NAME") LIKE ? ESCAPE '!') "#,
                r#"ORDER BY "NAME" DESC LIMIT 10 OFFSET 20"#
            )
        );
        assert_eq!(b.values, vec!["1", "9", "%50!%!_x%"]);
        let mut inline = Binds::inline();
        let preview = sql::filter(&f, &cols(), &req.filters, Some(1), OPS, &mut inline).unwrap();
        assert_eq!(
            preview,
            r#"(TO_VARCHAR("NAME") LIKE '%50!%!_x%' ESCAPE '!')"#
        );
    }

    #[test]
    fn any_column_negative_and_unknown_operators() {
        let f = Sf { database: "" };
        let mut b = Binds::new();
        let any = [row(ANY_COLUMN, "neq", "o'k\\")];
        let s = sql::filter(&f, &cols()[..2], &any, None, OPS, &mut b).unwrap();
        assert_eq!(
            s,
            concat!(
                r#"(NOT (COALESCE(TO_VARCHAR("ID") = ?, FALSE) OR "#,
                r#"COALESCE(TO_VARCHAR("NAME") = ?, FALSE)))"#
            )
        );
        let mut inline = Binds::inline();
        let p = sql::filter(&f, &cols()[..1], &any, None, OPS, &mut inline).unwrap();
        assert!(p.contains(r"'o''k\\'"));
        let bad = [row("ID", "like", "x")];
        assert!(sql::filter(&f, &cols(), &bad, None, OPS, &mut b).is_err());
    }

    #[test]
    fn edits_by_primary_key() {
        let f = Sf { database: "DB" };
        let c = cols();
        let t = EditTable {
            schema: "S",
            table: "T",
            columns: &c,
        };
        let map = |v: serde_json::Value| v.as_object().unwrap().clone();
        let upd = Change::Update {
            pk: map(json!({"ID": 7})),
            values: map(json!({"DOC": {"a": 1}, "NAME": null})),
        };
        let mut b = Binds::new();
        let e = t.edit(&f, &upd, &mut b).unwrap().unwrap();
        assert_eq!(
            e.sql,
            concat!(
                r#"UPDATE "DB"."S"."T" SET "DOC" = PARSE_JSON(?), "NAME" = NULL "#,
                r#"WHERE "ID" = CAST(? AS NUMBER(38,0))"#
            )
        );
        assert_eq!(b.values, vec![r#"{"a":1}"#, "7"]);
        let ins = Change::Insert {
            values: map(json!({"ID": 8, "NAME": "x"})),
        };
        let e = t.edit(&f, &ins, &mut Binds::inline()).unwrap().unwrap();
        assert_eq!(
            e.sql,
            r#"INSERT INTO "DB"."S"."T" ("ID", "NAME") SELECT CAST('8' AS NUMBER(38,0)), 'x'"#
        );
        assert!(e.key_where.is_none());
        let reset = Change::Update {
            pk: map(json!({"ID": 7})),
            values: map(json!({"NAME": {"$default": true}})),
        };
        assert!(t.edit(&f, &reset, &mut Binds::new()).is_err());
        let del = Change::Delete {
            pk: map(json!({"ID": null})),
        };
        assert!(t.edit(&f, &del, &mut Binds::new()).is_err());
    }
}
