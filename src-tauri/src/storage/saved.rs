//! Saved queries (QRY-7): plain `.sql` files in `queries/<connectionId>/`. A name like
//! "Reports/Monthly" is the file `Monthly.sql` in the folder `Reports` (one level).

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedQuery {
    pub name: String,
    pub modified_at: i64,
    pub size: u64,
}

pub struct SavedQueries {
    root: PathBuf,
}

impl SavedQueries {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("queries"),
        }
    }

    fn dir(&self, connection_id: &str) -> AppResult<PathBuf> {
        if connection_id.is_empty()
            || !connection_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(AppError::validation("Invalid connection id."));
        }
        Ok(self.root.join(connection_id))
    }

    fn file(&self, connection_id: &str, name: &str) -> AppResult<PathBuf> {
        let (folder, name) = split_name(name)?;
        let dir = self.dir(connection_id)?;
        let dir = match folder {
            Some(f) => dir.join(f),
            None => dir,
        };
        Ok(dir.join(format!("{name}.sql")))
    }

    pub fn list(&self, connection_id: &str) -> AppResult<Vec<SavedQuery>> {
        let dir = self.dir(connection_id)?;
        let mut out = Vec::new();
        list_dir(&dir, None, &mut out)?;
        out.sort_by_key(|q| q.name.to_lowercase());
        Ok(out)
    }

    pub fn read(&self, connection_id: &str, name: &str) -> AppResult<String> {
        std::fs::read_to_string(self.file(connection_id, name)?).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::not_found(format!("Saved query \"{name}\" was not found."))
            } else {
                e.into()
            }
        })
    }

    pub fn write(&self, connection_id: &str, name: &str, sql: &str) -> AppResult<()> {
        crate::storage::atomic_write(&self.file(connection_id, name)?, sql.as_bytes())
    }

    pub fn rename(&self, connection_id: &str, from: &str, to: &str) -> AppResult<()> {
        let src = self.file(connection_id, from)?;
        let dst = self.file(connection_id, to)?;
        if dst.exists() {
            return Err(AppError::validation(format!(
                "A saved query named \"{to}\" already exists."
            )));
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&src, dst)?;
        self.remove_empty_folder(connection_id, &src);
        Ok(())
    }

    /// A folder left without queries goes away.
    fn remove_empty_folder(&self, connection_id: &str, file: &Path) {
        let (Ok(root), Some(parent)) = (self.dir(connection_id), file.parent()) else {
            return;
        };
        if parent != root {
            let _ = std::fs::remove_dir(parent);
        }
    }

    pub fn delete(&self, connection_id: &str, name: &str) -> AppResult<()> {
        let file = self.file(connection_id, name)?;
        match std::fs::remove_file(&file) {
            Ok(()) => {
                self.remove_empty_folder(connection_id, &file);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_connection(&self, connection_id: &str) -> AppResult<()> {
        match std::fs::remove_dir_all(self.dir(connection_id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// The `.sql` files of `dir`, and of its folders when `folder` is None.
fn list_dir(dir: &Path, folder: Option<&str>, out: &mut Vec<SavedQuery>) -> AppResult<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if folder.is_none() && validate_name(&file_name).is_ok() {
                list_dir(&path, Some(&file_name), out)?;
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let meta = entry.metadata()?;
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(SavedQuery {
            name: folder.map_or(stem.clone(), |f| format!("{f}/{stem}")),
            modified_at,
            size: meta.len(),
        });
    }
    Ok(())
}

/// "Folder/Name" → (Some("Folder"), "Name"); each part is checked like a file name.
fn split_name(name: &str) -> AppResult<(Option<&str>, &str)> {
    match name.trim().split_once('/') {
        Some((folder, rest)) => Ok((Some(validate_name(folder)?), validate_name(rest)?)),
        None => Ok((None, validate_name(name)?)),
    }
}

fn validate_name(name: &str) -> AppResult<&str> {
    let name = name.trim();
    let bad = name.is_empty()
        || name.len() > 120
        || name.starts_with('.')
        || name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '\0') || c.is_control());
    if bad {
        return Err(AppError::validation(
            "Use a name without slashes, colons or a leading dot.",
        ));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crud_and_name_validation() {
        let dir = tempfile::tempdir().unwrap();
        let s = SavedQueries::new(dir.path());
        assert!(s.list("c-1").unwrap().is_empty());
        s.write("c-1", "Monthly revenue", "select 1").unwrap();
        assert_eq!(s.read("c-1", "Monthly revenue").unwrap(), "select 1");
        s.rename("c-1", "Monthly revenue", "Revenue").unwrap();
        assert_eq!(s.list("c-1").unwrap()[0].name, "Revenue");
        s.delete("c-1", "Revenue").unwrap();
        assert!(s.list("c-1").unwrap().is_empty());
        assert!(s.write("c-1", "../escape", "x").is_err());
        assert!(s.write("../c", "x", "x").is_err());
        assert!(s.write("c-1", ".hidden", "x").is_err());
    }

    #[test]
    fn folders_one_level_deep() {
        let dir = tempfile::tempdir().unwrap();
        let s = SavedQueries::new(dir.path());
        s.write("c-1", "Reports/Monthly", "select 1").unwrap();
        s.write("c-1", "Loose", "select 2").unwrap();
        let names: Vec<_> = s.list("c-1").unwrap().into_iter().map(|q| q.name).collect();
        assert_eq!(names, ["Loose", "Reports/Monthly"]);
        assert_eq!(s.read("c-1", "Reports/Monthly").unwrap(), "select 1");
        // Moving out of the folder removes the empty folder.
        s.rename("c-1", "Reports/Monthly", "Monthly").unwrap();
        assert!(!dir.path().join("queries/c-1/Reports").exists());
        s.rename("c-1", "Monthly", "Archive/Monthly").unwrap();
        s.delete("c-1", "Archive/Monthly").unwrap();
        assert!(!dir.path().join("queries/c-1/Archive").exists());
        assert!(s.write("c-1", "a/b/c", "x").is_err());
        assert!(s.write("c-1", "../x/y", "x").is_err());
    }
}
