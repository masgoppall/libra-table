//! The data scenario every driver runs: metadata, grid filters and paging, edits, the
//! query editor with cancel, and exports.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use libratable_lib::db::changes::{self, Change};
use libratable_lib::db::data;
use libratable_lib::db::exec;
use libratable_lib::db::filter::{SortSpec, ANY_COLUMN, RAW_SQL};
use libratable_lib::db::meta;
use libratable_lib::error::ErrorKind;
use libratable_lib::export::{self, ExportRequest, Format, Options, Source};
use libratable_lib::models::Dialect;
use libratable_lib::state::{CancelTarget, EditorConn, Session};
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;

use crate::common::{exec_sql, filter, names, page};
use crate::schema;

const PEOPLE_PG: &str = "CREATE TABLE people (id serial PRIMARY KEY, name text NOT NULL, \
        note text, score integer, tags text[], created timestamptz DEFAULT now());
    INSERT INTO people (name, note, score, tags) VALUES
        ('Alice', 'likes 50%_off deals', 10, '{a,b}'), ('Bob', NULL, 20, NULL),
        ('O''Brien', 'quote', 30, '{c}'), ('alice2', 'lowercase', 40, NULL),
        ('Eve', '50 percent off', 50, NULL);";

const PEOPLE_MYSQL: &str = "CREATE TABLE people (id int AUTO_INCREMENT PRIMARY KEY, \
        name varchar(100) NOT NULL, note text, score int, tags json, \
        created datetime DEFAULT CURRENT_TIMESTAMP);
    INSERT INTO people (name, note, score, tags) VALUES
        ('Alice', 'likes 50%_off deals', 10, '[\"a\",\"b\"]'), ('Bob', NULL, 20, NULL),
        ('O''Brien', 'quote', 30, '[\"c\"]'), ('alice2', 'lowercase', 40, NULL),
        ('Eve', '50 percent off', 50, NULL);";

const PEOPLE_SQLITE: &str = "CREATE TABLE people (id INTEGER PRIMARY KEY AUTOINCREMENT, \
        name TEXT NOT NULL, note TEXT, score INTEGER, tags TEXT, \
        created TEXT DEFAULT CURRENT_TIMESTAMP);
    CREATE INDEX people_name ON people(name);
    INSERT INTO people (name, note, score, tags) VALUES
        ('Alice', 'likes 50%_off deals', 10, 'a,b'), ('Bob', NULL, 20, NULL),
        ('O''Brien', 'quote', 30, 'c'), ('alice2', 'lowercase', 40, NULL),
        ('Eve', '50 percent off', 50, NULL);";

fn setup_sql(driver: Dialect) -> &'static str {
    match driver {
        Dialect::Postgres => PEOPLE_PG,
        Dialect::Mysql => PEOPLE_MYSQL,
        Dialect::Sqlite => PEOPLE_SQLITE,
    }
}

fn object(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// The same scenario for every driver; `sleep_sql` runs long enough to be cancelled.
pub async fn run(session: Arc<Session>, schema: &str, sleep_sql: &str) {
    exec_sql(&session.pool, setup_sql(session.dialect())).await;
    metadata(&session, schema).await;
    filters(&session, schema).await;
    paging(&session, schema).await;
    edits(&session, schema).await;
    {
        let mut slot = session.editor.lock().await;
        let target = exec::prepare(&session, &mut slot).await.unwrap();
        editor_script(&mut slot).await;
        cancel(&mut slot, target, sleep_sql, session.dialect()).await;
    }
    exports(&session, schema).await;
    schema::run(&session, schema).await;
    session.close().await;
}

/// SCH-1, SCH-2, SCH-5 and autocomplete names.
async fn metadata(session: &Session, schema: &str) {
    let schemas = meta::list_schemas(&session.pool, false).await.unwrap();
    assert!(
        schemas.iter().any(|s| s == schema),
        "{schemas:?} should contain {schema}"
    );
    let objects = meta::list_objects(&session.pool, schema).await.unwrap();
    assert!(objects
        .iter()
        .any(|o| o.name == "people" && o.kind == "table"));
    let cols = meta::list_columns(&session.pool, schema, "people")
        .await
        .unwrap();
    assert_eq!(meta::primary_key(&cols), vec!["id".to_string()]);
    assert!(!cols.iter().find(|c| c.name == "name").unwrap().nullable);
    let structure = meta::table_structure(&session.pool, schema, "people")
        .await
        .unwrap();
    assert!(
        structure.ddl.to_lowercase().contains("create table"),
        "ddl: {}",
        structure.ddl
    );
    assert!(
        structure.indexes.iter().any(|i| i.primary),
        "indexes: {:?}",
        structure.indexes
    );
    let completion = meta::completion(&session.pool, schema, false)
        .await
        .unwrap();
    assert_eq!(completion.tables.get("people").map(|c| c.len()), Some(6));
}

/// Every filter type (FLT-*): wildcards, quotes, injection attempts, lists, ranges,
/// Any column, Raw SQL and the driver's case operators.
async fn filters(session: &Session, schema: &str) {
    let only = |f| page(schema, vec![f]);
    assert_eq!(names(session, &page(schema, vec![])).await.len(), 5);

    let wildcard = names(session, &only(filter("note", "contains", "50%_off"))).await;
    assert_eq!(wildcard, vec!["Alice"], "%/_ must match literally");

    let quote = names(session, &only(filter("name", "eq", "O'Brien"))).await;
    assert_eq!(quote, vec!["O'Brien"]);

    let injection = names(session, &only(filter("name", "eq", "x' OR '1'='1"))).await;
    assert!(injection.is_empty(), "injection must stay a value");

    let listed = names(session, &only(filter("name", "in", "Bob, 'O''Brien', Eve"))).await;
    assert_eq!(
        listed.len(),
        3,
        "Bob, O'Brien (doubled quote inside quotes) and Eve => {listed:?}"
    );

    let mut between = filter("score", "between", "20");
    between.value2 = "40".into();
    let mut sorted = only(between);
    sorted.sort = vec![SortSpec {
        column: "score".into(),
        desc: true,
    }];
    assert_eq!(
        names(session, &sorted).await,
        vec!["alice2", "O'Brien", "Bob"]
    );

    let nulls = names(session, &only(filter("note", "is_null", ""))).await;
    assert_eq!(nulls, vec!["Bob"]);
    let any = names(session, &only(filter(ANY_COLUMN, "contains", "percent"))).await;
    assert_eq!(any, vec!["Eve"]);
    let none_contains = names(session, &only(filter(ANY_COLUMN, "not_contains", "e"))).await;
    assert_eq!(
        none_contains,
        vec!["Bob"],
        "Bob is the only row with no 'e' in any column => {none_contains:?}"
    );
    let raw = names(session, &only(filter(RAW_SQL, "", "score >= 40"))).await;
    assert_eq!(raw.len(), 2);

    let case_op = match session.dialect() {
        Dialect::Postgres => Some(("has_prefix_ci", 2usize)),
        Dialect::Mysql => Some(("has_prefix_cs", 1usize)),
        Dialect::Sqlite => None,
    };
    if let Some((op, expected)) = case_op {
        let found = names(session, &only(filter("name", op, "Alice"))).await;
        assert_eq!(found.len(), expected, "{op}");
    }
}

/// Paging, counts, and a Postgres array column read as text.
async fn paging(session: &Session, schema: &str) {
    let mut two = page(schema, vec![]);
    two.page_size = 2;
    two.page = 2;
    let last = data::fetch_page(session, &two).await.unwrap();
    assert_eq!((last.rows.len(), last.has_more), (1, false));
    let over_15 = page(schema, vec![filter("score", "gt", "15")]);
    let count = data::count_rows(session, &over_15).await.unwrap();
    assert_eq!((count.total, count.exact), (4, true));

    if session.dialect() == Dialect::Postgres {
        let alice = page(schema, vec![filter("name", "eq", "Alice")]);
        let p = data::fetch_page(session, &alice).await.unwrap();
        let tags = p.columns.iter().position(|c| c.name == "tags").unwrap();
        assert_eq!(p.rows[0][tags], json!("{a,b}"));
    }
}

/// Cell edits (GRD-4), and a commit that rolls back as a whole.
async fn edits(session: &Session, schema: &str) {
    let driver = session.dialect();
    let cols = session.columns(schema, "people").await.unwrap();
    let bob = page(schema, vec![filter("name", "eq", "Bob")]);
    let bob_id = {
        let p = data::fetch_page(session, &bob).await.unwrap();
        let id = p.columns.iter().position(|c| c.name == "id").unwrap();
        p.rows[0][id].clone()
    };
    let edits = vec![
        Change::Update {
            pk: object(json!({ "id": bob_id })),
            values: object(json!({ "note": "edited", "score": 21 })),
        },
        Change::Insert {
            values: object(json!({ "name": "Zed", "score": null })),
        },
    ];
    let builder = changes::Builder::new(driver, schema, "people", &cols);
    let stmts = builder.build(&edits).unwrap();
    assert_eq!(changes::apply(&session.pool, &stmts).await.unwrap(), 2);
    let edited = page(schema, vec![filter("note", "eq", "edited")]);
    assert_eq!(names(session, &edited).await, vec!["Bob"]);
    assert_eq!(names(session, &page(schema, vec![])).await.len(), 6);

    // A delete that matches no row rolls back the whole commit, including the update
    // before it.
    let bad = vec![
        Change::Update {
            pk: object(json!({ "id": bob_id })),
            values: object(json!({ "note": "should roll back" })),
        },
        Change::Delete {
            pk: object(json!({ "id": 999_999 })),
        },
    ];
    let stmts = builder.build(&bad).unwrap();
    assert!(changes::apply(&session.pool, &stmts).await.is_err());
    assert_eq!(names(session, &edited).await, vec!["Bob"], "rolled back");
}

/// QRY-3: statements run one by one, rows past the limit are counted, a run stops at
/// the first error.
async fn editor_script(slot: &mut Option<EditorConn>) {
    let flag = Arc::new(AtomicBool::new(false));
    let script = [
        "SELECT name FROM people WHERE score > 25 ORDER BY score",
        "UPDATE people SET score = score + 1 WHERE name = 'Eve'",
        "SELECT name FROM people WHERE 1 = 0",
        "SELECT * FROM table_that_does_not_exist",
        "SELECT 1",
    ]
    .map(String::from);
    let results = exec::run_locked(slot, &script, 2, &flag).await;
    assert_eq!(results.len(), 4, "stops at the first error");
    let first = &results[0];
    assert_eq!(
        (first.rows.len(), first.total_rows, first.truncated),
        (2, 3, true)
    );
    assert_eq!(results[1].rows_affected, Some(1));
    assert!(
        results[2].rows.is_empty() && results[2].columns.len() == 1,
        "empty SELECT still has its columns: {:?}",
        results[2].columns
    );
    assert!(results[3].error.is_some());
}

/// QRY-4: cancelling stops the query on the server.
async fn cancel(
    slot: &mut Option<EditorConn>,
    target: CancelTarget,
    sleep_sql: &str,
    driver: Dialect,
) {
    let flag = Arc::new(AtomicBool::new(false));
    let canceller = {
        let flag = flag.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            flag.store(true, Ordering::SeqCst);
            exec::cancel(&target, || true).await.unwrap();
        })
    };
    let started = Instant::now();
    let results = exec::run_locked(slot, &[sleep_sql.to_string()], 10, &flag).await;
    canceller.await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "cancel must stop the query early ({:?})",
        started.elapsed()
    );
    if driver != Dialect::Mysql {
        // MySQL's SLEEP() returns 1 instead of failing when killed.
        assert_eq!(
            results[0].error.as_ref().map(|e| e.kind),
            Some(ErrorKind::Cancelled)
        );
    }
}

/// EXP-*: a filtered table in every format, and a query result.
async fn exports(session: &Session, schema: &str) {
    let dir = tempfile::tempdir().unwrap();
    for (format, file) in [
        (Format::Csv, "out.csv"),
        (Format::Json, "out.json"),
        (Format::Sql, "out.sql"),
    ] {
        let path = dir.path().join(file);
        let req = ExportRequest {
            source: Source::Table {
                request: page(schema, vec![filter("score", "gte", "30")]),
                page_only: false,
            },
            format,
            path: path.display().to_string(),
            options: Options::default(),
        };
        let n = export::run(session, &req, &CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert_eq!(n, 3, "{file}");
        let text = std::fs::read_to_string(&path).unwrap();
        match format {
            Format::Csv => assert_eq!(text.lines().count(), 4, "{text}"),
            Format::Json => {
                let rows: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(rows.as_array().unwrap().len(), 3);
            }
            Format::Sql => assert_eq!(text.matches("INSERT INTO").count(), 3),
        }
    }
    let q = ExportRequest {
        source: Source::Query {
            sql: "SELECT name, score FROM people ORDER BY id".into(),
        },
        format: Format::Csv,
        path: dir.path().join("q.csv").display().to_string(),
        options: Options::default(),
    };
    let n = export::run(session, &q, &CancellationToken::new(), |_| {})
        .await
        .unwrap();
    assert_eq!(n, 6);
    assert!(std::fs::read_to_string(dir.path().join("q.csv"))
        .unwrap()
        .starts_with("name,score\n"));
}
