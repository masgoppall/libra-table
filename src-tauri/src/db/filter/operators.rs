//! The filter operators, which of them each driver offers, and their menu labels.

use serde::Serialize;

use crate::models::Driver;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    In,
    NotIn,
    IsNull,
    IsNotNull,
    Between,
    NotBetween,
    Like,
    ILike,
    Contains(Case),
    NotContains(Case),
    HasPrefix(Case),
    HasSuffix(Case),
}

/// Case handling for the text operators. `Default` is the driver's natural behaviour
/// (case-sensitive on Postgres, collation-dependent on MySQL, ASCII-insensitive on SQLite).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    Default,
    Insensitive,
    Sensitive,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperatorInfo {
    pub id: &'static str,
    pub label: &'static str,
    /// "none" | "single" | "list" | "range"
    pub value_kind: &'static str,
    pub placeholder: &'static str,
}

impl Op {
    pub fn parse(id: &str) -> Option<Op> {
        use Case::*;
        Some(match id {
            "eq" => Op::Eq,
            "neq" => Op::Neq,
            "lt" => Op::Lt,
            "gt" => Op::Gt,
            "lte" => Op::Lte,
            "gte" => Op::Gte,
            "in" => Op::In,
            "not_in" => Op::NotIn,
            "is_null" => Op::IsNull,
            "is_not_null" => Op::IsNotNull,
            "between" => Op::Between,
            "not_between" => Op::NotBetween,
            "like" => Op::Like,
            "ilike" => Op::ILike,
            "contains" => Op::Contains(Default),
            "not_contains" => Op::NotContains(Default),
            "has_prefix" => Op::HasPrefix(Default),
            "has_suffix" => Op::HasSuffix(Default),
            "contains_ci" => Op::Contains(Insensitive),
            "not_contains_ci" => Op::NotContains(Insensitive),
            "has_prefix_ci" => Op::HasPrefix(Insensitive),
            "has_suffix_ci" => Op::HasSuffix(Insensitive),
            "contains_cs" => Op::Contains(Sensitive),
            "not_contains_cs" => Op::NotContains(Sensitive),
            "has_prefix_cs" => Op::HasPrefix(Sensitive),
            "has_suffix_cs" => Op::HasSuffix(Sensitive),
            _ => return None,
        })
    }

    pub(super) fn is_negative(self) -> bool {
        matches!(
            self,
            Op::Neq | Op::NotIn | Op::NotBetween | Op::NotContains(_) | Op::IsNotNull
        )
    }

    /// The positive form, used to build `NOT (c1 … OR c2 …)` for Any column.
    pub(super) fn positive(self) -> Op {
        match self {
            Op::Neq => Op::Eq,
            Op::NotIn => Op::In,
            Op::NotBetween => Op::Between,
            Op::NotContains(c) => Op::Contains(c),
            Op::IsNotNull => Op::IsNull,
            other => other,
        }
    }
}

/// (id, label, value kind, placeholder)
type Entry = (&'static str, &'static str, &'static str, &'static str);

const COMMON: &[Entry] = &[
    ("eq", "=", "single", "value"),
    ("neq", "<>", "single", "value"),
    ("lt", "<", "single", "value"),
    ("gt", ">", "single", "value"),
    ("lte", "<=", "single", "value"),
    ("gte", ">=", "single", "value"),
    ("in", "IN", "list", "a, b, 'c d'"),
    ("not_in", "NOT IN", "list", "a, b, 'c d'"),
    ("is_null", "IS NULL", "none", ""),
    ("is_not_null", "IS NOT NULL", "none", ""),
    ("between", "BETWEEN", "range", "from"),
    ("not_between", "NOT BETWEEN", "range", "from"),
    ("like", "LIKE", "single", "pattern with % and _"),
];

const PG_ILIKE: Entry = ("ilike", "ILIKE", "single", "pattern with % and _");

const TEXT: &[Entry] = &[
    ("contains", "Contains", "single", "text"),
    ("not_contains", "Not contains", "single", "text"),
    ("has_prefix", "Has prefix", "single", "text"),
    ("has_suffix", "Has suffix", "single", "text"),
];

const PG_CASE_INSENSITIVE: &[Entry] = &[
    (
        "contains_ci",
        "Contains (case-insensitive)",
        "single",
        "text",
    ),
    (
        "not_contains_ci",
        "Not contains (case-insensitive)",
        "single",
        "text",
    ),
    (
        "has_prefix_ci",
        "Has prefix (case-insensitive)",
        "single",
        "text",
    ),
    (
        "has_suffix_ci",
        "Has suffix (case-insensitive)",
        "single",
        "text",
    ),
];

const MYSQL_CASE_SENSITIVE: &[Entry] = &[
    ("contains_cs", "Contains (case-sensitive)", "single", "text"),
    (
        "not_contains_cs",
        "Not contains (case-sensitive)",
        "single",
        "text",
    ),
    (
        "has_prefix_cs",
        "Has prefix (case-sensitive)",
        "single",
        "text",
    ),
    (
        "has_suffix_cs",
        "Has suffix (case-sensitive)",
        "single",
        "text",
    ),
];

/// The operators a driver offers, in menu order (FLT-2). The UI never hardcodes this.
pub fn operators(driver: Driver) -> Vec<OperatorInfo> {
    let mut entries: Vec<Entry> = COMMON.to_vec();
    if driver == Driver::Postgres {
        entries.push(PG_ILIKE);
    }
    entries.extend_from_slice(TEXT);
    match driver {
        Driver::Postgres => entries.extend_from_slice(PG_CASE_INSENSITIVE),
        Driver::Mysql => entries.extend_from_slice(MYSQL_CASE_SENSITIVE),
        Driver::Sqlite => {}
    }
    entries
        .into_iter()
        .map(|(id, label, value_kind, placeholder)| OperatorInfo {
            id,
            label,
            value_kind,
            placeholder,
        })
        .collect()
}

pub(super) fn supported(driver: Driver, op: Op) -> bool {
    match op {
        Op::ILike => driver == Driver::Postgres,
        Op::Contains(Case::Insensitive)
        | Op::NotContains(Case::Insensitive)
        | Op::HasPrefix(Case::Insensitive)
        | Op::HasSuffix(Case::Insensitive) => driver == Driver::Postgres,
        Op::Contains(Case::Sensitive)
        | Op::NotContains(Case::Sensitive)
        | Op::HasPrefix(Case::Sensitive)
        | Op::HasSuffix(Case::Sensitive) => driver == Driver::Mysql,
        _ => true,
    }
}
