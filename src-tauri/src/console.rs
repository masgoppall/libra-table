//! Console log (TablePlus "Console"): every statement the app sends — editor runs, grid
//! pages, catalog reads — as sqlx reports it. Kept in memory only, with password literals
//! masked, and streamed to the UI as `console-log` events.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::field::{Field, Visit};
use tracing::subscriber::Interest;
use tracing::{Event, Level, Metadata};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

use crate::state::now_ms;

const TARGET: &str = "sqlx::query";
const MAX_ENTRIES: usize = 2000;
const MAX_SQL_CHARS: usize = 20_000;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleEntry {
    pub at: i64,
    pub sql: String,
    pub rows_affected: u64,
    pub rows_returned: u64,
    pub elapsed_ms: f64,
    /// Slower than sqlx's alert threshold.
    pub slow: bool,
}

static ENTRIES: Mutex<VecDeque<ConsoleEntry>> = Mutex::new(VecDeque::new());
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Installs the collector; call once at startup.
pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let subscriber = tracing_subscriber::registry().with(ConsoleLayer);
    let _ = tracing::subscriber::set_global_default(subscriber);
}

pub fn entries() -> Vec<ConsoleEntry> {
    ENTRIES.lock().unwrap().iter().cloned().collect()
}

pub fn clear() {
    ENTRIES.lock().unwrap().clear();
}

struct ConsoleLayer;

impl<S: tracing::Subscriber> Layer<S> for ConsoleLayer {
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> Interest {
        if meta.target() == TARGET {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, meta: &Metadata<'_>, _: Context<'_, S>) -> bool {
        meta.target() == TARGET
    }

    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let entry = fields.into_entry(*event.metadata().level() <= Level::WARN);
        if let Some(app) = APP.get() {
            let _ = app.emit("console-log", &entry);
        }
        let mut list = ENTRIES.lock().unwrap();
        if list.len() == MAX_ENTRIES {
            list.pop_front();
        }
        list.push_back(entry);
    }
}

#[derive(Default)]
struct Fields {
    summary: String,
    statement: String,
    rows_affected: u64,
    rows_returned: u64,
    elapsed_secs: f64,
}

impl Fields {
    fn into_entry(self, slow: bool) -> ConsoleEntry {
        // `db.statement` holds the whole SQL when the summary had to shorten it.
        let sql = if self.statement.trim().is_empty() {
            self.summary
        } else {
            self.statement.trim().to_string()
        };
        ConsoleEntry {
            at: now_ms(),
            sql: mask_secrets(&truncate(sql)),
            rows_affected: self.rows_affected,
            rows_returned: self.rows_returned,
            elapsed_ms: self.elapsed_secs * 1000.0,
            slow,
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "summary" => self.summary = value.to_string(),
            "db.statement" => self.statement = value.to_string(),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "rows_affected" => self.rows_affected = value,
            "rows_returned" => self.rows_returned = value,
            _ => {}
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() == "elapsed_secs" {
            self.elapsed_secs = value;
        }
    }

    fn record_debug(&mut self, _: &Field, _: &dyn Debug) {}
}

fn truncate(sql: String) -> String {
    match sql.char_indices().nth(MAX_SQL_CHARS) {
        Some((at, _)) => format!("{}…", &sql[..at]),
        None => sql,
    }
}

/// `PASSWORD '…'` and `IDENTIFIED BY '…'` literals become `'***'`.
pub fn mask_secrets(sql: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < sql.len() {
        let rest = &lower[i..];
        let keyword = ["password", "identified by"]
            .iter()
            .find(|k| rest.starts_with(**k));
        let Some(k) = keyword else {
            let ch = sql[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
            continue;
        };
        out.push_str(&sql[i..i + k.len()]);
        i += k.len();
        let spaces = sql[i..].len() - sql[i..].trim_start().len();
        out.push_str(&sql[i..i + spaces]);
        i += spaces;
        if sql[i..].starts_with('\'') {
            out.push_str("'***'");
            i += quoted_len(&sql[i..]);
        }
    }
    out
}

/// Length of a single-quoted literal at the start of `s` ('' is an escaped quote).
fn quoted_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if bytes.get(i + 1) == Some(&b'\'') {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    s.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_password_literals() {
        assert_eq!(
            mask_secrets("ALTER ROLE app PASSWORD 'it''s secret' VALID UNTIL 'x'"),
            "ALTER ROLE app PASSWORD '***' VALID UNTIL 'x'"
        );
        assert_eq!(
            mask_secrets("create user u identified by 'pw'"),
            "create user u identified by '***'"
        );
        assert_eq!(
            mask_secrets("SELECT password FROM users"),
            "SELECT password FROM users"
        );
    }

    #[test]
    fn keeps_whole_sql_when_summarized() {
        let f = Fields {
            summary: "SELECT a …".into(),
            statement: "\n\nSELECT a, b FROM t\n".into(),
            ..Default::default()
        };
        assert_eq!(f.into_entry(false).sql, "SELECT a, b FROM t");
    }
}
