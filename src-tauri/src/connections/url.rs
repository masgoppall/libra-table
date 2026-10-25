//! Connection URLs (CON-8, CON-9).
//!
//! Accepted:
//! * `postgresql://` / `postgres://user:pass@host:port/db?sslmode=require`
//! * `mysql://user:pass@host:port/db?ssl-mode=REQUIRED`
//! * `postgresql+ssh://sshuser[:sshpass]@sshhost[:sshport]/dbuser[:dbpass]@dbhost[:dbport]/db`
//!   (the TablePlus form)
//! * `sqlite:///absolute/path.db`
//! * `libratable://open?url=<percent-encoded URL>` (deep link wrapper)
//!
//! Exported URLs never contain a password.

use serde::Serialize;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::models::{
    Connection, ConnectionOptions, DbSettings, Driver, SshSettings, Tag, TlsMode, TlsSettings,
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDraft {
    pub connection: Connection,
    /// Passwords typed into the URL; the form saves them to the keychain.
    pub db_password: Option<String>,
    pub ssh_password: Option<String>,
}

fn blank(driver: Driver) -> Connection {
    Connection {
        id: String::new(),
        name: String::new(),
        group_id: None,
        driver,
        tag: Tag::Local,
        color: String::new(),
        db: DbSettings::default(),
        ssh: SshSettings::default(),
        tls: TlsSettings::default(),
        options: ConnectionOptions::default(),
        created_at: 0,
        updated_at: 0,
    }
}

/// Percent-decodes on bytes (slicing the str could split a UTF-8 character).
fn decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn parse(input: &str) -> AppResult<ConnectionDraft> {
    let input = input.trim();
    if let Some(rest) = input.strip_prefix("libratable://") {
        return parse(&deep_link_target(rest)?);
    }
    if let Some(rest) = input.strip_prefix("postgresql+ssh://") {
        return parse_pg_ssh(rest);
    }
    if let Some(path) = input.strip_prefix("sqlite://") {
        return Ok(parse_sqlite(path));
    }
    parse_server_url(input)
}

/// The connection URL wrapped in a `libratable://open?url=…` link.
fn deep_link_target(rest: &str) -> AppResult<String> {
    let u = Url::parse(&format!("libratable://{rest}"))
        .map_err(|e| AppError::validation(format!("Invalid link: {e}")))?;
    u.query_pairs()
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| AppError::validation("The link has no connection URL."))
}

fn parse_sqlite(path: &str) -> ConnectionDraft {
    let mut c = blank(Driver::Sqlite);
    c.db.file_path = decode(path);
    c.name = std::path::Path::new(&c.db.file_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "SQLite".into());
    ConnectionDraft {
        connection: c,
        db_password: None,
        ssh_password: None,
    }
}

/// `postgresql://` and `mysql://` URLs.
fn parse_server_url(input: &str) -> AppResult<ConnectionDraft> {
    let u = Url::parse(input)
        .map_err(|e| AppError::validation(format!("Invalid connection URL: {e}")))?;
    let driver = match u.scheme() {
        "postgresql" | "postgres" => Driver::Postgres,
        "mysql" | "mariadb" => Driver::Mysql,
        other => {
            return Err(AppError::validation(format!(
                "Unsupported URL scheme \"{other}\"."
            )))
        }
    };
    let mut c = blank(driver);
    c.db.host = u
        .host_str()
        .unwrap_or("")
        .trim_matches(|ch| ch == '[' || ch == ']')
        .to_string();
    c.db.port = u.port();
    c.db.user = decode(u.username());
    c.db.database = decode(u.path().trim_start_matches('/'));
    let db_password = u.password().map(decode);
    apply_query(&mut c, &u);
    if c.name.is_empty() {
        c.name = default_name(&c);
    }
    Ok(ConnectionDraft {
        connection: c,
        db_password,
        ssh_password: None,
    })
}

/// Options in the query string: TLS mode, socket, colour, tag and name.
fn apply_query(c: &mut Connection, u: &Url) {
    for (k, v) in u.query_pairs() {
        let v = v.to_ascii_lowercase();
        match k.as_ref() {
            "sslmode" | "ssl-mode" | "ssl_mode" => c.tls.mode = tls_mode(&v),
            "host" if c.db.host.is_empty() => {
                // postgres://user@/db?host=/var/run/postgresql
                c.db.use_socket = true;
                c.db.socket = v;
            }
            "statusColor" | "statuscolor" => c.color = v,
            "env" | "tag" => c.tag = tag(&v),
            "name" => c.name = v,
            _ => {}
        }
    }
}

fn parse_pg_ssh(rest: &str) -> AppResult<ConnectionDraft> {
    // sshuser[:sshpass]@sshhost[:sshport]/dbuser[:dbpass]@dbhost[:dbport]/db
    let (ssh_part, db_part) = rest.split_once('/').ok_or_else(|| {
        AppError::validation("Expected postgresql+ssh://ssh_user@ssh_host/db_user@db_host/database")
    })?;
    let ssh = Url::parse(&format!("ssh://{ssh_part}"))
        .map_err(|e| AppError::validation(format!("Invalid SSH part: {e}")))?;
    let db = Url::parse(&format!("postgresql://{db_part}"))
        .map_err(|e| AppError::validation(format!("Invalid database part: {e}")))?;
    let mut draft = parse(db.as_str())?;
    let c = &mut draft.connection;
    c.ssh.enabled = true;
    c.ssh.host = ssh.host_str().unwrap_or("").to_string();
    c.ssh.port = ssh.port();
    c.ssh.user = decode(ssh.username());
    draft.ssh_password = ssh.password().map(decode);
    if c.ssh.host.is_empty() {
        return Err(AppError::validation("The SSH host is missing."));
    }
    c.name = format!("{} via {}", default_name(c), c.ssh.host);
    Ok(draft)
}

fn tls_mode(v: &str) -> TlsMode {
    match v {
        "disable" | "disabled" | "false" => TlsMode::Disable,
        "allow" => TlsMode::Allow,
        "require" | "required" | "true" => TlsMode::Require,
        "verify-ca" | "verify_ca" => TlsMode::VerifyCa,
        "verify-full" | "verify_full" | "verify-identity" | "verify_identity" => {
            TlsMode::VerifyFull
        }
        _ => TlsMode::Prefer,
    }
}

fn tag(v: &str) -> Tag {
    match v {
        "testing" | "test" => Tag::Testing,
        "development" | "dev" => Tag::Development,
        "staging" | "stage" => Tag::Staging,
        "production" | "prod" => Tag::Production,
        _ => Tag::Local,
    }
}

fn default_name(c: &Connection) -> String {
    let host = if c.db.host.is_empty() {
        "localhost".to_string()
    } else {
        c.db.host.clone()
    };
    if c.db.database.is_empty() {
        host
    } else {
        format!("{} @ {host}", c.db.database)
    }
}

/// URL for "Copy as URL" — without passwords (CON-9).
pub fn to_url(c: &Connection) -> String {
    let user = if c.db.user.is_empty() {
        String::new()
    } else {
        format!("{}@", encode(&c.db.user))
    };
    let host = if c.db.host.is_empty() {
        "127.0.0.1".to_string()
    } else {
        c.db.host.clone()
    };
    let port = c.db.port.map(|p| format!(":{p}")).unwrap_or_default();
    let db = encode(&c.db.database);
    let sslmode = match c.tls.mode {
        TlsMode::Disable => "disable",
        TlsMode::Allow => "allow",
        TlsMode::Prefer => "prefer",
        TlsMode::Require => "require",
        TlsMode::VerifyCa => "verify-ca",
        TlsMode::VerifyFull => "verify-full",
    };
    match c.driver {
        Driver::Sqlite => format!("sqlite://{}", c.db.file_path),
        Driver::Postgres if c.ssh.enabled => {
            let ssh_user = if c.ssh.user.is_empty() {
                String::new()
            } else {
                format!("{}@", encode(&c.ssh.user))
            };
            let ssh_port = c.ssh.port.map(|p| format!(":{p}")).unwrap_or_default();
            format!(
                "postgresql+ssh://{ssh_user}{}{ssh_port}/{user}{host}{port}/{db}?sslmode={sslmode}",
                c.ssh.host
            )
        }
        Driver::Postgres => format!("postgresql://{user}{host}{port}/{db}?sslmode={sslmode}"),
        Driver::Mysql => format!("mysql://{user}{host}{port}/{db}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_url_with_password_and_sslmode() {
        let d = parse("postgresql://app%40corp:s3cr%2Ft@db.example.com:6543/sales?sslmode=require")
            .unwrap();
        let c = d.connection;
        assert_eq!(c.driver, Driver::Postgres);
        assert_eq!(
            (
                c.db.user.as_str(),
                c.db.host.as_str(),
                c.db.port,
                c.db.database.as_str()
            ),
            ("app@corp", "db.example.com", Some(6543), "sales")
        );
        assert_eq!(c.tls.mode, TlsMode::Require);
        assert_eq!(d.db_password.as_deref(), Some("s3cr/t"));
        assert_eq!(c.name, "sales @ db.example.com");
    }

    #[test]
    fn mysql_and_postgres_alias() {
        assert_eq!(
            parse("mysql://root@127.0.0.1/shop")
                .unwrap()
                .connection
                .driver,
            Driver::Mysql
        );
        let p = parse("postgres://localhost").unwrap().connection;
        assert_eq!((p.db.port, p.db.database.as_str()), (None, ""));
    }

    #[test]
    fn tableplus_ssh_form() {
        let d = parse(
            "postgresql+ssh://deploy:pw@bastion.example.com:2222/app:dbpw@10.0.0.5:5432/main",
        )
        .unwrap();
        let c = &d.connection;
        assert!(c.ssh.enabled);
        assert_eq!(
            (c.ssh.user.as_str(), c.ssh.host.as_str(), c.ssh.port),
            ("deploy", "bastion.example.com", Some(2222))
        );
        assert_eq!(
            (
                c.db.user.as_str(),
                c.db.host.as_str(),
                c.db.port,
                c.db.database.as_str()
            ),
            ("app", "10.0.0.5", Some(5432), "main")
        );
        assert_eq!(
            (d.ssh_password.as_deref(), d.db_password.as_deref()),
            (Some("pw"), Some("dbpw"))
        );
    }

    #[test]
    fn deep_link_wrapper_and_sqlite() {
        let inner = "postgresql://u@h/db";
        let link = format!("libratable://open?url={}", encode(inner));
        assert_eq!(parse(&link).unwrap().connection.db.database, "db");
        let s = parse("sqlite:///Users/me/data/app.db").unwrap().connection;
        assert_eq!(
            (s.driver, s.db.file_path.as_str(), s.name.as_str()),
            (Driver::Sqlite, "/Users/me/data/app.db", "app.db")
        );
    }

    #[test]
    fn rejects_unknown_schemes() {
        assert!(parse("ftp://x").is_err());
        assert!(parse("not a url").is_err());
    }

    #[test]
    fn exported_url_has_no_password_and_roundtrips() {
        let mut d = parse(concat!(
            "postgresql+ssh://deploy:pw@bastion:2222/",
            "app:dbpw@10.0.0.5:5432/main?sslmode=verify-full"
        ))
        .unwrap();
        d.connection.tls.mode = TlsMode::VerifyFull;
        let url = to_url(&d.connection);
        assert!(!url.contains("pw"));
        let back = parse(&url).unwrap().connection;
        assert_eq!(back.ssh.host, "bastion");
        assert_eq!(back.db.database, "main");
        assert_eq!(back.tls.mode, TlsMode::VerifyFull);
    }
}
