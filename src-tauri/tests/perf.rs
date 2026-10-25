//! PRD acceptance checks that take a while; run with:
//!   cargo test --release --test perf -- --ignored --nocapture

use std::time::Instant;

use libratable_lib::db::data::PageRequest;
use libratable_lib::db::open_pool;
use libratable_lib::export::{self, ExportRequest, Format, Options, Source};
use libratable_lib::models::Connection;
use libratable_lib::state::Session;
use libratable_lib::storage::history::{History, HistoryQuery, NewEntry};
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
        / 1024.0
}

/// QRY criterion: one word out of 100,000 history entries in < 200 ms.
#[tokio::test]
#[ignore]
async fn history_search_100k() {
    let dir = tempfile::tempdir().unwrap();
    let h = History::open(&dir.path().join("history.db")).await.unwrap();
    let words = [
        "invoices",
        "users",
        "orders",
        "payments",
        "shipments",
        "audit_log",
        "sessions",
        "products",
    ];
    for i in 0..100_000u32 {
        let sql = format!(
            "SELECT * FROM {} WHERE id = {i} AND status = 'open'",
            words[(i as usize) % words.len()]
        );
        h.add(NewEntry {
            connection_id: "c1",
            connection_name: "Perf",
            database: "app",
            sql: &sql,
            started_at: i as i64,
            duration_ms: 1,
            row_count: Some(1),
            status: "ok",
            error: None,
        })
        .await
        .unwrap();
    }
    // Warm up once, then time.
    h.search(&HistoryQuery {
        text: "payments".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let t = Instant::now();
    let hits = h
        .search(&HistoryQuery {
            text: "shipments".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let ms = t.elapsed().as_millis();
    println!(
        "history search over 100k entries: {ms} ms, {} hits (page of 200)",
        hits.len()
    );
    assert_eq!(hits.len(), 200);
    assert!(ms < 200, "search took {ms} ms");
}

/// EXP criterion: exporting 5,000,000 rows to CSV raises process memory by < 200 MB.
#[tokio::test]
#[ignore]
async fn export_5m_rows_flat_memory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.db");
    std::fs::File::create(&path).unwrap();
    let conn: Connection = serde_json::from_value(json!({
        "id": "perf", "name": "Perf", "driver": "sqlite",
        "db": { "filePath": path.display().to_string() }
    }))
    .unwrap();
    let pool = open_pool(&conn, None, None, None, 2).await.unwrap();
    if let libratable_lib::db::DbPool::Sqlite(p) = &pool {
        sqlx::raw_sql(
            "CREATE TABLE big (id INTEGER PRIMARY KEY, name TEXT, amount REAL, note TEXT);
             WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < 5000000)
             INSERT INTO big SELECT i, 'customer ' || i, i * 1.25,
                 CASE WHEN i % 7 = 0 THEN NULL ELSE 'note for row ' || i END FROM s;",
        )
        .execute(p)
        .await
        .unwrap();
    }
    let session = Session::new(conn, pool, None, "main".into(), String::new(), None);
    let before = rss_mb();
    let req = ExportRequest {
        source: Source::Table {
            request: PageRequest {
                schema: "main".into(),
                table: "big".into(),
                page: 0,
                page_size: 0,
                sort: vec![],
                filters: vec![],
                apply_only: None,
            },
            page_only: false,
        },
        format: Format::Csv,
        path: dir.path().join("big.csv").display().to_string(),
        options: Options::default(),
    };
    let peak = std::sync::Mutex::new(before);
    let t = Instant::now();
    let rows = export::run(&session, &req, &CancellationToken::new(), |n| {
        if n % 500_000 == 0 {
            let m = rss_mb();
            let mut p = peak.lock().unwrap();
            if m > *p {
                *p = m;
            }
        }
    })
    .await
    .unwrap();
    let peak = *peak.lock().unwrap();
    let size = std::fs::metadata(dir.path().join("big.csv")).unwrap().len() as f64 / 1e6;
    println!(
        "exported {rows} rows ({size:.0} MB CSV) in {:.1}s; \
         RSS before {before:.0} MB, peak {peak:.0} MB (+{:.0} MB)",
        t.elapsed().as_secs_f64(),
        peak - before
    );
    assert_eq!(rows, 5_000_000);
    assert!(
        peak - before < 200.0,
        "memory grew by {:.0} MB",
        peak - before
    );
}
