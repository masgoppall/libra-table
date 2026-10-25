//! Finding the native client tools (pg_dump, psql, mysqldump, …) and choosing the one
//! that fits the server. An app started from the Dock gets a minimal PATH, so the
//! usual install places (Homebrew, Postgres.app, DBngin, the MySQL package) are
//! searched as well.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::models::Driver;

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCandidate {
    pub path: String,
    /// First line of `--version`.
    pub version: String,
    pub major: u32,
    pub mariadb: bool,
}

/// The program names a tool goes by (MariaDB ships `mariadb-dump` next to `mysqldump`).
pub fn program_names(tool: &str) -> &'static [&'static str] {
    match tool {
        "pg_dump" => &["pg_dump"],
        "pg_restore" => &["pg_restore"],
        "psql" => &["psql"],
        "mysqldump" => &["mysqldump", "mariadb-dump"],
        "mysql" => &["mysql", "mariadb"],
        "sqlite3" => &["sqlite3"],
        _ => &[],
    }
}

/// Subdirectories of `dir` whose name starts with one of `prefixes`, each joined
/// with `tail` (e.g. `/opt/homebrew/opt/postgresql@*/bin`).
fn children(dir: &Path, prefixes: &[&str], tail: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            prefixes.is_empty() || prefixes.iter().any(|p| name.starts_with(p))
        })
        .map(|e| e.path().join(tail))
        .collect();
    out.sort();
    out
}

/// Every `bin` directory below `dir`, at most `depth` levels down (DBngin's bundles).
fn bin_dirs(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    for sub in children(dir, &[], "") {
        if sub.file_name().is_some_and(|n| n == "bin") {
            out.push(sub);
        } else if sub.is_dir() {
            bin_dirs(&sub, depth - 1, out);
        }
    }
}

/// The directories to look in, PATH first, without duplicates.
pub fn search_dirs(home: Option<PathBuf>, path_var: Option<OsString>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = path_var
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for fixed in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/usr/local/mysql/bin",
    ] {
        dirs.push(PathBuf::from(fixed));
    }
    let brew = ["postgresql", "libpq", "mysql", "mariadb"];
    for opt in ["/opt/homebrew/opt", "/usr/local/opt"] {
        dirs.extend(children(Path::new(opt), &brew, "bin"));
    }
    let postgres_app = Path::new("/Applications/Postgres.app/Contents/Versions");
    dirs.extend(children(postgres_app, &[], "bin"));
    if let Some(home) = home {
        let dbngin = home.join("Library/Application Support/com.tinyapp.DBngin/Binaries");
        bin_dirs(&dbngin, 4, &mut dirs);
    }
    // DBngin also keeps its engines in a shared folder: <engine>/<version>/bin.
    for engine in children(Path::new("/Users/Shared/DBngin"), &[], "") {
        dirs.extend(children(&engine, &[], "bin"));
    }
    let mut seen = HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// Major version and flavor from a `--version` line, e.g.
/// `pg_dump (PostgreSQL) 18.4`, `mysqldump  Ver 10.13 Distrib 5.7.23, for osx`,
/// `mysqldump  Ver 9.7.1 for macos15`, `mariadb-dump from 11.4.2-MariaDB, client 10.19`.
pub fn parse_version(output: &str) -> Option<(u32, bool)> {
    let line = output.lines().next()?;
    let mariadb = line.contains("MariaDB");
    let after = |key: &str| line.find(key).map(|i| &line[i + key.len()..]);
    let rest = after("Distrib ")
        .or_else(|| after(" from "))
        .or_else(|| after("(PostgreSQL) "))
        .or_else(|| after("Ver "))
        .unwrap_or(line);
    leading_number(rest.trim_start()).map(|m| (m, mariadb))
}

fn leading_number(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Major version and flavor of the server from its version string ("16.4 (Debian…)",
/// "10.11.6-MariaDB-log", "9.7.1").
pub fn server_major(version: &str) -> Option<(u32, bool)> {
    leading_number(version.trim()).map(|m| (m, version.contains("MariaDB")))
}

/// Runs `<path> --version` and reads it.
pub async fn probe(path: &Path) -> Option<ToolCandidate> {
    let run = tokio::process::Command::new(path)
        .arg("--version")
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(PROBE_TIMEOUT, run).await.ok()?.ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (major, mariadb) = parse_version(&text)?;
    Some(ToolCandidate {
        path: path.display().to_string(),
        version: text.lines().next().unwrap_or("").trim().to_string(),
        major,
        mariadb,
    })
}

/// Every copy of `tool` in `dirs` that answers `--version` (symlinks to the same
/// file count once).
pub async fn candidates(tool: &str, dirs: &[PathBuf]) -> Vec<ToolCandidate> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for dir in dirs {
        for name in program_names(tool) {
            let p = dir.join(name);
            if !p.is_file() {
                continue;
            }
            let real = std::fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
            if seen.insert(real) {
                paths.push(p);
            }
        }
    }
    let probes = paths.iter().map(|p| probe(p));
    futures::future::join_all(probes)
        .await
        .into_iter()
        .flatten()
        .collect()
}

/// The candidate to use: the same flavor (MySQL / MariaDB), the oldest version that is
/// at least the server's (pg_dump refuses servers newer than itself), else the newest
/// there is. Returns its index and a warning, if any.
pub fn pick(
    driver: Driver,
    cands: &[ToolCandidate],
    server: Option<(u32, bool)>,
) -> (Option<usize>, Option<String>) {
    let Some((want, mariadb)) = server else {
        return (if cands.is_empty() { None } else { Some(0) }, None);
    };
    let flavor_ok = |c: &ToolCandidate| driver != Driver::Mysql || c.mariadb == mariadb;
    let same: Vec<usize> = (0..cands.len()).filter(|&i| flavor_ok(&cands[i])).collect();
    let pool: Vec<usize> = if same.is_empty() {
        (0..cands.len()).collect()
    } else {
        same
    };
    let newer = pool
        .iter()
        .copied()
        .filter(|&i| cands[i].major >= want)
        .min_by_key(|&i| cands[i].major);
    if let Some(i) = newer {
        return (Some(i), None);
    }
    let newest = pool.iter().copied().max_by_key(|&i| cands[i].major);
    let warning = newest.map(|i| {
        format!(
            "The newest version found ({}) is older than the server ({want}); it may refuse \
             to work with it.",
            cands[i].major
        )
    });
    (newest, warning)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(major: u32, mariadb: bool) -> ToolCandidate {
        ToolCandidate {
            path: format!("/x/{major}"),
            version: String::new(),
            major,
            mariadb,
        }
    }

    #[test]
    fn parses_tool_versions() {
        let v = |s| parse_version(s);
        assert_eq!(
            v("pg_dump (PostgreSQL) 18.4 (DBngin.app)\n"),
            Some((18, false))
        );
        assert_eq!(v("psql (PostgreSQL) 9.6.24"), Some((9, false)));
        assert_eq!(
            v("mysqldump  Ver 10.13 Distrib 5.7.23, for osx10.13 (x86_64)"),
            Some((5, false))
        );
        assert_eq!(
            v("mysqldump  Ver 9.7.1 for macos15 on arm64"),
            Some((9, false))
        );
        assert_eq!(
            v("mysql  Ver 15.1 Distrib 10.11.6-MariaDB, for osx10.19 (arm64)"),
            Some((10, true))
        );
        assert_eq!(
            v("mariadb-dump from 11.4.2-MariaDB, client 10.19 for osx10.19"),
            Some((11, true))
        );
        assert_eq!(v("3.43.2 2023-10-10 13:08:14"), Some((3, false)));
        assert_eq!(v("nonsense"), None);
        assert_eq!(server_major("10.11.6-MariaDB-log"), Some((10, true)));
        assert_eq!(server_major("16.4 (Debian 16.4-1)"), Some((16, false)));
    }

    #[test]
    fn picks_the_closest_version_not_older_than_the_server() {
        let pg = vec![cand(14, false), cand(18, false), cand(16, false)];
        assert_eq!(pick(Driver::Postgres, &pg, Some((15, false))).0, Some(2));
        assert_eq!(
            pick(Driver::Postgres, &pg, Some((18, false))),
            (Some(1), None)
        );
        let (i, warn) = pick(Driver::Postgres, &pg, Some((19, false)));
        assert_eq!(i, Some(1));
        assert!(warn.unwrap().contains("older than the server"));
        assert_eq!(pick(Driver::Postgres, &[], Some((16, false))).0, None);
    }

    #[test]
    fn prefers_the_server_flavor() {
        let my = vec![cand(9, false), cand(10, true), cand(8, false)];
        assert_eq!(pick(Driver::Mysql, &my, Some((10, true))).0, Some(1));
        assert_eq!(pick(Driver::Mysql, &my, Some((8, false))).0, Some(2));
        // Only the other flavor installed: still offered.
        let only = vec![cand(9, false)];
        assert_eq!(pick(Driver::Mysql, &only, Some((10, true))).0, Some(0));
    }

    #[test]
    fn search_dirs_start_with_path_and_have_no_duplicates() {
        let path = std::env::join_paths(["/a/bin", "/usr/local/bin"]).unwrap();
        let dirs = search_dirs(None, Some(path));
        assert_eq!(dirs[0], PathBuf::from("/a/bin"));
        let n = dirs.iter().filter(|d| d.ends_with("usr/local/bin")).count();
        assert_eq!(n, 1);
    }
}
