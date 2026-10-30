//! Export commands: start a background export job and cancel it. Progress goes to
//! the UI as `export-progress` events.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::export::{self, ExportRequest};
use crate::state::{AppState, Session};
use crate::storage::atomic_write;

use super::{confirm_safe_mode, Cmd};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    job_id: String,
    rows: u64,
    done: bool,
    error: Option<AppError>,
    path: String,
}

#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    job_id: String,
    request: ExportRequest,
    confirmed: Option<bool>,
) -> Cmd<()> {
    let s = state.session_in(&id, database.as_deref()).await?;
    if request.path.trim().is_empty() {
        return Err(AppError::validation("Choose where to save the file."));
    }
    // Exporting a query result runs the query again: safe mode applies (SAFE-2).
    if let export::Source::Query { sql } = &request.source {
        let stmts: Vec<String> = crate::engines::split(s.connection.driver, sql)
            .into_iter()
            .map(|st| st.text)
            .collect();
        confirm_safe_mode(&state, &s, &stmts, confirmed.unwrap_or(false))?;
    }
    let token = CancellationToken::new();
    state
        .exports
        .lock()
        .unwrap()
        .insert(job_id.clone(), token.clone());
    tauri::async_runtime::spawn(run_job(app, s, job_id, request, token));
    Ok(())
}

/// Runs one export job and reports its progress, then forgets the job.
async fn run_job(
    app: AppHandle,
    s: Arc<Session>,
    job_id: String,
    request: ExportRequest,
    token: CancellationToken,
) {
    let emit = |rows: u64, done: bool, error: Option<AppError>| {
        let progress = ExportProgress {
            job_id: job_id.clone(),
            rows,
            done,
            error,
            path: request.path.clone(),
        };
        let _ = app.emit("export-progress", progress);
    };
    let result = export::run(&s, &request, &token, |rows| emit(rows, false, None)).await;
    match result {
        Ok(rows) => emit(rows, true, None),
        Err(e) => emit(0, true, Some(e)),
    }
    if let Some(state) = app.try_state::<AppState>() {
        state.exports.lock().unwrap().remove(&job_id);
    }
}

#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>, job_id: String) -> bool {
    match state.exports.lock().unwrap().get(&job_id) {
        Some(t) => {
            t.cancel();
            true
        }
        None => false,
    }
}

/// Extensions `save_text_file` may write: data the UI formatted itself (query results).
const TEXT_EXTENSIONS: &[&str] = &["csv", "tsv", "json", "sql", "md", "html", "txt"];

/// Writes text the UI made (e.g. the results of every result tab) to a file the user picked.
#[tauri::command]
pub fn save_text_file(path: String, contents: String) -> Cmd<()> {
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !TEXT_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::validation(
            "Save the results as .csv, .tsv, .json, .sql, .md, .html or .txt.",
        ));
    }
    atomic_write(p, contents.as_bytes())
}

/// Deletes the oldest backups of a schedule in `folder`, keeping `keep`: only files named
/// `<prefix>-<date>.<sql|dump|db>` (what scheduled backups write) are touched. Returns
/// how many were deleted.
#[tauri::command]
pub fn prune_backups(folder: String, prefix: String, keep: u32) -> Cmd<u32> {
    if keep == 0 || prefix.is_empty() || prefix.contains(['/', '\\']) {
        return Ok(0);
    }
    let start = format!("{prefix}-");
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(&folder)?
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ext_ok = [".sql", ".dump", ".db"].iter().any(|x| name.ends_with(x));
            let stamp = name
                .strip_prefix(&start)
                .is_some_and(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()));
            ext_ok && stamp && e.path().is_file()
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|a| std::cmp::Reverse(a.0));
    let mut deleted = 0;
    for (_, path) in files.into_iter().skip(keep as usize) {
        std::fs::remove_file(path)?;
        deleted += 1;
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    #[test]
    fn prunes_only_its_own_old_backups() {
        let dir = std::env::temp_dir().join(format!("libratable-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (i, n) in [
            "shop-2026-01-01-0200.sql",
            "shop-2026-01-02-0200.sql",
            "shop-2026-01-03-0200.sql",
        ]
        .iter()
        .enumerate()
        {
            std::fs::write(dir.join(n), "x").unwrap();
            let t =
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000 + i as u64);
            std::fs::File::options()
                .write(true)
                .open(dir.join(n))
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        std::fs::write(dir.join("shop-notes.sql"), "keep me").unwrap();
        std::fs::write(dir.join("other-2026-01-01.sql"), "keep me").unwrap();
        let folder = dir.to_string_lossy().into_owned();
        assert_eq!(
            super::prune_backups(folder.clone(), "shop".into(), 2).unwrap(),
            1
        );
        assert!(!dir.join("shop-2026-01-01-0200.sql").exists());
        assert!(dir.join("shop-2026-01-03-0200.sql").exists());
        assert!(dir.join("shop-notes.sql").exists() && dir.join("other-2026-01-01.sql").exists());
        assert_eq!(super::prune_backups(folder, "x/..".into(), 1).unwrap(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writes_only_text_data_files() {
        let dir = std::env::temp_dir().join(format!("libratable-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("result.csv");
        super::save_text_file(ok.to_string_lossy().into(), "a,b\n1,2".into()).unwrap();
        assert_eq!(std::fs::read_to_string(&ok).unwrap(), "a,b\n1,2");
        let bad = dir.join("evil.sh");
        assert!(super::save_text_file(bad.to_string_lossy().into(), "x".into()).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
