//! The scenario every driver runs.

use std::sync::Arc;

use libratable_lib::diagram;
use libratable_lib::export::tables::{self as export_tables, TablesRequest};
use libratable_lib::export::{Format, Options};
use libratable_lib::import::sqlrun::{self, SqlFileRequest};
use libratable_lib::import::{self, FileFormat, ImportSummary, OnError, TxScope};
use libratable_lib::metrics::{self, VarKind, Variable};
use libratable_lib::models::Driver;
use libratable_lib::state::Session;
use tokio_util::sync::CancellationToken;

use crate::files::{exec, int, request, tables_sql, text, write};

pub async fn run(s: Arc<Session>, schema: String) {
    let dir = tempfile::tempdir().unwrap();
    exec(&s.pool, tables_sql(s.driver())).await;
    import_csv(&s, &schema, dir.path()).await;
    import_json(&s, &schema, dir.path()).await;
    import_errors(&s, &schema, dir.path()).await;
    schema_diagram(&s, &schema).await;
    sql_file(&s, dir.path()).await;
    export_two_tables(&s, &schema, dir.path()).await;
    widget_queries(&s).await;
}

async fn import(s: &Session, req: &import::ImportRequest) -> ImportSummary {
    let token = CancellationToken::new();
    import::run(s, req, &token, |_| {})
        .await
        .expect("import runs")
}

async fn import_csv(s: &Session, schema: &str, dir: &std::path::Path) {
    let body = "Author ID;Full name;born\n1;Ann;1970-01-02\n2;\"Bob; Jr\";NULL\n3;Cy;\n";
    let path = write(dir, "authors.csv", body);
    let mut req = request(
        &path,
        FileFormat::Csv,
        schema,
        "authors",
        &["id", "name", "born"],
    );
    req.source.csv.delimiter = ";".into();
    let sum = import(s, &req).await;
    assert_eq!(
        (sum.inserted, sum.skipped, sum.rows_read),
        (3, 0, 3),
        "{sum:?}"
    );
    assert_eq!(
        int(&s.pool, "SELECT count(*) FROM authors WHERE born IS NULL").await,
        2
    );
    let bob = text(&s.pool, "SELECT name FROM authors WHERE id = 2").await;
    assert_eq!(bob.as_deref(), Some("Bob; Jr"));
}

async fn import_json(s: &Session, schema: &str, dir: &std::path::Path) {
    let body = r#"[
        {"id": 1, "author_id": 1, "title": "A", "price": 9.5, "in_stock": true, "tags": ["x"]},
        {"id": 2, "author_id": 2, "title": "B \"q\"", "price": null, "in_stock": false,
         "tags": {"k": 1}},
        {"id": 3, "author_id": 1, "title": "C"}
    ]"#;
    let path = write(dir, "books.json", body);
    let cols = ["id", "author_id", "title", "price", "in_stock", "tags"];
    let sum = import(s, &request(&path, FileFormat::Json, schema, "books", &cols)).await;
    assert_eq!(sum.inserted, 3, "{sum:?}");
    let in_stock = "SELECT count(*) FROM books WHERE in_stock = true";
    assert_eq!(int(&s.pool, in_stock).await, 1);
    let price = "SELECT count(*) FROM books WHERE price = 9.5 AND id = 1";
    assert_eq!(int(&s.pool, price).await, 1);
    let title = text(&s.pool, "SELECT title FROM books WHERE id = 2").await;
    assert_eq!(title.as_deref(), Some("B \"q\""));
}

/// Skip vs stop, per-run vs per-batch transactions, cancel.
async fn import_errors(s: &Session, schema: &str, dir: &std::path::Path) {
    let cols = ["id", "author_id", "title"];
    // Row 2 has no title (NOT NULL), row 3 repeats an existing id.
    let bad = write(
        dir,
        "bad.csv",
        "id,author_id,title\n10,1,ok\n11,1,\n1,1,dup\n13,1,ok\n",
    );
    let mut req = request(&bad, FileFormat::Csv, schema, "books", &cols);
    req.on_error = OnError::Skip;
    let sum = import(s, &req).await;
    assert_eq!((sum.inserted, sum.skipped), (2, 2), "{sum:?}");
    let rows: Vec<u64> = sum.errors.iter().map(|e| e.row).collect();
    assert_eq!(rows, vec![2, 3]);
    assert_eq!(sum.errors[0].line, Some(3));

    let stop = write(
        dir,
        "stop.csv",
        "id,author_id,title\n20,1,a\n21,1,\n22,1,c\n",
    );
    let sum = import(s, &request(&stop, FileFormat::Csv, schema, "books", &cols)).await;
    assert!(
        sum.stopped && sum.rolled_back && sum.inserted == 0,
        "{sum:?}"
    );
    assert_eq!(
        int(&s.pool, "SELECT count(*) FROM books WHERE id >= 20").await,
        0
    );

    let batch = write(
        dir,
        "batch.csv",
        "id,author_id,title\n30,1,a\n31,1,b\n32,1,\n33,1,d\n",
    );
    let mut req = request(&batch, FileFormat::Csv, schema, "books", &cols);
    (req.transaction, req.batch_size) = (TxScope::Batch, 1);
    let sum = import(s, &req).await;
    assert!(sum.stopped && sum.inserted == 2, "{sum:?}");
    assert_eq!(
        int(&s.pool, "SELECT count(*) FROM books WHERE id >= 30").await,
        2
    );

    let token = CancellationToken::new();
    token.cancel();
    let req = request(&batch, FileFormat::Csv, schema, "books", &cols);
    let sum = import::run(s, &req, &token, |_| {}).await.unwrap();
    assert!(sum.cancelled && sum.inserted == 0, "{sum:?}");
}

async fn schema_diagram(s: &Session, schema: &str) {
    let d = diagram::schema_diagram(&s.pool, schema).await.unwrap();
    let names: Vec<&str> = d.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["authors", "books"]);
    let authors = &d.tables[0];
    assert!(authors.columns[0].primary_key && !authors.columns[1].primary_key);
    assert!(!authors.columns[1].nullable && authors.columns[2].nullable);
    let fk = &d.tables[1].foreign_keys[0];
    assert_eq!(
        (fk.ref_table.as_str(), fk.columns.clone()),
        ("authors", vec!["author_id".into()])
    );
    assert_eq!(fk.ref_columns, vec!["id".to_string()]);
}

fn sql_script(driver: Driver) -> String {
    let mut sql = String::new();
    if driver == Driver::Mysql {
        sql.push_str("/*!40101 SET NAMES utf8mb4 */;\n");
    }
    sql.push_str(
        "CREATE TABLE notes (id integer PRIMARY KEY, body text);\n\
         INSERT INTO notes VALUES (1, 'a;b');\n\
         INSERT INTO notes VALUES (1, 'dup');\n\
         INSERT INTO notes VALUES (2, 'c');\n",
    );
    match driver {
        Driver::Postgres => sql.push_str("COPY notes (id, body) FROM stdin;\n3\tcopied\n\\.\n"),
        Driver::Mysql => sql.push_str(
            "DELIMITER ;;\nCREATE TRIGGER notes_bi BEFORE INSERT ON notes FOR EACH ROW \
             BEGIN\n  SET NEW.body = UPPER(NEW.body);\nEND ;;\nDELIMITER ;\n\
             INSERT INTO notes VALUES (3, 'low');\n",
        ),
        Driver::Sqlite => sql.push_str("INSERT INTO notes VALUES (3, 'x');\n"),
    }
    sql
}

async fn sql_file(s: &Session, dir: &std::path::Path) {
    let path = write(dir, "script.sql", &sql_script(s.driver()));
    let req = SqlFileRequest {
        path: path.display().to_string(),
        stop_on_error: false,
    };
    let token = CancellationToken::new();
    let sum = sqlrun::run(s, &req, &token, |_| {}).await.unwrap();
    assert_eq!(sum.failed, 1, "{sum:?}");
    let offset = u64::from(s.driver() == Driver::Mysql);
    assert_eq!(
        (sum.errors[0].index, sum.errors[0].line),
        (3 + offset, 3 + offset)
    );
    assert_eq!(int(&s.pool, "SELECT count(*) FROM notes").await, 3);
    if s.driver() == Driver::Mysql {
        let body = text(&s.pool, "SELECT body FROM notes WHERE id = 3").await;
        assert_eq!(body.as_deref(), Some("LOW"));
    }
    let broken = write(dir, "broken.sql", "SELECT 1;\nSELEC oops;\nSELECT 2;\n");
    let req = SqlFileRequest {
        path: broken.display().to_string(),
        stop_on_error: true,
    };
    let sum = sqlrun::run(s, &req, &token, |_| {}).await.unwrap();
    assert!(
        sum.stopped && sum.statements == 2 && sum.failed == 1,
        "{sum:?}"
    );
}

async fn export_two_tables(s: &Session, schema: &str, dir: &std::path::Path) {
    let out = dir.join("export");
    std::fs::create_dir(&out).unwrap();
    let req = TablesRequest {
        schema: schema.into(),
        tables: vec!["authors".into(), "books".into()],
        format: Format::Csv,
        folder: out.display().to_string(),
        options: Options::default(),
    };
    let token = CancellationToken::new();
    let p = export_tables::run(s, &req, &token, |_| {}).await.unwrap();
    // 3 authors; 3 books from JSON, 2 skipped-mode and 2 batch-mode rows.
    assert_eq!((p.files.len(), p.total_rows), (2, 10));
    let authors = std::fs::read_to_string(out.join("authors.csv")).unwrap();
    assert_eq!(authors.lines().count(), 4);
}

async fn widget_queries(s: &Session) {
    let d = s.driver();
    let run = |sql: &str| metrics::read_statements(d, sql);
    let r = metrics::run_widget_query(s, run("SELECT count(*) AS n FROM authors").unwrap())
        .await
        .unwrap();
    assert_eq!(r.rows[0][0], serde_json::json!(3));
    assert!(run("DELETE FROM authors").is_err());
    let vars = [
        Variable {
            name: "id".into(),
            value: "2".into(),
            kind: VarKind::Number,
        },
        Variable {
            name: "name".into(),
            value: "x' OR '1'='1".into(),
            kind: VarKind::Text,
        },
    ];
    let sql = "SELECT count(*) FROM authors WHERE id = {{id}} OR name = {{name}}";
    let filled = metrics::substitute(d, sql, &vars).unwrap();
    let r = metrics::run_widget_query(s, run(&filled).unwrap())
        .await
        .unwrap();
    assert_eq!(r.rows[0][0], serde_json::json!(1));
    // The editor's own session is not used, and nothing stays read-only afterwards.
    exec(
        &s.pool,
        "INSERT INTO authors (id, name) VALUES (9, 'after')",
    )
    .await;
}
