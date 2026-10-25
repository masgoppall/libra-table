//! What the HTTP cloud drivers (Snowflake, BigQuery, DynamoDB) share: the HTTPS client,
//! transport errors, endpoint overrides, and the grid SQL of the two SQL services.

pub mod edits;
pub mod sql;

use std::error::Error as _;
use std::time::Duration;

use base64::Engine as _;
use serde_json::Value;

use crate::db::cell::{sniff_mime, CellContent};
use crate::error::{AppError, AppResult, ErrorKind, Layer};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Long enough for a server that holds a query request open (Snowflake waits ~45 s).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// The HTTPS client of one engine; `max_connections` caps its idle keep-alive pool.
pub fn client(max_connections: u32) -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .pool_max_idle_per_host(max_connections.max(1) as usize)
        .user_agent(concat!("LibraTable/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::internal(format!("Can't set up HTTPS: {}", detail(&e))))
}

/// The error and its causes in one line, without the URL (which may name the account).
pub fn detail(e: &reqwest::Error) -> String {
    let mut out = if e.is_timeout() {
        "the request timed out".to_string()
    } else if e.is_connect() {
        "can't connect".to_string()
    } else {
        "the request failed".to_string()
    };
    let mut source = e.source();
    while let Some(s) = source {
        let text = s.to_string();
        if !out.contains(&text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        source = s.source();
    }
    out
}

/// A request that got no answer (DNS, TCP, TLS, timeout).
pub fn transport(e: reqwest::Error) -> AppError {
    AppError::unreachable(detail(&e), false)
}

/// A service answer that isn't the JSON we expect.
pub fn bad_answer(service: &str, what: impl std::fmt::Display) -> AppError {
    AppError::query(format!(
        "{service} sent an answer LibraTable can't read: {what}"
    ))
}

/// An endpoint override (`http(s)://host[:port][/path]`) without the trailing slash.
pub fn endpoint(raw: &str, field: &str) -> AppResult<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let ok = url::Url::parse(trimmed)
        .map(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
        .unwrap_or(false);
    if ok {
        Ok(trimmed.to_string())
    } else {
        Err(
            AppError::validation(format!("The {field} \"{trimmed}\" is not an http(s) URL."))
                .hint("Use a URL such as http://127.0.0.1:9050, or leave the field empty."),
        )
    }
}

/// Quick Look reads at most this much of one binary value.
pub const CELL_BYTES: usize = 16 * 1024 * 1024;

/// A whole cell (as the value rules encode it) for Quick Look.
pub fn cell_content(v: Value) -> CellContent {
    match v {
        Value::Null => CellContent::Null,
        Value::Object(o) if o.contains_key("$binary") => {
            let hex_text = o.get("$binary").and_then(Value::as_str).unwrap_or_default();
            let bytes = hex::decode(hex_text).unwrap_or_default();
            let size = o.get("size").and_then(Value::as_u64).unwrap_or(0) as usize;
            CellContent::Binary {
                mime: sniff_mime(&bytes),
                truncated: size > bytes.len(),
                base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
                size,
            }
        }
        Value::String(text) => CellContent::Text { text },
        other => CellContent::Text {
            text: serde_json::to_string_pretty(&other).unwrap_or_default(),
        },
    }
}

/// An UPDATE or DELETE that didn't touch exactly one row (same words as the built-ins).
pub fn not_one_row(change: usize, affected: u64) -> AppError {
    AppError::query(format!(
        "Change {change} affected {affected} rows instead of 1, so nothing was saved."
    ))
    .hint("The row may have been changed or deleted by someone else. Refresh and try again.")
}

/// A write on a read-only connection (the commands refuse these first).
pub fn read_only() -> AppError {
    AppError::new(
        ErrorKind::ReadOnly,
        Layer::App,
        "This connection is read-only, so it can't change data or structure.",
    )
    .hint("Turn off Read-only in the connection settings to make changes.")
}

/// A statement stopped by the user.
pub fn cancelled() -> AppError {
    AppError::new(
        ErrorKind::Cancelled,
        Layer::Database,
        "The query was cancelled.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quick_look_cells() {
        assert_eq!(cell_content(Value::Null), CellContent::Null);
        let b = cell_content(json!({ "$binary": "89504e470d0a1a0a", "size": 8 }));
        assert!(matches!(
            b,
            CellContent::Binary {
                mime: Some("image/png"),
                ..
            }
        ));
        let t = cell_content(json!({ "a": 1 }));
        assert_eq!(
            t,
            CellContent::Text {
                text: "{\n  \"a\": 1\n}".into()
            }
        );
    }

    #[test]
    fn endpoints_must_be_http_urls() {
        assert_eq!(
            endpoint(" http://127.0.0.1:9050/ ", "endpoint").unwrap(),
            "http://127.0.0.1:9050"
        );
        assert!(endpoint("127.0.0.1:9050", "endpoint").is_err());
        assert!(endpoint("ftp://x", "endpoint").is_err());
    }
}
