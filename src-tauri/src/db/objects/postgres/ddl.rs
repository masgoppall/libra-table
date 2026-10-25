//! Postgres catalog rows turned into readable descriptions and DDL. Pure, so the
//! mapping is unit tested without a server.

use crate::db::sql::literal;

use super::super::{ObjectDefinition, SequenceInfo};

/// One row of the type definition query: (typtype, base type, NOT NULL, default,
/// enum labels, range subtype, composite attributes, domain constraints).
pub type TypeRow = (
    String,
    Option<String>,
    bool,
    Option<String>,
    Vec<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
);

// pg_trigger.tgtype bits (catalog/pg_trigger.h).
const ROW: i32 = 1;
const BEFORE: i32 = 2;
const INSERT: i32 = 4;
const DELETE: i32 = 8;
const UPDATE: i32 = 16;
const TRUNCATE: i32 = 32;
const INSTEAD: i32 = 64;

/// "BEFORE INSERT OR UPDATE · row" from `pg_trigger.tgtype`.
pub fn trigger_detail(tgtype: i32) -> String {
    let timing = if tgtype & INSTEAD != 0 {
        "INSTEAD OF"
    } else if tgtype & BEFORE != 0 {
        "BEFORE"
    } else {
        "AFTER"
    };
    let events: Vec<&str> = [
        (INSERT, "INSERT"),
        (UPDATE, "UPDATE"),
        (DELETE, "DELETE"),
        (TRUNCATE, "TRUNCATE"),
    ]
    .iter()
    .filter(|(bit, _)| tgtype & bit != 0)
    .map(|(_, e)| *e)
    .collect();
    let level = if tgtype & ROW != 0 {
        "row"
    } else {
        "statement"
    };
    format!("{timing} {} · {level}", events.join(" OR "))
}

/// "enum", "composite", "domain over integer" or "range" from `pg_type.typtype`.
pub fn type_detail(typtype: &str, base: Option<&str>) -> String {
    match typtype {
        "e" => "enum".into(),
        "c" => "composite".into(),
        "d" => format!("domain over {}", base.unwrap_or("?")),
        "r" => "range".into(),
        other => other.to_string(),
    }
}

/// CREATE TYPE / CREATE DOMAIN for a type row, and a drop-and-create edit script.
pub fn type_ddl(target: &str, row: &TypeRow) -> ObjectDefinition {
    let (typtype, base, not_null, default, labels, subtype, attributes, constraints) = row;
    let (ddl, drop, note) = match typtype.as_str() {
        "e" => {
            let labels: Vec<String> = labels.iter().map(|l| literal(l)).collect();
            let ddl = format!("CREATE TYPE {target} AS ENUM ({});", labels.join(", "));
            let note = "-- ALTER TYPE … ADD VALUE adds a label without dropping the type.\n";
            (ddl, "TYPE", note)
        }
        "d" => {
            let mut ddl = format!(
                "CREATE DOMAIN {target} AS {}",
                base.as_deref().unwrap_or("")
            );
            if let Some(d) = default {
                ddl.push_str(&format!(" DEFAULT {d}"));
            }
            if *not_null {
                ddl.push_str(" NOT NULL");
            }
            for c in constraints {
                ddl.push_str(&format!("\n    {c}"));
            }
            ddl.push(';');
            (ddl, "DOMAIN", "")
        }
        "r" => {
            let sub = subtype.as_deref().unwrap_or("");
            (
                format!("CREATE TYPE {target} AS RANGE (SUBTYPE = {sub});"),
                "TYPE",
                "",
            )
        }
        _ => {
            let body = attributes
                .iter()
                .map(|a| format!("    {a}"))
                .collect::<Vec<_>>()
                .join(",\n");
            (format!("CREATE TYPE {target} AS (\n{body}\n);"), "TYPE", "")
        }
    };
    let warn = "-- Dropping fails while columns or functions use the type.\n";
    ObjectDefinition {
        edit_script: format!("{note}{warn}DROP {drop} {target};\n{ddl}"),
        ddl,
    }
}

/// `CREATE SEQUENCE …` or `ALTER SEQUENCE …` with every setting spelled out.
pub fn sequence_ddl(verb: &str, target: &str, s: &SequenceInfo) -> String {
    let as_type = if verb == "CREATE" {
        format!(" AS {}", s.data_type)
    } else {
        String::new()
    };
    let cycle = if s.cycle { "CYCLE" } else { "NO CYCLE" };
    format!(
        "{verb} SEQUENCE {target}{as_type}\n    INCREMENT BY {}\n    MINVALUE {}\n    \
         MAXVALUE {}\n    START WITH {}\n    CACHE {}\n    {cycle};",
        s.increment, s.min_value, s.max_value, s.start_value, s.cache
    )
}
