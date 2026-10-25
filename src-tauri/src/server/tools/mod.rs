//! Backup and restore with the native tools (pg_dump / pg_restore / psql, mysqldump /
//! mysql) and SQLite's `VACUUM INTO`, plus the client command for "Open in command line".
//!
//! The tool is chosen to match the server's major version; the user can point LibraTable
//! at another copy, which is remembered in `tool_paths.json`.

pub mod args;
pub mod cli;
pub mod endpoint;
pub mod locate;
pub mod run;

use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::Driver;
use crate::state::Session;

use args::{BackupRequest, RestoreRequest};
use endpoint::Endpoint;
use locate::ToolCandidate;
use run::{Job, Progress};

/// The tools LibraTable drives; anything else is refused as a path override.
pub const TOOLS: &[&str] = &[
    "pg_dump",
    "pg_restore",
    "psql",
    "mysqldump",
    "mysql",
    "sqlite3",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolChoice {
    pub tool: String,
    /// None when no copy was found.
    pub path: Option<String>,
    pub version: Option<String>,
    pub major: Option<u32>,
    pub mariadb: bool,
    /// The path was chosen by the user.
    pub custom: bool,
    pub warning: Option<String>,
    /// Every copy found, for the dialog's picker.
    pub candidates: Vec<ToolCandidate>,
}

/// A tool and the command it will run (without secrets), for the dialogs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPlan {
    pub tool: Option<ToolChoice>,
    pub command: String,
}

fn install_hint(tool: &str) -> &'static str {
    match tool {
        "pg_dump" | "pg_restore" | "psql" => {
            "Install the PostgreSQL client tools (brew install libpq, Postgres.app or DBngin), \
             or choose the program."
        }
        "sqlite3" => "sqlite3 comes with macOS; choose the program if it was moved.",
        _ => {
            "Install the MySQL client (brew install mysql-client, or DBngin), or choose the \
             program."
        }
    }
}

/// Finds `tool` for a server of `server_version`. `custom` is the user's saved path.
pub async fn resolve(
    tool: &str,
    driver: Driver,
    server_version: &str,
    custom: Option<&str>,
) -> ToolChoice {
    let server = locate::server_major(server_version);
    let dirs = locate::search_dirs(dirs::home_dir(), std::env::var_os("PATH"));
    let candidates = locate::candidates(tool, &dirs).await;
    let mut choice = ToolChoice {
        tool: tool.to_string(),
        path: None,
        version: None,
        major: None,
        mariadb: false,
        custom: custom.is_some(),
        warning: None,
        candidates,
    };
    let picked = match custom {
        Some(path) => locate::probe(Path::new(path)).await.or_else(|| {
            choice.path = Some(path.to_string());
            choice.warning = Some("This program doesn't answer --version.".into());
            None
        }),
        None => {
            let (i, warning) = locate::pick(driver, &choice.candidates, server);
            choice.warning = warning;
            i.and_then(|i| choice.candidates.get(i).cloned())
        }
    };
    if let Some(c) = picked {
        if let (Some((want, _)), true) = (server, custom.is_some()) {
            if c.major < want && tool != "sqlite3" {
                choice.warning = Some(format!(
                    "This is version {}; the server is {want}. It may refuse to work with it.",
                    c.major
                ));
            }
        }
        choice.path = Some(c.path);
        choice.version = Some(c.version);
        choice.major = Some(c.major);
        choice.mariadb = c.mariadb;
    } else if choice.path.is_none() {
        choice.warning = Some(format!("{tool} was not found. {}", install_hint(tool)));
    }
    choice
}

/// The tool a backup on `driver` uses (SQLite needs none).
pub fn backup_tool(driver: Driver) -> Option<&'static str> {
    match driver {
        Driver::Postgres => Some("pg_dump"),
        Driver::Mysql => Some("mysqldump"),
        Driver::Sqlite => None,
    }
}

/// The restore tool before a file is chosen.
pub fn restore_tool_default(driver: Driver) -> AppResult<&'static str> {
    match driver {
        Driver::Postgres => Ok("psql"),
        Driver::Mysql => Ok("mysql"),
        Driver::Sqlite => Err(AppError::validation(
            "An SQLite backup is a database file: open it as a connection instead.",
        )),
    }
}

/// The tool a restore of the file at `path` uses: psql or pg_restore by its format.
pub fn restore_tool(driver: Driver, path: &str) -> AppResult<&'static str> {
    restore_tool_default(driver)?;
    let mut head = [0u8; 5];
    let n = std::io::Read::read(&mut std::fs::File::open(path)?, &mut head)?;
    Ok(args::restore_tool(driver, &head[..n]))
}

pub fn client_tool(driver: Driver) -> &'static str {
    match driver {
        Driver::Postgres => "psql",
        Driver::Mysql => "mysql",
        Driver::Sqlite => "sqlite3",
    }
}

fn program(choice: &ToolChoice) -> AppResult<PathBuf> {
    choice.path.as_deref().map(PathBuf::from).ok_or_else(|| {
        AppError::not_found(format!("{} was not found.", choice.tool))
            .hint(install_hint(&choice.tool))
    })
}

fn require_path(path: &str) -> AppResult<()> {
    if path.trim().is_empty() {
        return Err(AppError::validation("Choose the file."));
    }
    Ok(())
}

fn backup_job(s: &Session, r: &BackupRequest, c: &ToolChoice) -> AppResult<Job> {
    let e = Endpoint::of(s);
    let args = match s.driver() {
        Driver::Postgres => args::pg_dump(&e, r),
        _ => {
            let server_mariadb = s.server_version.contains("MariaDB");
            let tool = (c.major.unwrap_or(0), c.mariadb);
            args::mysqldump(&e, r, tool, server_mariadb)
        }
    };
    Ok(Job {
        program: program(c)?,
        args,
        env: e.env(),
        stdin: None,
        output: Some(PathBuf::from(&r.path)),
    })
}

fn restore_job(s: &Session, r: &RestoreRequest, c: &ToolChoice) -> AppResult<Job> {
    let e = Endpoint::of(s);
    let (args, stdin) = match c.tool.as_str() {
        "pg_restore" => (args::pg_restore(&e, r), None),
        "psql" => (args::psql(&e, r), Some(PathBuf::from(&r.path))),
        _ => (args::mysql(&e, r, c.mariadb), Some(PathBuf::from(&r.path))),
    };
    Ok(Job {
        program: program(c)?,
        args,
        env: e.env(),
        stdin,
        output: None,
    })
}

/// The command line of a job as the user could type it (the password is in the
/// environment, so it isn't there).
fn describe(job: &Job) -> String {
    let mut words = vec![cli::shell_quote(&job.program.display().to_string())];
    words.extend(job.args.iter().map(|a| cli::shell_quote(a)));
    if let Some(input) = &job.stdin {
        words.push(format!(
            "< {}",
            cli::shell_quote(&input.display().to_string())
        ));
    }
    words.join(" ")
}

fn sqlite_backup_sql(path: &str) -> String {
    format!("VACUUM INTO {}", crate::db::sql::literal(path))
}

pub fn backup_plan(s: &Session, r: &BackupRequest, c: Option<ToolChoice>) -> ToolPlan {
    let command = match &c {
        None => sqlite_backup_sql(&r.path),
        Some(c) => backup_job(s, r, c)
            .map(|j| describe(&j))
            .unwrap_or_default(),
    };
    ToolPlan { tool: c, command }
}

pub fn restore_plan(s: &Session, r: &RestoreRequest, c: ToolChoice) -> ToolPlan {
    let command = restore_job(s, r, &c)
        .map(|j| describe(&j))
        .unwrap_or_default();
    ToolPlan {
        tool: Some(c),
        command,
    }
}

/// Runs a backup. A failed or cancelled backup removes the partial file.
pub async fn backup(
    s: &Session,
    r: &BackupRequest,
    c: Option<&ToolChoice>,
    token: &CancellationToken,
    on: impl FnMut(Progress),
) -> AppResult<()> {
    require_path(&r.path)?;
    match (&s.pool, c) {
        (DbPool::Sqlite(p), _) => sqlite_backup(p, &r.path, &s.connection.db.file_path).await,
        (_, Some(c)) => {
            let job = backup_job(s, r, c)?;
            let result = run::run(job, token, on).await;
            if result.is_err() {
                let _ = std::fs::remove_file(&r.path);
            }
            result
        }
        (_, None) => Err(AppError::internal("No backup tool.")),
    }
}

/// `VACUUM INTO` writes a compact copy of the open database; it refuses an existing
/// file, so one the save dialog agreed to replace is removed first (never the database
/// itself).
async fn sqlite_backup(p: &sqlx::SqlitePool, path: &str, db_file: &str) -> AppResult<()> {
    let target = Path::new(path);
    if target.exists() {
        let same = std::fs::canonicalize(target).ok() == std::fs::canonicalize(db_file).ok();
        if same {
            return Err(AppError::validation(
                "Choose another file: this is the database being backed up.",
            ));
        }
        std::fs::remove_file(target)?;
    }
    sqlx::query("VACUUM INTO ?").bind(path).execute(p).await?;
    Ok(())
}

pub async fn restore(
    s: &Session,
    r: &RestoreRequest,
    c: &ToolChoice,
    token: &CancellationToken,
    on: impl FnMut(Progress),
) -> AppResult<()> {
    require_path(&r.path)?;
    run::run(restore_job(s, r, c)?, token, on).await
}
