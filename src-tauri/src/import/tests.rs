//! Parsing CSV/JSON files, the preview, and streaming SQL files.

use std::io::Cursor;

use serde_json::json;

use super::source::{self, CsvEvent, Record};
use super::sqlfile::{Piece, StatementReader};
use super::{preview_file, CsvOptions, Encoding, FileFormat, Kind, SourceSpec};
use crate::db::split;
use crate::models::Dialect;

fn csv_events(text: &[u8], o: &CsvOptions) -> Vec<CsvEvent> {
    let mut out = Vec::new();
    source::read_csv(Cursor::new(text.to_vec()), o, |e| {
        out.push(e);
        true
    })
    .unwrap();
    out
}

fn values(e: &CsvEvent) -> Vec<Option<String>> {
    match e {
        CsvEvent::Row(Record { values: Ok(v), .. }) => v.clone(),
        other => panic!("expected a row, got {other:?}"),
    }
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

#[test]
fn csv_header_quotes_and_nulls() {
    let text = b"id;name;note\n1;\"Smith; John\";NULL\n2;\"multi\nline\";\n";
    let o = CsvOptions {
        delimiter: ";".into(),
        ..Default::default()
    };
    let ev = csv_events(text, &o);
    assert_eq!(
        ev[0],
        CsvEvent::Columns(vec!["id".into(), "name".into(), "note".into()])
    );
    assert_eq!(values(&ev[1]), vec![s("1"), s("Smith; John"), None]);
    assert_eq!(values(&ev[2]), vec![s("2"), s("multi\nline"), None]);
    let CsvEvent::Row(r) = &ev[2] else { panic!() };
    assert_eq!((r.row, r.line), (2, Some(3)));
}

#[test]
fn csv_skip_rows_no_header_and_empty_strings() {
    let text = b"exported by tool\na,,c\n";
    let o = CsvOptions {
        header: false,
        skip_rows: 1,
        empty_as_null: false,
        null_marker: String::new(),
        ..Default::default()
    };
    let ev = csv_events(text, &o);
    let names = vec!["column_1".to_string(), "column_2".into(), "column_3".into()];
    assert_eq!(ev[0], CsvEvent::Columns(names));
    assert_eq!(values(&ev[1]), vec![s("a"), s(""), s("c")]);
}

#[test]
fn csv_bad_rows_are_reported_not_fatal() {
    let o = CsvOptions::default();
    let ev = csv_events(b"a,b\n1,2,3\n\xff,x\n4,5\n", &o);
    let errors: Vec<String> = ev
        .iter()
        .filter_map(|e| match e {
            CsvEvent::Row(Record { values: Err(m), .. }) => Some(m.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(errors.len(), 2);
    assert!(errors[0].contains("3 fields"));
    assert!(errors[1].contains("UTF-8"));
    assert_eq!(values(&ev[3]), vec![s("4"), s("5")]);
    let latin = CsvOptions {
        encoding: Encoding::Latin1,
        ..Default::default()
    };
    assert_eq!(
        values(&csv_events(b"a\ncaf\xe9\n", &latin)[1]),
        vec![s("café")]
    );
}

#[test]
fn csv_header_names_are_cleaned() {
    let ev = csv_events(b"\xef\xbb\xbfname, ,name\n1,2,3\n", &CsvOptions::default());
    let want = vec!["name".to_string(), "column_2".into(), "name_2".into()];
    assert_eq!(ev[0], CsvEvent::Columns(want));
}

fn json_rows(text: &str) -> Vec<(u64, Result<serde_json::Value, String>)> {
    let mut out = Vec::new();
    source::read_json(Cursor::new(text.as_bytes().to_vec()), |row, obj| {
        out.push((row, obj.map(serde_json::Value::Object)));
        true
    })
    .unwrap();
    out
}

#[test]
fn json_array_and_ndjson() {
    let rows = json_rows(r#" [ {"a": 1}, 5, {"a": null, "b": {"x": [1]}} ] "#);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], (1, Ok(json!({"a": 1}))));
    assert!(rows[1].1.as_ref().unwrap_err().contains("a number"));
    let nd = json_rows("{\"a\": \"x\"}\n{\"a\": true}\n");
    assert_eq!(nd.len(), 2);
    let obj = json!({"a": null, "b": {"x": [1]}, "c": 2.5, "d": false});
    let cols = ["b", "a", "c", "d", "missing"].map(String::from);
    assert_eq!(
        source::json_values(obj.as_object().unwrap(), &cols),
        vec![s("{\"x\":[1]}"), None, s("2.5"), s("false"), None]
    );
}

#[test]
fn json_errors_and_early_stop() {
    let bad = source::read_json(Cursor::new(b"[{\"a\": 1}, {".to_vec()), |_, _| true);
    assert!(bad.unwrap_err().message.contains("not valid"));
    let mut seen = 0;
    let text = b"[{\"a\":1},{\"a\":2},{\"a\":3}]".to_vec();
    source::read_json(Cursor::new(text), |_, _| {
        seen += 1;
        seen < 2
    })
    .unwrap();
    assert_eq!(seen, 2);
}

#[test]
fn preview_infers_types_from_files() {
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("p.csv");
    std::fs::write(
        &csv,
        "id,price,when,name\n1,9.99,2024-01-02,a\n2,,2024-02-03,\n",
    )
    .unwrap();
    let spec = |path: &std::path::Path, format| SourceSpec {
        path: path.display().to_string(),
        format,
        csv: CsvOptions::default(),
    };
    let p = preview_file(&spec(&csv, FileFormat::Csv), Dialect::Postgres).unwrap();
    let kinds: Vec<Kind> = p.columns.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![Kind::Integer, Kind::Decimal, Kind::Date, Kind::Text]
    );
    assert_eq!(p.columns[1].suggested_type, "numeric(3,2)");
    assert!(p.columns[1].nullable && !p.columns[0].nullable);
    assert_eq!(p.rows.len(), 2);
    let json = dir.path().join("p.json");
    std::fs::write(
        &json,
        r#"[{"id": 1, "tags": ["a"]}, {"id": 2, "ok": true}]"#,
    )
    .unwrap();
    let p = preview_file(&spec(&json, FileFormat::Json), Dialect::Mysql).unwrap();
    let names: Vec<&str> = p.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["id", "tags", "ok"]);
    let types: Vec<&str> = p
        .columns
        .iter()
        .map(|c| c.suggested_type.as_str())
        .collect();
    assert_eq!(types, vec!["int", "json", "tinyint(1)"]);
}

fn pieces(sql: &str, driver: Dialect, split_at: usize) -> Vec<Piece> {
    StatementReader::with_split_at(Cursor::new(sql.as_bytes().to_vec()), driver, split_at)
        .map(Result::unwrap)
        .collect()
}

fn statements(p: &[Piece]) -> Vec<String> {
    p.iter()
        .filter_map(|p| match p {
            Piece::Statement { sql, .. } => Some(sql.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn sql_file_streams_like_a_whole_split() {
    let mut sql = String::from("-- dump\nCREATE TABLE t (a text);\n");
    for i in 0..200 {
        sql.push_str(&format!(
            "INSERT INTO t VALUES ('row {i}; ''quoted''\nnext');\n"
        ));
    }
    sql.push_str("CREATE FUNCTION f() RETURNS int AS $$ BEGIN RETURN 1; END $$ LANGUAGE plpgsql;");
    let whole: Vec<String> = split::split(Dialect::Postgres, &sql)
        .into_iter()
        .map(|s| s.text)
        .collect();
    for split_at in [1, 7, 64, 1 << 20] {
        assert_eq!(
            statements(&pieces(&sql, Dialect::Postgres, split_at)),
            whole
        );
    }
    let p = pieces(&sql, Dialect::Postgres, 16);
    let Piece::Statement { index, line, .. } = &p[2] else {
        panic!()
    };
    assert_eq!((*index, *line), (3, 5));
}

#[test]
fn sql_file_mysql_delimiters() {
    let sql = "/*!40101 SET NAMES utf8 */;\nDELIMITER ;;\n\
               CREATE TRIGGER t1 BEFORE INSERT ON x FOR EACH ROW BEGIN\n  SET NEW.a = 1;\nEND ;;\n\
               DELIMITER ;\nSELECT 1;\n";
    let st = statements(&pieces(sql, Dialect::Mysql, 8));
    assert_eq!(st.len(), 3);
    assert_eq!(st[0], "/*!40101 SET NAMES utf8 */");
    assert!(st[1].starts_with("CREATE TRIGGER") && st[1].ends_with("END"));
    assert_eq!(st[2], "SELECT 1");
}

#[test]
fn sql_file_postgres_copy_and_meta_commands() {
    let sql = "\\connect shop\nCREATE TABLE t (a int, b text);\n\
               COPY public.t (a, b) FROM stdin;\n1\tx;y\n2\t\\N\n\\.\nSELECT 2;\n";
    let p = pieces(sql, Dialect::Postgres, 4);
    assert_eq!(
        statements(&p),
        vec!["CREATE TABLE t (a int, b text)", "SELECT 2"]
    );
    let copy = p
        .iter()
        .position(|x| matches!(x, Piece::CopyStart { .. }))
        .unwrap();
    assert_eq!(
        p[copy],
        Piece::CopyStart {
            index: 2,
            line: 3,
            sql: "COPY public.t (a, b) FROM stdin".into()
        }
    );
    assert_eq!(p[copy + 1], Piece::CopyData(b"1\tx;y\n2\t\\N\n".to_vec()));
    assert_eq!(p[copy + 2], Piece::CopyEnd);
    let mut r = StatementReader::new(Cursor::new(sql.as_bytes().to_vec()), Dialect::Postgres);
    r.by_ref().for_each(drop);
    assert_eq!(r.skipped_commands, 1);
}
