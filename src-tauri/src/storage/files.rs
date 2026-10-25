//! Low-level file helpers: atomic writes, the `schemaVersion` envelope, migration of
//! older files and the guard against secret-like keys.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::CONFIG_SCHEMA_VERSION;

/// Keys that must never appear in a config file. The model cannot hold secrets by
/// construction; this is the belt to that pair of braces.
const FORBIDDEN_KEYS: &[&str] = &[
    "password",
    "dbPassword",
    "sshPassword",
    "passphrase",
    "sshKeyPassphrase",
    "privateKey",
    "secret",
];

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Versioned<T> {
    pub schema_version: u32,
    #[serde(flatten)]
    pub body: T,
}

/// The `schemaVersion` of a raw file (0 for files written before it existed).
pub(super) fn schema_version(raw: &Value) -> u32 {
    raw.get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32
}

pub(super) fn newer_schema_error(name: &str, version: u32) -> AppError {
    AppError::validation(format!(
        "{name} was written by a newer LibraTable (schema {version}); \
         this version reads up to {CONFIG_SCHEMA_VERSION}."
    ))
    .hint("Update LibraTable to open this data.")
}

/// Migrates an older file to the current schema. Version 0 is a bare array.
pub(super) fn migrate(raw: Value, from: u32, key: &str) -> Value {
    let mut value = raw;
    if from == 0 && value.is_array() {
        let mut obj = serde_json::Map::new();
        obj.insert(key.to_string(), value);
        value = Value::Object(obj);
    }
    if let Value::Object(ref mut obj) = value {
        obj.insert("schemaVersion".into(), Value::from(CONFIG_SCHEMA_VERSION));
    }
    value
}

pub(super) fn find_forbidden_key(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if FORBIDDEN_KEYS.contains(&k.as_str()) {
                    return Some(k.clone());
                }
                if let Some(found) = find_forbidden_key(v) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_forbidden_key),
        _ => None,
    }
}

/// Writes to a temp file next to `path`, syncs it, then renames it into place.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let dir = path
        .parent()
        .ok_or_else(|| AppError::internal("Invalid file path."))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        uuid::Uuid::new_v4()
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        AppError::from(e)
    })
}
