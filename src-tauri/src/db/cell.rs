//! One whole cell for Quick Look: the grid only previews binary values (4 KiB), so the
//! viewer fetches the full value of one row by its primary key.

use base64::Engine;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::db::changes::Builder;
use crate::db::data::select_list;
use crate::db::sql::qualified;
use crate::db::{bind_all, value, DbPool};
use crate::error::{AppError, AppResult};
use crate::state::Session;

/// Larger values are cut here: they would only clog the IPC channel.
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CellContent {
    Null,
    Text {
        text: String,
    },
    Binary {
        base64: String,
        size: usize,
        truncated: bool,
        /// Sniffed from the first bytes: images and PDF.
        mime: Option<&'static str>,
    },
}

pub async fn fetch_cell(
    session: &Session,
    schema: &str,
    table: &str,
    pk: &Map<String, Value>,
    column: &str,
) -> AppResult<CellContent> {
    let driver = session.dialect();
    let cols = session.columns(schema, table).await?;
    let col = cols
        .iter()
        .find(|c| c.name == column)
        .ok_or_else(|| AppError::validation(format!("Unknown column \"{column}\".")))?;
    let (condition, binds) = Builder::new(driver, schema, table, &cols).pk_condition(pk)?;
    let sql = format!(
        "SELECT {} FROM {} WHERE {condition}",
        select_list(driver, std::slice::from_ref(col)),
        qualified(driver, schema, table)
    );
    let cell = fetch_one(&session.pool, &sql, &binds).await?;
    content(cell)
}

async fn fetch_one(pool: &DbPool, sql: &str, binds: &[Option<String>]) -> AppResult<Value> {
    macro_rules! one {
        ($p:expr, $decode:path) => {
            bind_all!(sqlx::query(sql), binds)
                .fetch_optional($p)
                .await?
                .map(|r| $decode(&r, MAX_BYTES))
        };
    }
    let row = match pool {
        DbPool::Postgres(p) => one!(p, value::pg::row),
        DbPool::Mysql(p) => one!(p, value::mysql::row),
        DbPool::Sqlite(p) => one!(p, value::sqlite::row),
        DbPool::Engine(_) => return Err(crate::db::engine::sqlx_only()),
    };
    let row = row.ok_or_else(|| {
        AppError::validation("The row no longer exists.").hint("Refresh the table.")
    })?;
    Ok(row.into_iter().next().unwrap_or(Value::Null))
}

fn content(cell: Value) -> AppResult<CellContent> {
    Ok(match cell {
        Value::Null => CellContent::Null,
        Value::Object(o) if o.contains_key("$binary") => {
            let hex_text = o.get("$binary").and_then(Value::as_str).unwrap_or_default();
            let bytes = hex::decode(hex_text)
                .map_err(|_| AppError::internal("Binary value could not be decoded."))?;
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
            text: other.to_string(),
        },
    })
}

/// Image and PDF types by their magic bytes.
pub fn sniff_mime(b: &[u8]) -> Option<&'static str> {
    let starts = |sig: &[u8]| b.starts_with(sig);
    if starts(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if starts(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        Some("image/gif")
    } else if b.len() > 12 && starts(b"RIFF") && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if starts(b"%PDF-") {
        Some("application/pdf")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sniffs_common_images() {
        assert_eq!(sniff_mime(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff_mime(b"\xff\xd8\xff\xe0"), Some("image/jpeg"));
        assert_eq!(sniff_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_mime(b"plain"), None);
    }

    #[test]
    fn binary_cells_become_base64() {
        let c = content(json!({ "$binary": "89504e470d0a1a0a", "size": 8 })).unwrap();
        assert_eq!(
            c,
            CellContent::Binary {
                base64: "iVBORw0KGgo=".into(),
                size: 8,
                truncated: false,
                mime: Some("image/png"),
            }
        );
        assert_eq!(
            content(json!("hi")).unwrap(),
            CellContent::Text { text: "hi".into() }
        );
    }
}
