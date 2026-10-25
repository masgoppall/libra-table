//! Paths of native tools the user chose in the backup / restore dialogs
//! (`tool_paths.json`: tool name → program path).

use std::collections::BTreeMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

use super::Storage;

const FILE: &str = "tool_paths.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolPaths {
    pub paths: BTreeMap<String, String>,
}

impl Storage {
    pub fn load_tool_paths(&self) -> AppResult<ToolPaths> {
        let path = self.path(FILE);
        if !path.exists() {
            return Ok(ToolPaths::default());
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    /// Sets (or with `None` forgets) the path of one tool.
    pub fn set_tool_path(&self, tool: &str, program: Option<&str>) -> AppResult<()> {
        let mut paths = self.load_tool_paths()?;
        match program {
            Some(p) => paths.paths.insert(tool.to_string(), p.to_string()),
            None => paths.paths.remove(tool),
        };
        self.write_json(FILE, &serde_json::to_value(&paths)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_forget() {
        let dir = tempfile::tempdir().unwrap();
        let s = Storage::new(dir.path()).unwrap();
        assert!(s.load_tool_paths().unwrap().paths.is_empty());
        s.set_tool_path("pg_dump", Some("/opt/pg/bin/pg_dump"))
            .unwrap();
        let got = s.load_tool_paths().unwrap();
        assert_eq!(got.paths["pg_dump"], "/opt/pg/bin/pg_dump");
        s.set_tool_path("pg_dump", None).unwrap();
        assert!(s.load_tool_paths().unwrap().paths.is_empty());
    }
}
