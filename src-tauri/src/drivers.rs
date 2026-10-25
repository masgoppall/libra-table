//! The driver catalog the UI builds the driver picker and connection form from, and the
//! features each driver supports (the UI hides what a driver can't do). The built-in sqlx
//! drivers are described here; each engine describes itself in `engines::<driver>`.

use serde::Serialize;

use crate::db::filter::{self, OperatorInfo};
use crate::engines;
use crate::models::{Dialect, Driver};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Relational,
    /// Speaks the protocol of another database (Postgres, MySQL or SQLite).
    Compatible,
    Analytics,
    Cloud,
    Nosql,
}

/// How the workspace presents the data.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Tables and SQL (also CQL and PartiQL-like languages).
    Sql,
    /// Collections of JSON documents (MongoDB, DynamoDB, Elasticsearch).
    Document,
    /// Keys and values (Redis).
    KeyValue,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Number,
    /// A local file; `options` holds the extensions offered by the picker.
    File,
    Select,
    Bool,
}

/// One field of the connection form. `key` is `host`, `port`, `user`, `database`,
/// `filePath` or `extra.<name>` (stored in `db.extra`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub placeholder: &'static str,
    pub required: bool,
    pub options: Vec<&'static str>,
    pub help: &'static str,
}

impl Field {
    pub const fn text(key: &'static str, label: &'static str, placeholder: &'static str) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Text,
            placeholder,
            required: false,
            options: Vec::new(),
            help: "",
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    pub fn kind(mut self, kind: FieldKind, options: Vec<&'static str>) -> Self {
        self.kind = kind;
        self.options = options;
        self
    }
}

/// The host, port, user and database fields most server drivers share.
pub fn server_fields(database_label: &'static str) -> Vec<Field> {
    vec![
        Field::text("host", "Host", "127.0.0.1"),
        Field::text("port", "Port", "").kind(FieldKind::Number, Vec::new()),
        Field::text("user", "User", ""),
        Field::text("database", database_label, ""),
    ]
}

/// The secret the connection keeps in the keychain (`db_password`), as the form labels it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Secret {
    pub label: &'static str,
    /// A pasted key file (BigQuery service account) rather than a one-line password.
    pub multiline: bool,
    pub optional: bool,
}

impl Secret {
    pub const fn password() -> Self {
        Self {
            label: "Password",
            multiline: false,
            optional: true,
        }
    }

    pub const fn named(label: &'static str) -> Self {
        Self {
            label,
            multiline: false,
            optional: false,
        }
    }
}

/// How much of the table structure a driver can show or change.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Structure {
    None,
    View,
    Edit,
}

/// What the UI may offer for a driver.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    /// False while the driver is still being built: the picker shows it as unavailable.
    pub ready: bool,
    pub has_schemas: bool,
    pub switch_database: bool,
    pub query: bool,
    pub edit_rows: bool,
    pub structure: Structure,
    pub create_table: bool,
    pub create_database: bool,
    /// Functions, procedures, sequences, triggers… (db::objects).
    pub schema_objects: bool,
    pub processes: bool,
    pub users: bool,
    pub backup: bool,
    pub import: bool,
    pub diagram: bool,
    pub explain: bool,
    pub metrics: bool,
    pub quick_look: bool,
}

impl Features {
    /// A driver that isn't built yet.
    pub fn none() -> Self {
        Self {
            ready: false,
            has_schemas: false,
            switch_database: false,
            query: false,
            edit_rows: false,
            structure: Structure::None,
            create_table: false,
            create_database: false,
            schema_objects: false,
            processes: false,
            users: false,
            backup: false,
            import: false,
            diagram: false,
            explain: false,
            metrics: false,
            quick_look: false,
        }
    }

    /// Everything the built-in drivers do.
    fn full(dialect: Dialect) -> Self {
        Self {
            ready: true,
            has_schemas: dialect == Dialect::Postgres,
            switch_database: dialect != Dialect::Sqlite,
            query: true,
            edit_rows: true,
            structure: Structure::Edit,
            create_table: true,
            create_database: dialect != Dialect::Sqlite,
            schema_objects: true,
            processes: dialect != Dialect::Sqlite,
            users: dialect != Dialect::Sqlite,
            // SQLite backups copy the file; the command line is sqlite3.
            backup: true,
            import: true,
            diagram: true,
            explain: true,
            metrics: true,
            quick_look: true,
        }
    }
}

/// One entry of the driver picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverInfo {
    pub id: Driver,
    pub label: &'static str,
    pub group: Group,
    pub kind: Kind,
    /// Quoting, formatting and highlighting in the UI: postgres, mysql, sqlite, mssql,
    /// oracle, clickhouse, duckdb, cql, snowflake, bigquery; None without SQL.
    pub sql_dialect: Option<&'static str>,
    /// The editor language: sql, redis, mongo or json.
    pub query_language: &'static str,
    pub default_port: Option<u16>,
    pub fields: Vec<Field>,
    pub secret: Option<Secret>,
    pub ssh: bool,
    pub tls: bool,
    /// One line under the name in the picker.
    pub note: &'static str,
}

impl DriverInfo {
    /// A SQL server driver with the usual host/port/user/database fields.
    pub fn server(id: Driver, group: Group, sql_dialect: &'static str) -> Self {
        Self {
            id,
            label: id.label(),
            group,
            kind: Kind::Sql,
            sql_dialect: Some(sql_dialect),
            query_language: "sql",
            default_port: id.default_port(),
            fields: server_fields("Database"),
            secret: Some(Secret::password()),
            ssh: true,
            tls: true,
            note: "",
        }
    }
}

/// Drivers on the built-in sqlx pools.
fn sqlx_info(id: Driver, dialect: Dialect) -> DriverInfo {
    let (group, note) = match id {
        Driver::Postgres | Driver::Mysql | Driver::Sqlite => (Group::Relational, ""),
        Driver::Mariadb => (Group::Relational, "MySQL protocol"),
        Driver::Redshift => (Group::Cloud, "PostgreSQL protocol"),
        _ => (Group::Compatible, "PostgreSQL protocol"),
    };
    let sql = match dialect {
        Dialect::Postgres => "postgres",
        Dialect::Mysql => "mysql",
        Dialect::Sqlite => "sqlite",
    };
    let mut info = DriverInfo::server(id, group, sql);
    info.note = note;
    if dialect == Dialect::Sqlite {
        info.fields = vec![Field::text("filePath", "Database file", "")
            .kind(FieldKind::File, vec!["db", "sqlite"])];
        info.secret = None;
        info.ssh = false;
        info.tls = false;
    }
    info
}

/// Every driver, in picker order.
pub fn list() -> Vec<DriverInfo> {
    Driver::ALL.iter().map(|&d| info(d)).collect()
}

pub fn info(driver: Driver) -> DriverInfo {
    match driver.sqlx_dialect() {
        Some(dialect) => sqlx_info(driver, dialect),
        None => engines::info(driver),
    }
}

pub fn features(driver: Driver) -> Features {
    match driver.sqlx_dialect() {
        // Vertica only resembles the Postgres protocol; not verified yet.
        _ if driver == Driver::Vertica => Features::none(),
        Some(dialect) => Features::full(dialect),
        None => engines::features(driver),
    }
}

pub fn operators(driver: Driver) -> Vec<OperatorInfo> {
    match driver.sqlx_dialect() {
        Some(dialect) => filter::operators(dialect),
        None => engines::operators(driver),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_driver_is_listed_once_with_a_label() {
        let all = list();
        assert_eq!(all.len(), Driver::ALL.len());
        for d in &all {
            assert!(!d.label.is_empty());
            assert!(all.iter().filter(|o| o.id == d.id).count() == 1);
        }
    }

    #[test]
    fn built_in_drivers_are_ready() {
        for d in [
            Driver::Postgres,
            Driver::Mysql,
            Driver::Sqlite,
            Driver::Mariadb,
        ] {
            assert!(features(d).ready);
        }
        assert!(!features(Driver::Sqlite).processes);
    }
}
