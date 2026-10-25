//! Everything LibraTable keeps on disk. Config lives in plain JSON files in the app
//! data directory (STO-2..STO-6): every write is atomic (temp file + rename) and every
//! file carries `schemaVersion`; an older file is copied to `<name>.bak` before it is
//! migrated. History, saved queries and keychain secrets have their own modules.

pub mod conn_files;
pub mod crypto;
mod files;
pub mod history;
pub mod saved;
pub mod secrets;
mod settings;
mod sidebar;
mod tools;

pub use files::atomic_write;
pub use settings::{Settings, TableState};
pub use sidebar::{SidebarItem, SidebarState};
pub use tools::ToolPaths;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::{Connection, Group, CONFIG_SCHEMA_VERSION};
use files::{find_forbidden_key, migrate, newer_schema_error, schema_version, Versioned};

pub struct Storage {
    dir: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct ConnectionsFile {
    connections: Vec<Connection>,
}

#[derive(Serialize, Deserialize, Default)]
struct GroupsFile {
    groups: Vec<Group>,
}

#[derive(Serialize, Deserialize, Default)]
struct TableStateFile {
    tables: BTreeMap<String, TableState>,
}

impl Storage {
    pub fn new(dir: impl Into<PathBuf>) -> AppResult<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn load_connections(&self) -> AppResult<Vec<Connection>> {
        let file: ConnectionsFile = self.load_versioned("connections.json", "connections")?;
        Ok(file.connections)
    }

    pub fn save_connections(&self, connections: &[Connection]) -> AppResult<()> {
        let connections = connections.to_vec();
        self.save_versioned("connections.json", &ConnectionsFile { connections })
    }

    pub fn load_groups(&self) -> AppResult<Vec<Group>> {
        let file: GroupsFile = self.load_versioned("groups.json", "groups")?;
        Ok(file.groups)
    }

    pub fn save_groups(&self, groups: &[Group]) -> AppResult<()> {
        let groups = groups.to_vec();
        self.save_versioned("groups.json", &GroupsFile { groups })
    }

    pub fn load_table_states(&self) -> AppResult<BTreeMap<String, TableState>> {
        let file: TableStateFile = self.load_versioned("table_state.json", "tables")?;
        Ok(file.tables)
    }

    pub fn save_table_states(&self, tables: &BTreeMap<String, TableState>) -> AppResult<()> {
        let tables = tables.clone();
        self.save_versioned("table_state.json", &TableStateFile { tables })
    }

    pub fn load_settings(&self) -> AppResult<Settings> {
        let path = self.path("settings.json");
        if !path.exists() {
            return Ok(Settings::default());
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.write_json("settings.json", &serde_json::to_value(settings)?)
    }

    /// The workspace (open tabs per connection) is owned by the UI; we only persist it.
    /// Restoring it never connects anything (STO-5).
    pub fn load_workspace(&self) -> AppResult<Value> {
        let path = self.path("workspace.json");
        if !path.exists() {
            return Ok(Value::Object(Default::default()));
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    pub fn save_workspace(&self, workspace: &Value) -> AppResult<()> {
        if !workspace.is_object() {
            return Err(AppError::validation("Workspace must be a JSON object."));
        }
        self.write_json("workspace.json", workspace)
    }

    fn load_versioned<T: DeserializeOwned + Default>(
        &self,
        name: &str,
        legacy_key: &str,
    ) -> AppResult<T> {
        let path = self.path(name);
        if !path.exists() {
            return Ok(T::default());
        }
        let raw: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let version = schema_version(&raw);
        if version > CONFIG_SCHEMA_VERSION {
            return Err(newer_schema_error(name, version));
        }
        if version == CONFIG_SCHEMA_VERSION {
            return Ok(serde_json::from_value(raw)?);
        }
        // Older file: keep a backup, migrate, rewrite.
        fs::copy(&path, self.path(&format!("{name}.bak")))?;
        let migrated = migrate(raw, version, legacy_key);
        Ok(serde_json::from_value(migrated)?)
    }

    fn save_versioned<T: Serialize>(&self, name: &str, body: &T) -> AppResult<()> {
        let value = serde_json::to_value(Versioned {
            schema_version: CONFIG_SCHEMA_VERSION,
            body,
        })?;
        self.write_json(name, &value)
    }

    fn write_json(&self, name: &str, value: &Value) -> AppResult<()> {
        if let Some(key) = find_forbidden_key(value) {
            return Err(AppError::internal(format!(
                "Refusing to write a secret-like field \"{key}\" to {name}."
            )));
        }
        let bytes = serde_json::to_vec_pretty(value)?;
        atomic_write(&self.path(name), &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Driver, Tag};

    fn conn(id: &str) -> Connection {
        Connection {
            id: id.into(),
            name: "Local".into(),
            group_id: None,
            driver: Driver::Postgres,
            tag: Tag::Local,
            color: String::new(),
            db: Default::default(),
            ssh: Default::default(),
            tls: Default::default(),
            options: Default::default(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn roundtrip_with_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let s = Storage::new(dir.path()).unwrap();
        s.save_connections(&[conn("a")]).unwrap();
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join("connections.json")).unwrap())
                .unwrap();
        assert_eq!(raw["schemaVersion"], 1);
        assert_eq!(s.load_connections().unwrap()[0].id, "a");
        // No temp files left behind.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn migrates_legacy_array_and_keeps_backup() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = serde_json::to_vec(&vec![conn("x")]).unwrap();
        fs::write(dir.path().join("connections.json"), legacy).unwrap();
        let s = Storage::new(dir.path()).unwrap();
        assert_eq!(s.load_connections().unwrap()[0].id, "x");
        assert!(dir.path().join("connections.json.bak").exists());
    }

    #[test]
    fn refuses_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("groups.json"),
            br#"{"schemaVersion": 99, "groups": []}"#,
        )
        .unwrap();
        let s = Storage::new(dir.path()).unwrap();
        assert!(s.load_groups().is_err());
    }

    #[test]
    fn refuses_secret_like_keys() {
        let dir = tempfile::tempdir().unwrap();
        let s = Storage::new(dir.path()).unwrap();
        let ws = serde_json::json!({"tabs": [{"password": "x"}]});
        assert!(s.save_workspace(&ws).is_err());
        assert!(!dir.path().join("workspace.json").exists());
    }
}
