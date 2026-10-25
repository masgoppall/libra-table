//! The key space as grid pages. SCAN has no offsets, so the engine remembers where each
//! page starts: the next SCAN cursor plus the keys of the last batch that didn't fit on
//! the page before (re-scanning a cursor isn't stable while Redis rehashes). Page 0
//! always starts afresh (and forgets the positions of that query), so a refresh sees new
//! keys. Each page then reads TYPE and TTL, and a size and short preview per type, in
//! two pipelines.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use redis::aio::{ConnectionLike, ConnectionManager};
use redis::{Cmd, Pipeline, Value as Reply};
use serde::Serialize;
use serde_json::Value;

use crate::db::filter::SortSpec;
use crate::error::AppResult;

use super::conn::command_error;
use super::filters::KeyQuery;
use super::values::{self, int, items, reply_bytes};

/// Elements shown in a collection preview.
const PREVIEW_ITEMS: isize = 10;
/// Characters of a value preview.
const PREVIEW_CHARS: usize = 200;
/// Queries whose page positions are kept.
const KEPT_QUERIES: usize = 16;

/// Where a page starts: keys left over from the previous page's last batch, then SCAN
/// from `cursor` (None once the scan is complete).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pos {
    pub cursor: Option<u64>,
    pub pending: Vec<Vec<u8>>,
}

fn start() -> Pos {
    Pos {
        cursor: Some(0),
        pending: Vec::new(),
    }
}

type PageKey = (KeyQuery, u32);
type Starts = HashMap<PageKey, Vec<Option<Pos>>>;

/// Page start positions per query and page size; None = no page there.
#[derive(Default)]
pub struct Pages {
    starts: Mutex<(Starts, VecDeque<PageKey>)>,
}

impl Pages {
    fn get(&self, key: &PageKey) -> Vec<Option<Pos>> {
        let guard = self.starts.lock().unwrap();
        guard
            .0
            .get(key)
            .cloned()
            .unwrap_or_else(|| vec![Some(start())])
    }

    fn put(&self, key: PageKey, starts: Vec<Option<Pos>>) {
        let mut guard = self.starts.lock().unwrap();
        let (map, order) = &mut *guard;
        if !map.contains_key(&key) {
            order.push_back(key.clone());
            if order.len() > KEPT_QUERIES {
                if let Some(old) = order.pop_front() {
                    map.remove(&old);
                }
            }
        }
        map.insert(key, starts);
    }
}

/// One key as the grid and the key list show it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyRow {
    #[serde(skip)]
    pub raw: Vec<u8>,
    pub key: Value,
    #[serde(rename = "type")]
    pub key_type: String,
    /// Seconds to live; -1 = no expiry.
    pub ttl: i64,
    /// Bytes of a string, elements of a collection; None for module types.
    pub size: Option<i64>,
    pub preview: Value,
}

impl KeyRow {
    /// Grid row: key, type, ttl, size, value.
    pub fn cells(&self) -> Vec<Value> {
        vec![
            self.key.clone(),
            Value::String(self.key_type.clone()),
            Value::from(self.ttl),
            self.size.map(Value::from).unwrap_or(Value::Null),
            self.preview.clone(),
        ]
    }
}

pub struct KeyPage {
    pub rows: Vec<KeyRow>,
    pub has_more: bool,
}

/// SCAN COUNT: the page size, so a batch rarely overflows the page by much.
pub fn count_hint(_q: &KeyQuery, page_size: u32) -> u32 {
    page_size
}

fn scan_cmd(q: &KeyQuery, cursor: u64, count: u32) -> Cmd {
    let mut c = redis::cmd("SCAN");
    c.arg(cursor);
    if let Some(p) = &q.pattern {
        c.arg("MATCH").arg(p);
    }
    c.arg("COUNT").arg(count);
    if let Some(t) = &q.key_type {
        c.arg("TYPE").arg(t);
    }
    c
}

/// One SCAN call: next cursor and the batch of keys.
pub async fn scan_once(
    conn: &mut ConnectionManager,
    q: &KeyQuery,
    cursor: u64,
    count: u32,
) -> AppResult<(u64, Vec<Vec<u8>>)> {
    let r: Reply = scan_cmd(q, cursor, count)
        .query_async(conn)
        .await
        .map_err(command_error)?;
    let mut parts = items(r).into_iter();
    let next = parts.next().and_then(|c| int(&c)).unwrap_or(0) as u64;
    let keys = parts.next().map(items).unwrap_or_default();
    Ok((next, keys.iter().map(reply_bytes).collect()))
}

/// Up to `size` keys from `start`, and where the next page starts (None at the end).
async fn scan_page(
    conn: &mut ConnectionManager,
    q: &KeyQuery,
    size: u32,
    start: Pos,
) -> AppResult<(Vec<Vec<u8>>, Option<Pos>)> {
    let size = size as usize;
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    let mut queue: VecDeque<Vec<u8>> = start.pending.into();
    let mut cursor = start.cursor;
    loop {
        while let Some(key) = queue.pop_front() {
            if keys.len() == size {
                queue.push_front(key);
                let pending = queue.into();
                return Ok((keys, Some(Pos { cursor, pending })));
            }
            if seen.insert(key.clone()) {
                keys.push(key);
            }
        }
        let Some(c) = cursor else {
            return Ok((keys, None));
        };
        if keys.len() == size {
            let pending = Vec::new();
            return Ok((keys, Some(Pos { cursor, pending })));
        }
        let (next, batch) = scan_once(conn, q, c, size as u32).await?;
        queue.extend(batch);
        cursor = (next != 0).then_some(next);
    }
}

/// Page `page` of the keys matching `q`, with their details.
pub async fn page(
    conn: &mut ConnectionManager,
    pages: &Pages,
    q: &KeyQuery,
    page: u32,
    size: u32,
) -> AppResult<KeyPage> {
    let cache_key = (q.clone(), size);
    let mut starts = if page == 0 {
        vec![Some(start())]
    } else {
        pages.get(&cache_key)
    };
    // Walk forward to the page from the last start we know.
    while starts.len() <= page as usize {
        let Some(Some(from)) = starts.last().cloned() else {
            break;
        };
        let (_, next) = scan_page(conn, q, size, from).await?;
        starts.push(next);
    }
    let Some(Some(from)) = starts.get(page as usize).cloned() else {
        pages.put(cache_key, starts);
        return Ok(KeyPage {
            rows: Vec::new(),
            has_more: false,
        });
    };
    let (keys, next) = scan_page(conn, q, size, from).await?;
    let has_more = next.is_some();
    starts.truncate(page as usize + 1);
    starts.push(next);
    pages.put(cache_key, starts);
    Ok(KeyPage {
        rows: details(conn, keys).await?,
        has_more,
    })
}

/// Sends a pipeline and returns every reply, server errors included.
pub async fn replies(
    conn: &mut impl ConnectionLike,
    pipe: &Pipeline,
    n: usize,
) -> AppResult<Vec<Reply>> {
    if n == 0 {
        return Ok(Vec::new());
    }
    conn.req_packed_commands(pipe, 0, n)
        .await
        .map_err(command_error)
}

/// TYPE, TTL, size and preview of each key; keys gone meanwhile are left out.
pub async fn details(conn: &mut ConnectionManager, keys: Vec<Vec<u8>>) -> AppResult<Vec<KeyRow>> {
    let mut pipe = redis::pipe();
    for k in &keys {
        pipe.cmd("TYPE").arg(k).cmd("TTL").arg(k);
    }
    let meta = replies(conn, &pipe, keys.len() * 2).await?;
    let mut rows = Vec::new();
    let mut pipe = redis::pipe();
    for (k, m) in keys.into_iter().zip(meta.chunks(2)) {
        let key_type = values::scalar_text(&m[0]).unwrap_or_default();
        if key_type == "none" || key_type.is_empty() {
            continue;
        }
        preview_cmds(&mut pipe, &k, &key_type);
        rows.push(KeyRow {
            key: values::bytes(&k),
            raw: k,
            key_type,
            ttl: int(&m[1]).unwrap_or(-1),
            size: None,
            preview: Value::Null,
        });
    }
    let out = replies(conn, &pipe, rows.len() * 2).await?;
    for (row, r) in rows.iter_mut().zip(out.chunks(2)) {
        row.size = int(&r[0]);
        row.preview = preview(&row.key_type, &r[1], row.size);
    }
    Ok(rows)
}

/// Two commands per key: its size, then a few elements.
fn preview_cmds(pipe: &mut Pipeline, k: &[u8], key_type: &str) {
    let last = PREVIEW_ITEMS - 1;
    match key_type {
        "string" => pipe
            .cmd("STRLEN")
            .arg(k)
            .cmd("GETRANGE")
            .arg(k)
            .arg(0)
            .arg(PREVIEW_CHARS * 4),
        "hash" => pipe
            .cmd("HLEN")
            .arg(k)
            .cmd("HSCAN")
            .arg(k)
            .arg(0)
            .arg("COUNT")
            .arg(PREVIEW_ITEMS),
        "list" => pipe
            .cmd("LLEN")
            .arg(k)
            .cmd("LRANGE")
            .arg(k)
            .arg(0)
            .arg(last),
        "set" => pipe
            .cmd("SCARD")
            .arg(k)
            .cmd("SSCAN")
            .arg(k)
            .arg(0)
            .arg("COUNT")
            .arg(PREVIEW_ITEMS),
        "zset" => pipe
            .cmd("ZCARD")
            .arg(k)
            .cmd("ZRANGE")
            .arg(k)
            .arg(0)
            .arg(last)
            .arg("WITHSCORES"),
        "stream" => pipe
            .cmd("XLEN")
            .arg(k)
            .cmd("XREVRANGE")
            .arg(k)
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(1),
        // Module types: nothing to read generically; ECHO keeps the pairs aligned.
        _ => pipe.cmd("ECHO").arg("").cmd("ECHO").arg(""),
    };
}

/// A short text for the value column.
fn preview(key_type: &str, r: &Reply, size: Option<i64>) -> Value {
    if matches!(r, Reply::ServerError(_)) {
        return Value::Null;
    }
    let shown = match key_type {
        "string" => {
            let b = reply_bytes(r);
            let text = match std::str::from_utf8(&b) {
                Ok(s) => s.to_string(),
                // GETRANGE may cut a character in two at the end.
                Err(e) if e.error_len().is_none() => {
                    String::from_utf8_lossy(&b[..e.valid_up_to()]).into_owned()
                }
                Err(_) => return values::bytes(&b),
            };
            return cut(text, size.unwrap_or(0) > b.len() as i64);
        }
        "hash" => {
            let pairs = items(items(r.clone()).into_iter().nth(1).unwrap_or(Reply::Nil));
            let obj: serde_json::Map<String, Value> = pairs
                .chunks(2)
                .filter(|p| p.len() == 2)
                .take(PREVIEW_ITEMS as usize)
                .map(|p| {
                    let name = String::from_utf8_lossy(&reply_bytes(&p[0])).into_owned();
                    (name, values::reply(&p[1]))
                })
                .collect();
            let more = size.unwrap_or(0) > obj.len() as i64;
            return cut(Value::Object(obj).to_string(), more);
        }
        "set" => items(items(r.clone()).into_iter().nth(1).unwrap_or(Reply::Nil)),
        "zset" => {
            let flat = items(r.clone());
            let pairs: Vec<Value> = flat
                .chunks(2)
                .filter(|p| p.len() == 2)
                .map(|p| Value::Array(vec![values::reply(&p[0]), score(&p[1])]))
                .collect();
            let more = size.unwrap_or(0) > pairs.len() as i64;
            return cut(Value::Array(pairs).to_string(), more);
        }
        "list" | "stream" => items(r.clone()),
        _ => return Value::Null,
    };
    let shown: Vec<Value> = shown
        .iter()
        .take(PREVIEW_ITEMS as usize)
        .map(values::reply)
        .collect();
    let more = size.unwrap_or(0) > shown.len() as i64;
    cut(Value::Array(shown).to_string(), more)
}

fn score(r: &Reply) -> Value {
    match values::scalar_text(r).and_then(|t| t.parse::<f64>().ok()) {
        Some(f) => crate::db::value::float(f),
        None => values::reply(r),
    }
}

fn cut(text: String, more: bool) -> Value {
    let mut out: String = text.chars().take(PREVIEW_CHARS).collect();
    if more || out.len() < text.len() {
        out.push('…');
    }
    Value::String(out)
}

/// Sorts a page by key, type, ttl or size (SCAN itself has no order).
pub fn sort(rows: &mut [KeyRow], sort: &[SortSpec]) {
    rows.sort_by(|a, b| {
        for s in sort {
            let ord = match s.column.as_str() {
                "type" => a.key_type.cmp(&b.key_type),
                "ttl" => a.ttl.cmp(&b.ttl),
                "size" => a.size.cmp(&b.size),
                "value" => a.preview.to_string().cmp(&b.preview.to_string()),
                _ => a.raw.cmp(&b.raw),
            };
            let ord = if s.desc { ord.reverse() } else { ord };
            if ord.is_ne() {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bulk(s: &str) -> Reply {
        Reply::BulkString(s.as_bytes().to_vec())
    }

    #[test]
    fn previews_per_type() {
        assert_eq!(preview("string", &bulk("hello"), Some(5)), "hello");
        let hscan = Reply::Array(vec![bulk("0"), Reply::Array(vec![bulk("f"), bulk("v")])]);
        assert_eq!(preview("hash", &hscan, Some(1)), r#"{"f":"v"}"#);
        assert_eq!(preview("hash", &hscan, Some(3)), r#"{"f":"v"}…"#);
        let list = Reply::Array(vec![bulk("a"), bulk("b")]);
        assert_eq!(preview("list", &list, Some(2)), r#"["a","b"]"#);
        let z = Reply::Array(vec![bulk("m"), bulk("2")]);
        assert_eq!(preview("zset", &z, Some(1)), r#"[["m",2.0]]"#);
        assert_eq!(preview("ReJSON-RL", &bulk(""), None), Value::Null);
        let long = "x".repeat(300);
        let shown = preview("string", &bulk(&long), Some(300));
        assert_eq!(shown.as_str().unwrap().chars().count(), PREVIEW_CHARS + 1);
    }

    #[test]
    fn sorts_within_the_page() {
        let row = |k: &str, ttl: i64| KeyRow {
            raw: k.as_bytes().to_vec(),
            key: Value::from(k),
            key_type: "string".into(),
            ttl,
            size: Some(1),
            preview: Value::Null,
        };
        let mut rows = vec![row("b", 5), row("a", -1), row("c", 9)];
        sort(
            &mut rows,
            &[SortSpec {
                column: "ttl".into(),
                desc: true,
            }],
        );
        assert_eq!(rows[0].raw, b"c");
        sort(
            &mut rows,
            &[SortSpec {
                column: "key".into(),
                desc: false,
            }],
        );
        assert_eq!(rows[0].raw, b"a");
    }

    #[test]
    fn remembers_page_starts_per_query() {
        let pages = Pages::default();
        let key = (KeyQuery::default(), 10);
        assert_eq!(pages.get(&key), vec![Some(start())]);
        let second = Pos {
            cursor: Some(7),
            pending: vec![b"k".to_vec()],
        };
        pages.put(key.clone(), vec![Some(start()), Some(second.clone())]);
        assert_eq!(pages.get(&key)[1], Some(second));
        for i in 0..KEPT_QUERIES as u32 {
            pages.put((KeyQuery::default(), 100 + i), vec![None]);
        }
        assert_eq!(pages.get(&key), vec![Some(start())]);
    }
}
