//! Data model shared with the UI. Nothing here holds a secret: passwords and keys
//! live in the OS keychain (see `secrets.rs`) and move only as `SecretChange`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// The database product a connection talks to (what the driver picker offers).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Driver {
    Postgres,
    Mysql,
    Sqlite,
    Mariadb,
    Cockroach,
    Redshift,
    Greenplum,
    Vertica,
    Libsql,
    CloudflareD1,
    Mssql,
    Oracle,
    Clickhouse,
    Duckdb,
    Cassandra,
    Snowflake,
    Bigquery,
    Redis,
    Mongodb,
    Dynamodb,
    Elasticsearch,
}

impl Driver {
    pub const ALL: [Driver; 21] = [
        Driver::Postgres,
        Driver::Mysql,
        Driver::Sqlite,
        Driver::Mariadb,
        Driver::Cockroach,
        Driver::Redshift,
        Driver::Greenplum,
        Driver::Vertica,
        Driver::Libsql,
        Driver::CloudflareD1,
        Driver::Mssql,
        Driver::Oracle,
        Driver::Clickhouse,
        Driver::Duckdb,
        Driver::Cassandra,
        Driver::Snowflake,
        Driver::Bigquery,
        Driver::Redis,
        Driver::Mongodb,
        Driver::Dynamodb,
        Driver::Elasticsearch,
    ];

    /// Drivers that run on one of the built-in sqlx pools; the rest are `db::engine`s.
    pub fn sqlx_dialect(self) -> Option<Dialect> {
        match self {
            Driver::Postgres
            | Driver::Cockroach
            | Driver::Redshift
            | Driver::Greenplum
            | Driver::Vertica => Some(Dialect::Postgres),
            Driver::Mysql | Driver::Mariadb => Some(Dialect::Mysql),
            Driver::Sqlite => Some(Dialect::Sqlite),
            _ => None,
        }
    }

    /// The closest built-in SQL dialect, used to split scripts and classify statements
    /// for safe mode. Engines without SQL (Redis, MongoDB…) fall back to Postgres rules.
    pub fn dialect(self) -> Dialect {
        match self {
            Driver::Libsql | Driver::CloudflareD1 => Dialect::Sqlite,
            // Backtick identifiers and backslash escapes in strings.
            Driver::Clickhouse | Driver::Bigquery => Dialect::Mysql,
            d => d.sqlx_dialect().unwrap_or(Dialect::Postgres),
        }
    }

    pub fn default_port(self) -> Option<u16> {
        match self {
            Driver::Postgres | Driver::Greenplum => Some(5432),
            Driver::Mysql | Driver::Mariadb => Some(3306),
            Driver::Cockroach => Some(26257),
            Driver::Redshift => Some(5439),
            Driver::Vertica => Some(5433),
            Driver::Mssql => Some(1433),
            Driver::Oracle => Some(1521),
            Driver::Clickhouse => Some(8123),
            Driver::Cassandra => Some(9042),
            Driver::Redis => Some(6379),
            Driver::Mongodb => Some(27017),
            Driver::Elasticsearch => Some(9200),
            Driver::Sqlite
            | Driver::Libsql
            | Driver::CloudflareD1
            | Driver::Duckdb
            | Driver::Snowflake
            | Driver::Bigquery
            | Driver::Dynamodb => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Driver::Postgres => "PostgreSQL",
            Driver::Mysql => "MySQL",
            Driver::Sqlite => "SQLite",
            Driver::Mariadb => "MariaDB",
            Driver::Cockroach => "CockroachDB",
            Driver::Redshift => "Amazon Redshift",
            Driver::Greenplum => "Greenplum",
            Driver::Vertica => "Vertica",
            Driver::Libsql => "LibSQL / Turso",
            Driver::CloudflareD1 => "Cloudflare D1",
            Driver::Mssql => "Microsoft SQL Server",
            Driver::Oracle => "Oracle",
            Driver::Clickhouse => "ClickHouse",
            Driver::Duckdb => "DuckDB",
            Driver::Cassandra => "Cassandra",
            Driver::Snowflake => "Snowflake",
            Driver::Bigquery => "BigQuery",
            Driver::Redis => "Redis",
            Driver::Mongodb => "MongoDB",
            Driver::Dynamodb => "DynamoDB",
            Driver::Elasticsearch => "Elasticsearch",
        }
    }

    /// Local files rather than servers (no host, SSH or TLS).
    pub fn is_file(self) -> bool {
        matches!(self, Driver::Sqlite | Driver::Duckdb)
    }
}

/// SQL dialect of the built-in sqlx drivers: quoting, binds, catalogs and DDL.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Dialect {
    Postgres,
    Mysql,
    Sqlite,
}

impl Dialect {
    pub fn label(self) -> &'static str {
        match self {
            Dialect::Postgres => "PostgreSQL",
            Dialect::Mysql => "MySQL",
            Dialect::Sqlite => "SQLite",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    #[default]
    Local,
    Testing,
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PasswordMode {
    /// Stored in the OS keychain.
    #[default]
    Keychain,
    /// Asked every time the connection opens; never stored.
    Ask,
    /// No password.
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    Disable,
    Allow,
    #[default]
    Prefer,
    Require,
    VerifyCa,
    /// Postgres verify-full / MySQL verify-identity.
    VerifyFull,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct DbSettings {
    /// Empty means 127.0.0.1 (resolved at connect time, never stored).
    pub host: String,
    /// Empty means the driver default port.
    pub port: Option<u16>,
    pub user: String,
    pub database: String,
    pub use_socket: bool,
    pub socket: String,
    /// SQLite and DuckDB: path to the database file.
    pub file_path: String,
    pub password_mode: PasswordMode,
    /// Driver-specific settings that are not secrets (account, region, URL, keyspace…);
    /// the keys each driver reads are listed in `drivers::fields`.
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SshSettings {
    pub enabled: bool,
    pub host: String,
    /// Empty means 22.
    pub port: Option<u16>,
    pub user: String,
    pub password_mode: PasswordMode,
    pub use_key: bool,
    /// Display name of the imported key file; its content is in the keychain.
    pub key_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TlsSettings {
    pub mode: TlsMode,
    pub ca_path: String,
    pub cert_path: String,
    pub key_path: String,
}

/// When statements need a confirmation (TablePlus "safe mode"). `Auto` follows the tag:
/// production connections confirm writes. "Safe" levels ask to type the connection name.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SafeMode {
    #[default]
    Auto,
    Silent,
    AlertWrites,
    AlertAll,
    SafeWrites,
    SafeAll,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ConnectionOptions {
    pub show_system_schemas: bool,
    pub page_size: u32,
    pub query_row_limit: u32,
    pub safe_mode: SafeMode,
    /// Refuse writes; also set on the server session (READ ONLY transactions).
    pub read_only: bool,
    /// SQL run on every new connection of the pool (SET search_path, time zone…).
    pub startup_query: String,
    /// Ping interval that keeps idle connections and SSH tunnels open; 0 = off.
    pub keep_alive_secs: u32,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            show_system_schemas: false,
            page_size: 300,
            query_row_limit: 1000,
            safe_mode: SafeMode::Auto,
            read_only: false,
            startup_query: String::new(),
            keep_alive_secs: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group_id: Option<String>,
    pub driver: Driver,
    #[serde(default)]
    pub tag: Tag,
    #[serde(default)]
    pub color: String,
    /// Custom tags (Settings › Tags) on top of the environment tag.
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub db: DbSettings,
    #[serde(default)]
    pub ssh: SshSettings,
    #[serde(default)]
    pub tls: TlsSettings,
    #[serde(default)]
    pub options: ConnectionOptions,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl Connection {
    pub fn is_production(&self) -> bool {
        self.tag == Tag::Production
    }

    pub fn db_host(&self) -> String {
        if self.db.host.trim().is_empty() {
            "127.0.0.1".into()
        } else {
            self.db.host.trim().to_string()
        }
    }

    pub fn db_port(&self) -> u16 {
        self.db.port.or(self.driver.default_port()).unwrap_or(0)
    }

    pub fn ssh_port(&self) -> u16 {
        self.ssh.port.unwrap_or(22)
    }

    /// A driver-specific setting from `db.extra` ("" when unset).
    pub fn extra(&self, key: &str) -> &str {
        self.db.extra.get(key).map(|v| v.trim()).unwrap_or("")
    }

    /// One-line summary for lists: `host:port/db`, a file path, a URL or a cloud account.
    pub fn summary(&self) -> String {
        let with_db = |mut s: String| {
            if !self.db.database.is_empty() {
                s.push('/');
                s.push_str(&self.db.database);
            }
            s
        };
        match self.driver {
            d if d.is_file() => self.db.file_path.clone(),
            Driver::Libsql => self.extra("url").to_string(),
            Driver::CloudflareD1 => with_db(format!("d1:{}", self.extra("accountId"))),
            Driver::Snowflake => with_db(self.extra("account").to_string()),
            Driver::Bigquery => with_db(self.extra("project").to_string()),
            Driver::Dynamodb => match self.extra("endpoint") {
                "" => format!("dynamodb:{}", self.extra("region")),
                endpoint => endpoint.to_string(),
            },
            _ if self.db.use_socket && !self.db.socket.is_empty() => {
                with_db(self.db.socket.clone())
            }
            _ => with_db(format!("{}:{}", self.db_host(), self.db_port())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    /// The group this one sits in (groups nest like folders); None at the top level.
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub collapsed: bool,
    /// A custom icon chosen in the group dialog: a small `data:image/…` URL.
    #[serde(default)]
    pub icon: Option<String>,
}

/// How a form changes one secret. Empty strings are valid passwords, so "keep" and
/// "clear" are explicit instead of being encoded as None / "".
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SecretChange {
    #[default]
    Keep,
    Set {
        value: String,
    },
    Clear,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SecretsInput {
    pub db_password: SecretChange,
    pub ssh_password: SecretChange,
    /// Token returned by `import_ssh_key`; the key bytes never leave Rust.
    pub ssh_key_token: Option<String>,
    pub clear_ssh_key: bool,
    pub ssh_key_passphrase: SecretChange,
}

/// Secrets typed into a prompt for one connect attempt ("Ask every time", or a
/// passphrase the keychain does not have). `save` stores them in the keychain.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PromptedSecrets {
    pub db_password: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_key_passphrase: Option<String>,
    pub save_passphrase: bool,
}

/// What the UI lists: the connection plus which secrets exist (never their values).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionView {
    #[serde(flatten)]
    pub connection: Connection,
    pub summary: String,
    pub has_db_password: bool,
    pub has_ssh_password: bool,
    pub has_ssh_key: bool,
    pub has_ssh_key_passphrase: bool,
}
