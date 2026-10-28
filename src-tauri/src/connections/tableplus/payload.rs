//! The decrypted export, whatever its shape: a flat list of connections, a dictionary of
//! connections and groups, or groups holding their connections (and subgroups) inside.
//! JSON and property lists are both read, and so are documents nested as (base64) text.

use base64::Engine;
use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

pub type Dict = Map<String, Value>;

/// Deeper than any real group tree; stops runaway documents.
const MAX_DEPTH: usize = 64;

#[derive(Default)]
pub struct Payload {
    pub connections: Vec<Dict>,
    pub groups: Vec<Dict>,
}

pub fn plist_to_json(v: plist::Value) -> Value {
    match v {
        plist::Value::Array(items) => Value::Array(items.into_iter().map(plist_to_json).collect()),
        plist::Value::Dictionary(d) => {
            Value::Object(d.into_iter().map(|(k, v)| (k, plist_to_json(v))).collect())
        }
        plist::Value::Boolean(b) => Value::Bool(b),
        plist::Value::Integer(n) => n.as_signed().map_or(Value::Null, Value::from),
        plist::Value::Real(f) => Value::from(f),
        plist::Value::String(s) => Value::String(s),
        plist::Value::Data(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Value::String(text),
            Err(e) => Value::String(base64::prelude::BASE64_STANDARD.encode(e.into_bytes())),
        },
        _ => Value::Null,
    }
}

/// A JSON or property-list document (only lists and dictionaries count).
fn parse(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .or_else(|| {
            plist::Value::from_reader(std::io::Cursor::new(bytes))
                .ok()
                .map(plist_to_json)
        })
        .filter(|v| v.is_array() || v.is_object())
}

/// A document stored as text inside the document.
fn nested(text: &str) -> Option<Value> {
    let t = text.trim();
    if t.starts_with(['[', '{', '<']) {
        return parse(t.as_bytes());
    }
    if t.len() < 16 {
        return None;
    }
    let bytes = base64::prelude::BASE64_STANDARD.decode(t).ok()?;
    let looks_like_doc = bytes.starts_with(b"bplist") || bytes.starts_with(b"<?xml");
    if looks_like_doc || bytes.first().is_some_and(|b| matches!(b, b'[' | b'{')) {
        parse(&bytes)
    } else {
        None
    }
}

pub fn key<'a>(d: &'a Dict, name: &str) -> Option<&'a Value> {
    d.get(name).or_else(|| {
        d.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    })
}

fn text(d: &Dict, name: &str) -> String {
    key(d, name)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn is_connection(d: &Dict) -> bool {
    key(d, "ConnectionName").is_some() || key(d, "Driver").is_some()
}

/// A group of `ConnectionGroups.plist` (`ID`, `Name`, `GroupID`, `IsExpaned`), possibly
/// with its contents inside.
fn is_group(d: &Dict) -> bool {
    key(d, "Name").is_some()
        && (key(d, "IsExpaned").is_some() || d.values().any(|v| v.is_array() || v.is_object()))
}

/// Puts the enclosing group in `GroupID` when the entry doesn't name one itself.
fn adopt(d: &mut Dict, parent: Option<&str>) {
    if let (Some(p), true) = (parent, text(d, "GroupID").is_empty()) {
        d.insert("GroupID".into(), Value::String(p.into()));
    }
}

fn walk(v: &Value, parent: Option<&str>, out: &mut Payload, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    match v {
        Value::Array(items) => items.iter().for_each(|i| walk(i, parent, out, depth + 1)),
        // Connections are leaves: their nested settings are not walked.
        Value::Object(d) if is_connection(d) => {
            let mut c = d.clone();
            adopt(&mut c, parent);
            out.connections.push(c);
        }
        Value::Object(d) => {
            let mut id = parent.map(str::to_string);
            if is_group(d) {
                let mut g = d.clone();
                let own = Some(text(d, "ID"))
                    .filter(|i| !i.is_empty())
                    .unwrap_or_else(|| format!("tableplus-group-{}", out.groups.len()));
                g.insert("ID".into(), Value::String(own.clone()));
                adopt(&mut g, parent);
                out.groups.push(g);
                id = Some(own);
            }
            for child in d.values() {
                walk(child, id.as_deref(), out, depth + 1);
            }
        }
        Value::String(s) => {
            if let Some(inner) = nested(s) {
                walk(&inner, parent, out, depth + 1);
            }
        }
        _ => {}
    }
}

/// What the document holds, by key names only (never values), for the error message.
fn shape(v: &Value) -> String {
    let keys = |d: &Dict| {
        let mut k: Vec<&str> = d.keys().map(String::as_str).collect();
        k.sort_unstable();
        k.truncate(12);
        k.join(", ")
    };
    match v {
        Value::Array(items) => match items.iter().find_map(Value::as_object) {
            Some(d) => format!("{} entries with the keys {}", items.len(), keys(d)),
            None => format!("{} entries", items.len()),
        },
        Value::Object(d) => format!("a dictionary with the keys {}", keys(d)),
        _ => "no list".into(),
    }
}

pub fn read(plain: &[u8]) -> AppResult<Payload> {
    let doc = parse(plain)
        .ok_or_else(|| AppError::validation("The TablePlus export has an unknown format."))?;
    let mut out = Payload::default();
    walk(&doc, None, &mut out, 0);
    if out.connections.is_empty() {
        return Err(AppError::validation(format!(
            "No connections were found in the TablePlus export (it holds {}).",
            shape(&doc)
        )));
    }
    Ok(out)
}
