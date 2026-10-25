//! The sidebar's recent and pinned items, per connection (`sidebar.json`). The UI decides
//! what goes in; this keeps the lists bounded and writes them atomically.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

use super::Storage;

/// Upper bounds, whatever the UI sends (it keeps ~10 recent items per database).
const MAX_RECENT: usize = 200;
const MAX_PINNED: usize = 1000;

/// Serializes read-modify-write of the file between commands.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SidebarItem {
    pub database: String,
    pub schema: String,
    /// "table", "view", "query" (a saved query) or a schema object kind ("function", …).
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Recent items: when it was opened (ms since the epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opened_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SidebarState {
    pub recent: Vec<SidebarItem>,
    pub pinned: Vec<SidebarItem>,
}

#[derive(Serialize, Deserialize, Default)]
struct SidebarFile {
    connections: BTreeMap<String, SidebarState>,
}

const FILE: &str = "sidebar.json";

impl Storage {
    pub fn load_sidebar(&self, connection_id: &str) -> AppResult<SidebarState> {
        let file: SidebarFile = self.load_versioned(FILE, "connections")?;
        Ok(file
            .connections
            .get(connection_id)
            .cloned()
            .unwrap_or_default())
    }

    pub fn save_sidebar(&self, connection_id: &str, mut state: SidebarState) -> AppResult<()> {
        state.recent.truncate(MAX_RECENT);
        state.pinned.truncate(MAX_PINNED);
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut file: SidebarFile = self.load_versioned(FILE, "connections")?;
        if state == SidebarState::default() {
            file.connections.remove(connection_id);
        } else {
            file.connections.insert(connection_id.to_string(), state);
        }
        self.save_versioned(FILE, &file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> SidebarItem {
        SidebarItem {
            database: "app".into(),
            schema: "public".into(),
            kind: "table".into(),
            name: name.into(),
            ..SidebarItem::default()
        }
    }

    #[test]
    fn keeps_each_connection_apart_and_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let s = Storage::new(dir.path()).unwrap();
        let recent: Vec<SidebarItem> = (0..MAX_RECENT + 5).map(|i| item(&i.to_string())).collect();
        let a = SidebarState {
            recent,
            pinned: vec![item("people")],
        };
        s.save_sidebar("a", a).unwrap();
        s.save_sidebar("b", SidebarState::default()).unwrap();
        let loaded = s.load_sidebar("a").unwrap();
        assert_eq!(loaded.recent.len(), MAX_RECENT);
        assert_eq!(loaded.pinned, vec![item("people")]);
        assert_eq!(s.load_sidebar("b").unwrap(), SidebarState::default());
        let raw = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(raw.contains("\"schemaVersion\": 1"));
        assert!(!raw.contains("openedAt"));
    }
}
