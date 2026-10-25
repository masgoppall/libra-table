//! One JSON file per connection in a folder of the data directory
//! (`metrics/<connectionId>.json`, `diagrams/<connectionId>.json`). Same rules as the
//! config files: atomic writes, a `schemaVersion` envelope, no secret-like keys.

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::CONFIG_SCHEMA_VERSION;

use super::atomic_write;
use super::files::{find_forbidden_key, newer_schema_error, schema_version, Versioned};

pub struct ConnFiles {
    root: PathBuf,
}

impl ConnFiles {
    /// Files live in `<data_dir>/<folder>/`.
    pub fn new(data_dir: &Path, folder: &str) -> Self {
        Self {
            root: data_dir.join(folder),
        }
    }

    fn file(&self, connection_id: &str) -> AppResult<PathBuf> {
        let valid = !connection_id.is_empty()
            && connection_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !valid {
            return Err(AppError::validation("Invalid connection id."));
        }
        Ok(self.root.join(format!("{connection_id}.json")))
    }

    /// The file's body, or the default when there is none yet.
    pub fn load<T: DeserializeOwned + Default>(&self, connection_id: &str) -> AppResult<T> {
        let path = self.file(connection_id)?;
        if !path.exists() {
            return Ok(T::default());
        }
        let raw: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let version = schema_version(&raw);
        if version > CONFIG_SCHEMA_VERSION {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            return Err(newer_schema_error(name, version));
        }
        Ok(serde_json::from_value(raw)?)
    }

    pub fn save<T: Serialize>(&self, connection_id: &str, body: &T) -> AppResult<()> {
        let path = self.file(connection_id)?;
        let value = serde_json::to_value(Versioned {
            schema_version: CONFIG_SCHEMA_VERSION,
            body,
        })?;
        if let Some(key) = find_forbidden_key(&value) {
            return Err(AppError::internal(format!(
                "Refusing to write a secret-like field \"{key}\"."
            )));
        }
        atomic_write(&path, &serde_json::to_vec_pretty(&value)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
    struct Body {
        items: Vec<String>,
    }

    #[test]
    fn roundtrip_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        let files = ConnFiles::new(dir.path(), "boards");
        assert_eq!(files.load::<Body>("c-1").unwrap(), Body::default());
        let body = Body {
            items: vec!["a".into()],
        };
        files.save("c-1", &body).unwrap();
        assert_eq!(files.load::<Body>("c-1").unwrap(), body);
        let raw = fs::read_to_string(dir.path().join("boards/c-1.json")).unwrap();
        assert!(raw.contains("\"schemaVersion\": 1"));
        assert!(files.save("../x", &body).is_err());
    }
}
