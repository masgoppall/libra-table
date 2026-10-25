//! "Open in command line": the psql / mysql / sqlite3 command for a connection, and
//! opening it in Terminal. The command never holds the password: the client asks for it.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::models::{Dialect, TlsMode};

use super::endpoint::{pg_ssl_mode, Endpoint};

/// Quotes a word for a POSIX shell when it needs it.
pub fn shell_quote(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c));
    if safe {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The client command as one shell line. `program` is the tool's full path when it
/// was found, else its name; `asks_password` adds mysql's `-p` (psql asks by itself).
pub fn command_line(e: &Endpoint, program: &str, mariadb: bool, asks_password: bool) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut env = String::new();
    match e.driver {
        Dialect::Postgres => {
            if e.tls.mode != TlsMode::Prefer {
                env = format!("PGSSLMODE={} ", pg_ssl_mode(e.tls.mode));
            }
            words.push(program.into());
            words.extend(e.conn_args(false));
            if !e.database.is_empty() {
                words.extend(["-d".into(), e.database.clone()]);
            }
        }
        Dialect::Mysql => {
            words.push(program.into());
            words.extend(e.conn_args(mariadb));
            if asks_password {
                words.push("-p".into());
            }
            if !e.database.is_empty() {
                words.push(e.database.clone());
            }
        }
        Dialect::Sqlite => {
            words.push(program.into());
            words.push(e.file.clone());
        }
    }
    let line: Vec<String> = words.iter().map(|w| shell_quote(w)).collect();
    format!("{env}{}", line.join(" "))
}

/// A `.command` file Terminal runs: it deletes itself, then becomes the client.
pub fn terminal_script(command: &str) -> String {
    format!("#!/bin/sh\nrm -f \"$0\"\nclear\nexec {command}\n")
}

/// Writes the script to the temp folder and opens it with Terminal (macOS).
pub fn open_in_terminal(command: &str, dir: &Path) -> AppResult<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err(AppError::validation(
            "Opening Terminal is only available on macOS.",
        ));
    }
    let path = dir.join(format!("libratable-{}.command", uuid::Uuid::new_v4()));
    std::fs::write(&path, terminal_script(command))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    }
    let status = std::process::Command::new("open")
        .args(["-a", "Terminal"])
        .arg(&path)
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_file(&path);
        return Err(AppError::internal("Terminal could not be opened."));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::super::endpoint::tests::endpoint;
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("/usr/bin/psql"), "/usr/bin/psql");
        assert_eq!(shell_quote("my db"), "'my db'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn commands_have_no_password() {
        let pg = endpoint(Dialect::Postgres);
        let line = command_line(&pg, "psql", false, true);
        assert_eq!(line, "psql -h 127.0.0.1 -p 5432 -U admin -d shop");
        let my = Endpoint {
            port: 3306,
            database: "my shop".into(),
            ..endpoint(Dialect::Mysql)
        };
        let line = command_line(&my, "/opt/mysql", false, true);
        assert_eq!(
            line,
            "/opt/mysql -h 127.0.0.1 -P 3306 --protocol=TCP -u admin -p 'my shop'"
        );
        assert!(!line.contains("s3cret"));
        let lite = Endpoint {
            file: "/x/a b.db".into(),
            ..endpoint(Dialect::Sqlite)
        };
        assert_eq!(
            command_line(&lite, "sqlite3", false, false),
            "sqlite3 '/x/a b.db'"
        );
        let tls = Endpoint {
            tls: crate::models::TlsSettings {
                mode: TlsMode::Require,
                ..Default::default()
            },
            ..endpoint(Dialect::Postgres)
        };
        assert!(command_line(&tls, "psql", false, false).starts_with("PGSSLMODE=require psql"));
        let script = terminal_script("psql -d shop");
        assert!(script.starts_with("#!/bin/sh\n") && script.ends_with("exec psql -d shop\n"));
    }
}
