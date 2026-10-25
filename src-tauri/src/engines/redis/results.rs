//! Replies as result tables: a scalar is one cell, an array one row per element
//! (`#`, `value`), a map (or HGETALL / CONFIG GET pairs) `field`/`value` rows, scores
//! `member`/`score` rows and INFO `section`/`field`/`value` rows. Nil is NULL.

use redis::Value as Reply;
use serde_json::Value;

use crate::db::exec::StatementResult;
use crate::db::value::ColumnMeta;

use super::values::{self, reply, scalar_text};

/// Commands whose flat array reply alternates field and value.
const PAIRS: &[&str] = &["HGETALL", "CONFIG"];
/// Commands that answer member, score pairs when given WITHSCORES (or a count).
const SCORED: &[&str] = &[
    "ZRANGE",
    "ZRANGEBYSCORE",
    "ZRANGEBYLEX",
    "ZREVRANGE",
    "ZREVRANGEBYSCORE",
    "ZRANDMEMBER",
    "ZUNION",
    "ZINTER",
    "ZDIFF",
];

fn col(name: &str, type_name: &str) -> ColumnMeta {
    ColumnMeta {
        name: name.into(),
        type_name: type_name.into(),
    }
}

fn has_arg(cmd: &[Vec<u8>], word: &str) -> bool {
    cmd.iter()
        .skip(1)
        .any(|a| a.eq_ignore_ascii_case(word.as_bytes()))
}

/// How a flat array reply of `cmd` pairs up, if it does.
fn pair_columns(cmd: &[Vec<u8>]) -> Option<(&'static str, &'static str)> {
    let name = String::from_utf8_lossy(cmd.first()?).to_ascii_uppercase();
    if PAIRS.contains(&name.as_str()) || (name == "HRANDFIELD" && has_arg(cmd, "WITHVALUES")) {
        return Some(("field", "value"));
    }
    let popped = (name == "ZPOPMIN" || name == "ZPOPMAX") && cmd.len() > 2;
    if popped || (SCORED.contains(&name.as_str()) && has_arg(cmd, "WITHSCORES")) {
        return Some(("member", "score"));
    }
    None
}

fn is_info(cmd: &[Vec<u8>]) -> bool {
    cmd.first().is_some_and(|c| c.eq_ignore_ascii_case(b"INFO"))
}

/// Table of one reply; rows past `limit` are counted, not kept.
pub fn table(out: &mut StatementResult, cmd: &[Vec<u8>], r: Reply, limit: usize) {
    let (columns, rows) = shape(cmd, r);
    out.columns = columns;
    out.total_rows = rows.len() as u64;
    out.truncated = rows.len() > limit;
    out.rows = rows.into_iter().take(limit).collect();
}

fn shape(cmd: &[Vec<u8>], r: Reply) -> (Vec<ColumnMeta>, Vec<Vec<Value>>) {
    match r {
        Reply::Attribute { data, .. } => shape(cmd, *data),
        Reply::Map(pairs) => (
            vec![col("field", "text"), col("value", "text")],
            pairs
                .iter()
                .map(|(k, v)| vec![reply(k), reply(v)])
                .collect(),
        ),
        Reply::Array(items) | Reply::Set(items) => array(cmd, items),
        Reply::Push { data, .. } => array(cmd, data),
        Reply::BulkString(b) if is_info(cmd) => info(&String::from_utf8_lossy(&b)),
        Reply::VerbatimString { text, .. } if is_info(cmd) => info(&text),
        other => (vec![col("value", "text")], vec![vec![reply(&other)]]),
    }
}

fn array(cmd: &[Vec<u8>], items: Vec<Reply>) -> (Vec<ColumnMeta>, Vec<Vec<Value>>) {
    let flat = items
        .iter()
        .all(|i| scalar_text(i).is_some() || matches!(i, Reply::Nil));
    if let Some((a, b)) = pair_columns(cmd).filter(|_| flat && items.len() % 2 == 0) {
        let rows = items
            .chunks(2)
            .map(|p| vec![reply(&p[0]), score_or_value(b, &p[1])])
            .collect();
        let b_type = if b == "score" { "double" } else { "text" };
        return (vec![col(a, "text"), col(b, b_type)], rows);
    }
    // RESP3 answers WITHSCORES as [member, score] pairs.
    if pair_columns(cmd).is_some_and(|(_, b)| b == "score") && items.iter().all(is_pair) {
        let rows = items
            .into_iter()
            .map(|p| {
                let pair = values::items(p);
                vec![reply(&pair[0]), score_or_value("score", &pair[1])]
            })
            .collect();
        return (vec![col("member", "text"), col("score", "double")], rows);
    }
    let rows = items
        .iter()
        .enumerate()
        .map(|(i, v)| vec![Value::from(i + 1), reply(v)])
        .collect();
    (vec![col("#", "integer"), col("value", "text")], rows)
}

fn is_pair(r: &Reply) -> bool {
    matches!(r, Reply::Array(p) if p.len() == 2)
}

fn score_or_value(column: &str, r: &Reply) -> Value {
    if column == "score" {
        if let Some(n) = scalar_text(r).and_then(|t| t.parse::<f64>().ok()) {
            return crate::db::value::float(n);
        }
    }
    reply(r)
}

/// INFO text (`# Section` headers, `field:value` lines) as rows.
fn info(text: &str) -> (Vec<ColumnMeta>, Vec<Vec<Value>>) {
    let mut section = String::new();
    let mut rows = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('#') {
            section = name.trim().to_string();
        } else if let Some((field, value)) = line.split_once(':') {
            rows.push(vec![
                Value::String(section.clone()),
                Value::String(field.into()),
                Value::String(value.into()),
            ]);
        }
    }
    let cols = vec![
        col("section", "text"),
        col("field", "text"),
        col("value", "text"),
    ];
    (cols, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::engines::redis::args::parse;

    fn run(line: &str, r: Reply) -> StatementResult {
        let mut out = StatementResult::new(line);
        table(&mut out, &parse(line).unwrap(), r, 2);
        out
    }

    fn bulk(s: &str) -> Reply {
        Reply::BulkString(s.as_bytes().to_vec())
    }

    fn names(r: &StatementResult) -> Vec<&str> {
        r.columns.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn scalars_and_nil() {
        let r = run("GET k", bulk("v"));
        assert_eq!(
            (names(&r), r.rows.clone()),
            (vec!["value"], vec![vec![json!("v")]])
        );
        assert_eq!(run("GET nope", Reply::Nil).rows, vec![vec![Value::Null]]);
        assert_eq!(run("SET k v", Reply::Okay).rows, vec![vec![json!("OK")]]);
        assert_eq!(run("INCR n", Reply::Int(5)).rows, vec![vec![json!(5)]]);
    }

    #[test]
    fn arrays_are_rows_and_truncate() {
        let r = run(
            "LRANGE l 0 -1",
            Reply::Array(vec![bulk("a"), bulk("b"), bulk("c")]),
        );
        assert_eq!(names(&r), ["#", "value"]);
        assert_eq!(
            r.rows,
            vec![vec![json!(1), json!("a")], vec![json!(2), json!("b")]]
        );
        assert!(r.truncated);
        assert_eq!(r.total_rows, 3);
    }

    #[test]
    fn pairs_maps_and_scores() {
        let r = run("HGETALL h", Reply::Array(vec![bulk("f"), bulk("v")]));
        assert_eq!(names(&r), ["field", "value"]);
        assert_eq!(r.rows, vec![vec![json!("f"), json!("v")]]);
        let m = Reply::Map(vec![(bulk("f"), bulk("v"))]);
        assert_eq!(names(&run("HGETALL h", m)), ["field", "value"]);
        let z = run(
            "ZRANGE z 0 -1 WITHSCORES",
            Reply::Array(vec![bulk("m"), bulk("1.5")]),
        );
        assert_eq!(z.rows, vec![vec![json!("m"), json!(1.5)]]);
        let plain = run("ZRANGE z 0 -1", Reply::Array(vec![bulk("m"), bulk("n")]));
        assert_eq!(names(&plain), ["#", "value"]);
    }

    #[test]
    fn info_sections() {
        let text = "# Server\r\nredis_version:7.2.4\r\n\r\n# Keyspace\r\ndb0:keys=1\r\n";
        let r = run("INFO", bulk(text));
        assert_eq!(names(&r), ["section", "field", "value"]);
        assert_eq!(
            r.rows[0],
            vec![json!("Server"), json!("redis_version"), json!("7.2.4")]
        );
    }
}
