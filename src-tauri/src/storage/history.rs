//! Query history in a local SQLite database with an FTS5 index (QRY-5, QRY-6).
//! One row per execution: the editor run, a grid commit, or an export query.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;
use sqlx::Row;

use crate::error::AppResult;

pub struct History {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub connection_id: String,
    pub connection_name: String,
    pub database: String,
    pub sql: String,
    pub started_at: i64,
    pub duration_ms: i64,
    pub row_count: Option<i64>,
    /// "ok" | "error" | "cancelled"
    pub status: String,
    pub error: Option<String>,
}

pub struct NewEntry<'a> {
    pub connection_id: &'a str,
    pub connection_name: &'a str,
    pub database: &'a str,
    pub sql: &'a str,
    pub started_at: i64,
    pub duration_ms: i64,
    pub row_count: Option<i64>,
    pub status: &'a str,
    pub error: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryQuery {
    pub text: String,
    pub connection_id: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub limit: u32,
    pub offset: u32,
}

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS history (
        id INTEGER PRIMARY KEY,
        connection_id TEXT NOT NULL,
        connection_name TEXT NOT NULL,
        database TEXT NOT NULL DEFAULT '',
        sql TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        duration_ms INTEGER NOT NULL,
        row_count INTEGER,
        status TEXT NOT NULL,
        error TEXT
    )",
    "CREATE INDEX IF NOT EXISTS history_conn_time ON history(connection_id, started_at DESC)",
    "CREATE INDEX IF NOT EXISTS history_time ON history(started_at DESC)",
    "CREATE VIRTUAL TABLE IF NOT EXISTS history_fts USING fts5(
        sql, content='history', content_rowid='id', tokenize='unicode61'
    )",
    "CREATE TRIGGER IF NOT EXISTS history_ai AFTER INSERT ON history BEGIN
        INSERT INTO history_fts(rowid, sql) VALUES (new.id, new.sql);
     END",
    "CREATE TRIGGER IF NOT EXISTS history_ad AFTER DELETE ON history BEGIN
        INSERT INTO history_fts(history_fts, rowid, sql) VALUES ('delete', old.id, old.sql);
     END",
];

impl History {
    pub async fn open(path: &Path) -> AppResult<Self> {
        // filename() instead of a URL: "Application Support" has a space in it.
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            // The app's own bookkeeping stays out of the console log.
            .disable_statement_logging();
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(opts)
            .await?;
        Self::init(pool).await
    }

    pub async fn in_memory() -> AppResult<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        Self::init(pool).await
    }

    async fn init(pool: SqlitePool) -> AppResult<Self> {
        for stmt in SCHEMA {
            sqlx::query(stmt).execute(&pool).await?;
        }
        Ok(Self { pool })
    }

    pub async fn add(&self, e: NewEntry<'_>) -> AppResult<i64> {
        let id = sqlx::query(
            "INSERT INTO history (connection_id, connection_name, database, sql, started_at,
                                  duration_ms, row_count, status, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(e.connection_id)
        .bind(e.connection_name)
        .bind(e.database)
        .bind(e.sql)
        .bind(e.started_at)
        .bind(e.duration_ms)
        .bind(e.row_count)
        .bind(e.status)
        .bind(e.error)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    pub async fn search(&self, q: &HistoryQuery) -> AppResult<Vec<HistoryEntry>> {
        let limit = if q.limit == 0 { 200 } else { q.limit.min(1000) } as i64;
        let fts = fts_query(&q.text);
        let mut sql = String::from(
            "SELECT h.id, h.connection_id, h.connection_name, h.database, h.sql, h.started_at, \
             h.duration_ms, h.row_count, h.status, h.error FROM history h",
        );
        let mut conds: Vec<&str> = Vec::new();
        if fts.is_some() {
            sql.push_str(" JOIN history_fts f ON f.rowid = h.id");
            conds.push("history_fts MATCH ?1");
        }
        if q.connection_id.is_some() {
            conds.push("h.connection_id = ?2");
        }
        if q.from.is_some() {
            conds.push("h.started_at >= ?3");
        }
        if q.to.is_some() {
            conds.push("h.started_at <= ?4");
        }
        if !conds.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conds.join(" AND "));
        }
        sql.push_str(" ORDER BY h.started_at DESC, h.id DESC LIMIT ?5 OFFSET ?6");
        let rows = sqlx::query(&sql)
            .bind(fts.unwrap_or_default())
            .bind(q.connection_id.clone().unwrap_or_default())
            .bind(q.from.unwrap_or(0))
            .bind(q.to.unwrap_or(0))
            .bind(limit)
            .bind(q.offset as i64)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| HistoryEntry {
                id: r.get(0),
                connection_id: r.get(1),
                connection_name: r.get(2),
                database: r.get(3),
                sql: r.get(4),
                started_at: r.get(5),
                duration_ms: r.get(6),
                row_count: r.get(7),
                status: r.get(8),
                error: r.get(9),
            })
            .collect())
    }

    pub async fn delete(&self, ids: &[i64]) -> AppResult<u64> {
        let mut n = 0;
        for id in ids {
            n += sqlx::query("DELETE FROM history WHERE id = ?1")
                .bind(id)
                .execute(&self.pool)
                .await?
                .rows_affected();
        }
        Ok(n)
    }

    /// "Delete older than…" and the retention setting (QRY-6).
    pub async fn purge_older_than(&self, cutoff_ms: i64) -> AppResult<u64> {
        Ok(sqlx::query("DELETE FROM history WHERE started_at < ?1")
            .bind(cutoff_ms)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    pub async fn delete_connection(&self, connection_id: &str) -> AppResult<u64> {
        Ok(sqlx::query("DELETE FROM history WHERE connection_id = ?1")
            .bind(connection_id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }
}

/// Turns free text into a safe FTS5 query: every word becomes a quoted prefix term,
/// so punctuation in SQL (`*`, `"`, `-`, `:`) can't break the MATCH syntax.
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(conn: &'a str, sql: &'a str, at: i64) -> NewEntry<'a> {
        NewEntry {
            connection_id: conn,
            connection_name: "Local",
            database: "app",
            sql,
            started_at: at,
            duration_ms: 3,
            row_count: Some(1),
            status: "ok",
            error: None,
        }
    }

    #[tokio::test]
    async fn search_filters_and_purge() {
        let h = History::in_memory().await.unwrap();
        h.add(entry(
            "a",
            "SELECT * FROM invoices WHERE paid = true",
            1_000,
        ))
        .await
        .unwrap();
        h.add(entry("a", "UPDATE users SET name = 'x'", 2_000))
            .await
            .unwrap();
        h.add(entry("b", "select count(*) from invoice_lines", 3_000))
            .await
            .unwrap();

        let all = h.search(&HistoryQuery::default()).await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].started_at, 3_000, "newest first");

        let inv = h
            .search(&HistoryQuery {
                text: "invoice".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(inv.len(), 2, "prefix match on invoices / invoice_lines");

        let a_only = h
            .search(&HistoryQuery {
                text: "invoice".into(),
                connection_id: Some("a".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(a_only.len(), 1);

        let weird = h
            .search(&HistoryQuery {
                text: "count(*) \"".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(weird.len(), 1);

        assert_eq!(h.purge_older_than(2_500).await.unwrap(), 2);
        assert_eq!(
            h.search(&HistoryQuery {
                text: "users".into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .len(),
            0
        );
        assert_eq!(h.delete_connection("b").await.unwrap(), 1);
    }

    #[test]
    fn fts_query_quotes_terms() {
        assert_eq!(
            fts_query("select * from \"x\""),
            Some("\"select\"* \"from\"* \"x\"*".into())
        );
        assert_eq!(fts_query("  ;; "), None);
    }
}
