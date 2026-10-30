//! One TablePlus connection (the keys of its `Connections.plist`, which its exports
//! carry too) as a LibraTable connection plus the secrets that came with it.

use serde_json::{Map, Value};

use crate::db;
use crate::models::{
    Connection, ConnectionOptions, DbSettings, Driver, Group, PasswordMode, SshSettings, Tag,
    TlsMode, TlsSettings,
};
use crate::storage::secrets::SecretKind;

use super::payload;

/// TablePlus' default status color: no color of its own.
const DEFAULT_COLOR: &str = "#686B6F";

pub struct Converted {
    pub connection: Connection,
    pub secrets: Vec<(SecretKind, String)>,
}

/// Typed reads of a TablePlus dictionary; numbers often come as strings ("3306").
pub struct Entry<'a>(pub &'a Map<String, Value>);

impl Entry<'_> {
    fn get(&self, key: &str) -> Option<&Value> {
        payload::key(self.0, key)
    }

    pub fn text(&self, key: &str) -> String {
        match self.get(key) {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        }
    }

    pub fn int(&self, key: &str) -> i64 {
        match self.get(key) {
            Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
            Some(Value::Bool(b)) => *b as i64,
            _ => 0,
        }
    }

    pub fn flag(&self, key: &str) -> bool {
        self.int(key) != 0 || matches!(self.text(key).as_str(), "true" | "YES" | "yes")
    }

    fn port(&self, key: &str) -> Option<u16> {
        u16::try_from(self.int(key)).ok().filter(|p| *p != 0)
    }

    /// A secret that is set: TablePlus writes "" for none.
    fn secret(&self, key: &str) -> Option<String> {
        match self.get(key) {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }

    fn list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| v.as_str().unwrap_or("").trim().to_string())
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// TablePlus driver names (`Driver`), compared without case or punctuation.
pub fn driver(name: &str) -> Option<Driver> {
    let key: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    Some(match key.as_str() {
        "mysql" => Driver::Mysql,
        "mariadb" => Driver::Mariadb,
        "postgresql" | "postgres" => Driver::Postgres,
        "sqlite" => Driver::Sqlite,
        "redis" => Driver::Redis,
        "microsoftsqlserver" | "sqlserver" | "mssql" => Driver::Mssql,
        "cockroach" | "cockroachdb" => Driver::Cockroach,
        "redshift" | "amazonredshift" => Driver::Redshift,
        "greenplum" => Driver::Greenplum,
        "vertica" => Driver::Vertica,
        "oracle" => Driver::Oracle,
        "clickhouse" | "httpclickhouse" => Driver::Clickhouse,
        "duckdb" | "dudb" => Driver::Duckdb,
        "cassandra" => Driver::Cassandra,
        "snowflake" => Driver::Snowflake,
        "bigquery" => Driver::Bigquery,
        "mongo" | "mongodb" | "mongolegacy" => Driver::Mongodb,
        "dynamodb" => Driver::Dynamodb,
        "elasticsearch" => Driver::Elasticsearch,
        "libsql" | "tblibsql" => Driver::Libsql,
        "cloudflared1" | "d1" => Driver::CloudflareD1,
        _ => return None,
    })
}

/// "Store in keychain", "Ask every time", "No password", in the form's order.
fn password_mode(n: i64) -> PasswordMode {
    match n {
        1 => PasswordMode::Ask,
        2 => PasswordMode::None,
        _ => PasswordMode::Keychain,
    }
}

/// The SSL mode menu: PREFERRED, DISABLED, REQUIRED, then (Postgres family) ALLOW,
/// VERIFY-CA, VERIFY-FULL or (MySQL family) VERIFY-CA, VERIFY-IDENTITY.
fn tls_mode(driver: Driver, n: i64) -> TlsMode {
    let mysql = matches!(driver, Driver::Mysql | Driver::Mariadb);
    match (n, mysql) {
        (1, _) => TlsMode::Disable,
        (2, _) => TlsMode::Require,
        (3, false) => TlsMode::Allow,
        (3, true) | (4, false) => TlsMode::VerifyCa,
        (4, true) | (5, false) => TlsMode::VerifyFull,
        _ => TlsMode::Prefer,
    }
}

fn tag(env: &str) -> Tag {
    match env.to_ascii_lowercase().as_str() {
        "production" => Tag::Production,
        "staging" => Tag::Staging,
        "development" => Tag::Development,
        "testing" => Tag::Testing,
        _ => Tag::Local,
    }
}

fn db_settings(e: &Entry, driver: Driver) -> DbSettings {
    let mut db = DbSettings {
        host: e.text("DatabaseHost"),
        port: e.port("DatabasePort"),
        user: e.text("DatabaseUser"),
        database: e.text("DatabaseName"),
        use_socket: e.flag("isUseSocket"),
        socket: e.text("DatabaseSocket"),
        file_path: e.text("DatabasePath"),
        password_mode: password_mode(e.int("DatabasePasswordMode")),
        ..DbSettings::default()
    };
    let mut extra = |key: &str, value: String| {
        if !value.is_empty() {
            db.extra.insert(key.into(), value);
        }
    };
    match driver {
        Driver::Snowflake => {
            extra("account", e.text("DatabaseHost"));
            extra("warehouse", e.text("DatabaseWarehouse"));
            extra("role", e.text("DatabaseUserRole"));
        }
        Driver::Libsql => extra("url", e.text("DatabaseHost")),
        _ => {}
    }
    if driver == Driver::Snowflake || driver == Driver::Libsql {
        db.host.clear();
    }
    db
}

fn ssh_settings(e: &Entry) -> SshSettings {
    let use_key = e.flag("isUsePrivateKey");
    SshSettings {
        enabled: e.flag("isOverSSH"),
        host: e.text("ServerAddress"),
        port: e.port("ServerPort"),
        user: e.text("ServerUser"),
        password_mode: password_mode(e.int("ServerPasswordMode")),
        use_key,
        key_name: if use_key {
            e.text("ServerPrivateKeyName")
        } else {
            String::new()
        },
    }
}

fn options(e: &Entry) -> ConnectionOptions {
    let defaults = ConnectionOptions::default();
    let limit = |key: &str, fallback: u32, max: u32| match e.int(key) {
        n if n > 0 => n.min(max as i64) as u32,
        _ => fallback,
    };
    ConnectionOptions {
        show_system_schemas: e.flag("ShowSystemSchemas"),
        page_size: limit(
            "LimitRowsReturned",
            defaults.page_size,
            db::data::MAX_PAGE_SIZE,
        ),
        query_row_limit: limit("LimitQueryRowsReturned", defaults.query_row_limit, u32::MAX),
        startup_query: e.text("StartupCommands"),
        ..defaults
    }
}

fn tls_settings(e: &Entry, driver: Driver) -> TlsSettings {
    // "Key…", "Cert…", "CA Cert…".
    let paths = e.list("TlsKeyPaths");
    let path = |i: usize| paths.get(i).cloned().unwrap_or_default();
    TlsSettings {
        mode: tls_mode(driver, e.int("tLSMode")),
        key_path: path(0),
        cert_path: path(1),
        ca_path: path(2),
    }
}

/// The private key of an SSH tunnel: embedded in the export, or the file of that name
/// in `~/.ssh` when TablePlus only remembered the name.
fn ssh_key(e: &Entry, ssh: &SshSettings) -> Option<String> {
    if !ssh.use_key {
        return None;
    }
    if let Some(pem) = e
        .secret("ServerPrivateKeyData")
        .filter(|k| k.contains("PRIVATE KEY"))
    {
        return Some(pem);
    }
    let name = ssh.key_name.trim_start_matches("~/");
    let path = if name.starts_with('/') {
        name.into()
    } else {
        dirs::home_dir()?.join(".ssh").join(name)
    };
    std::fs::read_to_string(path)
        .ok()
        .filter(|k| k.contains("PRIVATE KEY"))
}

/// `Err` holds why the entry can't come over (shown next to its name).
pub fn connection(e: &Entry) -> Result<Converted, String> {
    let name = e.text("ConnectionName");
    let tp_driver = e.text("Driver");
    let driver = driver(&tp_driver).ok_or_else(|| match tp_driver.as_str() {
        "" => "the entry has no database driver".to_string(),
        d => format!("{d} isn't supported by LibraTable"),
    })?;
    let db = db_settings(e, driver);
    let ssh = ssh_settings(e);
    let color = e.text("statusColor");
    let connection = Connection {
        id: String::new(),
        name: if name.is_empty() {
            driver.label().into()
        } else {
            name
        },
        group_id: Some(e.text("GroupID")).filter(|g| !g.is_empty()),
        driver,
        tag: tag(&e.text("Enviroment")),
        labels: Vec::new(),
        color: if color.eq_ignore_ascii_case(DEFAULT_COLOR) {
            String::new()
        } else {
            color
        },
        tls: tls_settings(e, driver),
        options: options(e),
        created_at: 0,
        updated_at: 0,
        db,
        ssh,
    };
    let mut secrets = Vec::new();
    let mut add = |kind, value: Option<String>| {
        if let Some(v) = value {
            secrets.push((kind, v));
        }
    };
    if connection.db.password_mode == PasswordMode::Keychain {
        add(SecretKind::DbPassword, e.secret("DatabasePassword"));
    }
    if connection.ssh.enabled {
        if connection.ssh.password_mode == PasswordMode::Keychain {
            add(SecretKind::SshPassword, e.secret("ServerPassword"));
        }
        add(SecretKind::SshKey, ssh_key(e, &connection.ssh));
        add(SecretKind::SshKeyPassphrase, e.secret("PrivateKeyPassword"));
    }
    Ok(Converted {
        connection,
        secrets,
    })
}

/// A `ConnectionGroups.plist` entry: `ID`, `Name`, `GroupID` (parent), `IsExpaned`.
pub fn group(e: &Entry) -> Option<Group> {
    let id = e.text("ID");
    let name = e.text("Name");
    (!id.is_empty() && !name.is_empty()).then(|| Group {
        id,
        name,
        parent_id: Some(e.text("GroupID")).filter(|g| !g.is_empty()),
        collapsed: !e.flag("IsExpaned"),
        icon: None,
    })
}
