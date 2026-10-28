//! GoogleSQL text: backtick identifiers, named `@pN` parameters (sent as STRING and
//! cast to the column type), LIKE with backslash escapes, and the transaction script
//! grid edits run as.

use crate::db::filter::Case;
use crate::engines::cloud_common::edits::Edit;
use crate::engines::cloud_common::sql::Flavor;

/// Marks the error a failed one-row check raises: `libratable-one-row:<change>:<rows>`.
pub const ONE_ROW_TAG: &str = "libratable-one-row:";

/// Grid SQL for tables of `project`.
pub struct Bq<'a> {
    pub project: &'a str,
}

pub fn quote(name: &str) -> String {
    format!("`{}`", name.replace('\\', "\\\\").replace('`', "\\`"))
}

/// A string literal with backslash escapes (previews only).
pub fn literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

fn base(ty: &str) -> &str {
    let end = ty.find(['(', '<']).unwrap_or(ty.len());
    ty[..end].trim()
}

fn nested(ty: &str) -> bool {
    matches!(base(ty), "STRUCT" | "ARRAY" | "RANGE")
}

impl Flavor for Bq<'_> {
    fn quote(&self, name: &str) -> String {
        quote(name)
    }

    fn table(&self, schema: &str, table: &str) -> String {
        format!("{}.{}.{}", quote(self.project), quote(schema), quote(table))
    }

    fn literal(&self, value: &str) -> String {
        literal(value)
    }

    fn marker(&self, n: usize) -> String {
        format!("@p{n}")
    }

    fn typed(&self, text: String, ty: &str) -> String {
        match base(ty) {
            "" | "STRING" => text,
            "JSON" => format!("PARSE_JSON({text})"),
            "GEOGRAPHY" => format!("ST_GEOGFROMTEXT({text})"),
            "BYTES" => format!("FROM_HEX({text})"),
            _ if nested(ty) => text,
            _ => format!("CAST({text} AS {ty})"),
        }
    }

    fn as_text(&self, col: &str, ty: &str) -> String {
        match base(ty) {
            "STRING" => col.to_string(),
            "JSON" => format!("TO_JSON_STRING({col})"),
            _ if nested(ty) => format!("TO_JSON_STRING({col})"),
            _ => format!("CAST({col} AS STRING)"),
        }
    }

    /// Default matching ignores case (TablePlus offers "case sensitive" variants).
    fn like(&self, text: &str, pattern: String, not: bool, case: Case) -> String {
        let not = if not { "NOT " } else { "" };
        match case {
            Case::Sensitive => format!("{text} {not}LIKE {pattern}"),
            _ => format!("LOWER({text}) {not}LIKE LOWER({pattern})"),
        }
    }

    fn escape_like(&self, value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for c in value.chars() {
            if matches!(c, '\\' | '%' | '_') {
                out.push('\\');
            }
            out.push(c);
        }
        out
    }

    fn scalar(&self, ty: &str) -> bool {
        !nested(ty)
    }

    fn update_default(&self) -> bool {
        true
    }
}

/// The edits as one script: a transaction in which every UPDATE / DELETE is preceded
/// by a count of its row (snapshot isolation makes the count what the statement will
/// touch); a count other than 1 raises, and any error rolls everything back.
pub fn edit_script(f: &Bq<'_>, edits: &[Edit], schema: &str, table: &str) -> String {
    let t = f.table(schema, table);
    let mut s = String::from("DECLARE libratable_n INT64;\nBEGIN\nBEGIN TRANSACTION;\n");
    for (i, e) in edits.iter().enumerate() {
        if let Some(w) = &e.key_where {
            s.push_str(&format!(
                "SET libratable_n = (SELECT COUNT(*) FROM {t} WHERE {w});\n\
                 IF libratable_n <> 1 THEN\n  RAISE USING MESSAGE = \
                 FORMAT('{ONE_ROW_TAG}%d:%d', {}, libratable_n);\nEND IF;\n",
                i + 1
            ));
        }
        s.push_str(&e.sql);
        s.push_str(";\n");
    }
    s.push_str(
        "COMMIT TRANSACTION;\nEXCEPTION WHEN ERROR THEN\n  ROLLBACK TRANSACTION;\n  \
         RAISE USING MESSAGE = @@error.message;\nEND;",
    );
    s
}

/// (change, rows) from an error raised by a failed one-row check.
pub fn one_row_failure(message: &str) -> Option<(usize, u64)> {
    let rest = &message[message.find(ONE_ROW_TAG)? + ONE_ROW_TAG.len()..];
    let mut it = rest.split(|c: char| !c.is_ascii_digit());
    let change = it.next()?.parse().ok()?;
    let rows = it.next()?.parse().ok()?;
    Some((change, rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::changes::Change;
    use crate::db::data::PageRequest;
    use crate::db::filter::FilterRow;
    use crate::db::meta::ColumnInfo;
    use crate::engines::cloud_common::edits::EditTable;
    use crate::engines::cloud_common::sql::{self as gsql, Binds};
    use serde_json::json;

    fn col(name: &str, ty: &str, pk: Option<i32>) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            data_type: ty.into(),
            nullable: true,
            default: None,
            pk_position: pk,
            auto_increment: false,
            comment: None,
            ..Default::default()
        }
    }

    fn cols() -> Vec<ColumnInfo> {
        vec![
            col("id", "INT64", Some(1)),
            col("name", "STRING", None),
            col("rec", "STRUCT<a INT64>", None),
        ]
    }

    fn row(column: &str, op: &str, value: &str) -> FilterRow {
        FilterRow {
            enabled: true,
            column: column.into(),
            operator: op.into(),
            value: value.into(),
            value2: String::new(),
        }
    }

    const OPS: &[&str] = &["eq", "in", "contains", "contains_cs", "has_prefix"];

    #[test]
    fn filters_use_named_typed_parameters() {
        let f = Bq { project: "my-proj" };
        let req = PageRequest {
            schema: "ds".into(),
            table: "t".into(),
            page: 0,
            page_size: 5,
            sort: vec![],
            filters: vec![
                row("id", "in", "1, 2"),
                row("name", "contains", "50%"),
                row("name", "contains_cs", "A_b"),
                row("rec", "eq", "{\"a\":1}"),
            ],
            apply_only: None,
        };
        let mut b = Binds::new();
        let s = gsql::select(&f, &cols(), &req, OPS, Some((5, 0)), &mut b).unwrap();
        assert_eq!(
            s,
            concat!(
                "SELECT `id`, `name`, `rec` FROM `my-proj`.`ds`.`t` ",
                "WHERE (`id` IN (CAST(@p1 AS INT64), CAST(@p2 AS INT64))) ",
                "AND (LOWER(`name`) LIKE LOWER(@p3)) AND (`name` LIKE @p4) ",
                "AND (TO_JSON_STRING(`rec`) = @p5) LIMIT 5 OFFSET 0"
            )
        );
        assert_eq!(b.values, vec!["1", "2", "%50\\%%", "%A\\_b%", "{\"a\":1}"]);
        assert_eq!(literal("a'b\\\n"), "'a\\'b\\\\\\n'");
    }

    #[test]
    fn edits_become_one_checked_transaction() {
        let f = Bq { project: "p" };
        let c = cols();
        let t = EditTable {
            schema: "ds",
            table: "t",
            columns: &c,
        };
        let map = |v: serde_json::Value| v.as_object().unwrap().clone();
        let mut b = Binds::new();
        let changes = [
            Change::Update {
                pk: map(json!({"id": 1})),
                values: map(json!({"name": "x"})),
            },
            Change::Insert {
                values: map(json!({"id": 2, "name": {"$default": true}})),
            },
        ];
        let edits: Vec<_> = changes
            .iter()
            .map(|ch| t.edit(&f, ch, &mut b).unwrap().unwrap())
            .collect();
        assert_eq!(
            edits[0].sql,
            "UPDATE `p`.`ds`.`t` SET `name` = @p1 WHERE `id` = CAST(@p2 AS INT64)"
        );
        assert_eq!(
            edits[1].sql,
            "INSERT INTO `p`.`ds`.`t` (`id`) VALUES (CAST(@p3 AS INT64))"
        );
        let script = edit_script(&f, &edits, "ds", "t");
        assert!(script.starts_with("DECLARE libratable_n INT64;\nBEGIN\nBEGIN TRANSACTION;\n"));
        assert!(script.contains(concat!(
            "SET libratable_n = (SELECT COUNT(*) FROM `p`.`ds`.`t` ",
            "WHERE `id` = CAST(@p2 AS INT64));"
        )));
        assert!(script.contains("FORMAT('libratable-one-row:%d:%d', 1, libratable_n)"));
        assert!(script
            .ends_with("ROLLBACK TRANSACTION;\n  RAISE USING MESSAGE = @@error.message;\nEND;"));
        assert_eq!(
            one_row_failure("Query error: libratable-one-row:3:0 at [5:3]"),
            Some((3, 0))
        );
        let nested = Change::Update {
            pk: map(json!({"id": 1})),
            values: map(json!({"rec": {"a": 2}})),
        };
        assert!(t.edit(&f, &nested, &mut Binds::new()).is_err());
    }
}
