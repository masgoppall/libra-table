//! Reading one key for the value panel, a page at a time for big collections: hashes
//! and sets page by HSCAN/SSCAN cursor, lists and sorted sets by index, streams by
//! entry id. Quick Look reads a whole key (up to a cap) as text or JSON.

use base64::Engine as _;
use redis::aio::ConnectionManager;
use redis::Value as Reply;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::db::cell::CellContent;
use crate::error::{AppError, AppResult};

use super::conn::command_error;
use super::keys::replies;
use super::values::{self, int, items, reply_bytes, scalar_text};

/// Elements per page when the UI doesn't say.
const DEFAULT_COUNT: u32 = 200;
const MAX_COUNT: u32 = 2000;
/// Bytes of a string value sent to the panel.
const MAX_STRING: usize = 4 * 1024 * 1024;
/// Elements Quick Look shows of a collection.
const CELL_ITEMS: usize = 10_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadRequest {
    pub key: Value,
    /// Where the page starts: "" for the first page, else `next` of the previous one.
    #[serde(default)]
    pub cursor: String,
    #[serde(default)]
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyDetail {
    pub key: Value,
    #[serde(rename = "type")]
    pub key_type: String,
    pub ttl: i64,
    pub size: Option<i64>,
    pub encoding: Option<String>,
    pub value: KeyData,
    /// Cursor of the next page; "" when this was the last.
    pub next: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeyData {
    /// `text` for UTF-8, else a hex preview in `binary`.
    String {
        text: Option<String>,
        binary: Option<Value>,
        truncated: bool,
    },
    Hash {
        entries: Vec<Value>,
    },
    List {
        items: Vec<Value>,
    },
    Set {
        members: Vec<Value>,
    },
    Zset {
        members: Vec<Value>,
    },
    Stream {
        entries: Vec<Value>,
    },
    /// Module types (JSON, time series…): use the query editor.
    Other,
    Missing,
}

fn count_of(req: &ReadRequest) -> u32 {
    match req.count {
        0 => DEFAULT_COUNT,
        n => n.min(MAX_COUNT),
    }
}

/// TYPE, TTL, size and encoding of `key`.
async fn meta(conn: &mut ConnectionManager, key: &[u8]) -> AppResult<(String, i64, String)> {
    let mut pipe = redis::pipe();
    pipe.cmd("TYPE").arg(key).cmd("TTL").arg(key);
    pipe.cmd("OBJECT").arg("ENCODING").arg(key);
    let r = replies(conn, &pipe, 3).await?;
    let t = scalar_text(&r[0]).unwrap_or_else(|| "none".into());
    let enc = scalar_text(&r[2]).unwrap_or_default();
    Ok((t, int(&r[1]).unwrap_or(-1), enc))
}

fn size_cmd(key_type: &str) -> Option<&'static str> {
    Some(match key_type {
        "string" => "STRLEN",
        "hash" => "HLEN",
        "list" => "LLEN",
        "set" => "SCARD",
        "zset" => "ZCARD",
        "stream" => "XLEN",
        _ => return None,
    })
}

async fn query(conn: &mut ConnectionManager, cmd: &redis::Cmd) -> AppResult<Reply> {
    cmd.query_async(conn).await.map_err(command_error)
}

/// One page of `key` for the value panel.
pub async fn read_key(conn: &mut ConnectionManager, req: &ReadRequest) -> AppResult<KeyDetail> {
    let key = values::key_bytes(&req.key)?;
    let (key_type, ttl, encoding) = meta(conn, &key).await?;
    let size = match size_cmd(&key_type) {
        Some(c) => int(&query(conn, redis::cmd(c).arg(&key)).await?),
        None => None,
    };
    let (value, next) = if key_type == "none" {
        (KeyData::Missing, String::new())
    } else {
        page(conn, &key, &key_type, &req.cursor, count_of(req)).await?
    };
    Ok(KeyDetail {
        key: values::bytes(&key),
        key_type,
        ttl,
        size,
        encoding: (!encoding.is_empty()).then_some(encoding),
        value,
        next,
    })
}

/// A page of a collection (or the string) and the cursor after it.
async fn page(
    conn: &mut ConnectionManager,
    key: &[u8],
    key_type: &str,
    cursor: &str,
    count: u32,
) -> AppResult<(KeyData, String)> {
    let offset: i64 = cursor.parse().unwrap_or(0);
    let last = offset + count as i64 - 1;
    Ok(match key_type {
        "string" => (string(conn, key).await?, String::new()),
        "hash" | "set" => {
            let scan = if key_type == "hash" { "HSCAN" } else { "SSCAN" };
            let start = if cursor.is_empty() { "0" } else { cursor };
            let cmd = redis::cmd(scan)
                .arg(key)
                .arg(start)
                .arg("COUNT")
                .arg(count)
                .clone();
            let mut parts = items(query(conn, &cmd).await?).into_iter();
            let next = parts
                .next()
                .and_then(|c| scalar_text(&c))
                .unwrap_or_default();
            let found = parts.next().map(items).unwrap_or_default();
            let next = if next == "0" { String::new() } else { next };
            let data = if key_type == "hash" {
                KeyData::Hash {
                    entries: pairs(&found, "field", "value"),
                }
            } else {
                KeyData::Set {
                    members: found.iter().map(values::reply).collect(),
                }
            };
            (data, next)
        }
        "list" => {
            let cmd = redis::cmd("LRANGE").arg(key).arg(offset).arg(last).clone();
            let found = items(query(conn, &cmd).await?);
            let next = more(found.len(), count, last);
            let items = found
                .iter()
                .enumerate()
                .map(|(i, v)| json!({ "index": offset + i as i64, "value": values::reply(v) }))
                .collect();
            (KeyData::List { items }, next)
        }
        "zset" => {
            let cmd = redis::cmd("ZRANGE")
                .arg(key)
                .arg(offset)
                .arg(last)
                .arg("WITHSCORES")
                .clone();
            let found = items(query(conn, &cmd).await?);
            let members = pairs(&found, "member", "score");
            let next = more(members.len(), count, last);
            (KeyData::Zset { members }, next)
        }
        "stream" => {
            let start = if cursor.is_empty() {
                "-".to_string()
            } else {
                format!("({cursor}")
            };
            let cmd = redis::cmd("XRANGE")
                .arg(key)
                .arg(start)
                .arg("+")
                .arg("COUNT")
                .arg(count)
                .clone();
            let entries: Vec<Value> = items(query(conn, &cmd).await?)
                .into_iter()
                .map(stream_entry)
                .collect();
            let next = match entries.last() {
                Some(e) if entries.len() == count as usize => {
                    e["id"].as_str().unwrap_or_default().to_string()
                }
                _ => String::new(),
            };
            (KeyData::Stream { entries }, next)
        }
        _ => (KeyData::Other, String::new()),
    })
}

/// The next index cursor when the page came back full.
fn more(found: usize, count: u32, last: i64) -> String {
    if found == count as usize {
        (last + 1).to_string()
    } else {
        String::new()
    }
}

fn pairs(flat: &[Reply], a: &str, b: &str) -> Vec<Value> {
    flat.chunks(2)
        .filter(|p| p.len() == 2)
        .map(|p| {
            let second = if b == "score" {
                scalar_text(&p[1])
                    .and_then(|t| t.parse::<f64>().ok())
                    .map(crate::db::value::float)
                    .unwrap_or_else(|| values::reply(&p[1]))
            } else {
                values::reply(&p[1])
            };
            let mut o = Map::new();
            o.insert(a.into(), values::reply(&p[0]));
            o.insert(b.into(), second);
            Value::Object(o)
        })
        .collect()
}

/// `[id, [field, value, …]]` as `{"id": …, "fields": [[field, value], …]}`.
fn stream_entry(r: Reply) -> Value {
    let mut parts = items(r).into_iter();
    let id = parts
        .next()
        .and_then(|i| scalar_text(&i))
        .unwrap_or_default();
    let flat = parts.next().map(items).unwrap_or_default();
    let fields: Vec<Value> = flat
        .chunks(2)
        .filter(|p| p.len() == 2)
        .map(|p| json!([values::reply(&p[0]), values::reply(&p[1])]))
        .collect();
    json!({ "id": id, "fields": fields })
}

async fn string(conn: &mut ConnectionManager, key: &[u8]) -> AppResult<KeyData> {
    let cmd = redis::cmd("GETRANGE")
        .arg(key)
        .arg(0)
        .arg(MAX_STRING - 1)
        .clone();
    let b = reply_bytes(&query(conn, &cmd).await?);
    let len = int(&query(conn, redis::cmd("STRLEN").arg(key)).await?).unwrap_or(0);
    let truncated = len as usize > b.len();
    Ok(match String::from_utf8(b) {
        Ok(text) => KeyData::String {
            text: Some(text),
            binary: None,
            truncated,
        },
        Err(e) => KeyData::String {
            text: None,
            binary: Some(crate::db::value::binary(e.as_bytes())),
            truncated,
        },
    })
}

/// The whole value of a key for Quick Look: strings as they are, collections as JSON
/// (at most `CELL_ITEMS` elements).
pub async fn cell(conn: &mut ConnectionManager, key: &[u8]) -> AppResult<CellContent> {
    let (key_type, _, _) = meta(conn, key).await?;
    if key_type == "none" {
        return Err(AppError::not_found("This key no longer exists."));
    }
    if key_type == "string" {
        let b = reply_bytes(&query(conn, redis::cmd("GET").arg(key)).await?);
        return Ok(match String::from_utf8(b) {
            Ok(text) => CellContent::Text { text },
            Err(e) => {
                let b = e.into_bytes();
                CellContent::Binary {
                    base64: base64::engine::general_purpose::STANDARD.encode(&b),
                    size: b.len(),
                    truncated: false,
                    mime: None,
                }
            }
        });
    }
    let mut all: Vec<Value> = Vec::new();
    let mut cursor = String::new();
    loop {
        let (data, next) = page(conn, key, &key_type, &cursor, MAX_COUNT).await?;
        all.extend(match data {
            KeyData::Hash { entries } => entries,
            KeyData::List { items } => items.into_iter().map(|i| i["value"].clone()).collect(),
            KeyData::Set { members } => members,
            KeyData::Zset { members } => members,
            KeyData::Stream { entries } => entries,
            _ => Vec::new(),
        });
        if next.is_empty() || all.len() >= CELL_ITEMS {
            break;
        }
        cursor = next;
    }
    all.truncate(CELL_ITEMS);
    let doc = if key_type == "hash" {
        let obj: Map<String, Value> = all
            .into_iter()
            .map(|e| {
                let field = match &e["field"] {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (field, e["value"].clone())
            })
            .collect();
        Value::Object(obj)
    } else {
        Value::Array(all)
    };
    let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    Ok(CellContent::Text { text })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bulk(s: &str) -> Reply {
        Reply::BulkString(s.as_bytes().to_vec())
    }

    #[test]
    fn shapes_pairs_and_stream_entries() {
        let flat = [bulk("m"), bulk("1.5"), bulk("n"), bulk("2")];
        assert_eq!(
            pairs(&flat, "member", "score"),
            vec![
                json!({"member": "m", "score": 1.5}),
                json!({"member": "n", "score": 2.0})
            ]
        );
        let entry = Reply::Array(vec![bulk("1-0"), Reply::Array(vec![bulk("f"), bulk("v")])]);
        assert_eq!(
            stream_entry(entry),
            json!({ "id": "1-0", "fields": [["f", "v"]] })
        );
        assert_eq!(more(10, 10, 9), "10");
        assert_eq!(more(3, 10, 9), "");
    }
}
