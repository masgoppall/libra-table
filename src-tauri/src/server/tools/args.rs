//! What the backup and restore dialogs ask for, turned into pg_dump / mysqldump /
//! psql / pg_restore / mysql arguments. Values are separate arguments (no shell), and
//! object names are quoted as pg_dump patterns so `*` or `.` in a name stay literal.

use serde::Deserialize;

use crate::models::Driver;

use super::endpoint::Endpoint;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DumpFormat {
    /// A SQL script.
    #[default]
    Plain,
    /// pg_dump's compressed archive, restored with pg_restore.
    Custom,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DumpContent {
    #[default]
    All,
    SchemaOnly,
    DataOnly,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TableRef {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct BackupRequest {
    pub path: String,
    pub format: DumpFormat,
    pub content: DumpContent,
    /// Postgres: only these schemas (empty: all).
    pub schemas: Vec<String>,
    /// Only these tables (empty: all).
    pub tables: Vec<TableRef>,
    pub no_owner: bool,
    pub no_privileges: bool,
    /// Postgres: DROP … IF EXISTS before each CREATE.
    pub clean: bool,
    /// Postgres: CREATE DATABASE in the dump. MySQL: `--databases` (CREATE + USE).
    pub create_database: bool,
    pub single_transaction: bool,
    pub routines: bool,
    pub triggers: bool,
    pub events: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct RestoreRequest {
    pub path: String,
    pub stop_on_error: bool,
    /// psql / pg_restore: all or nothing.
    pub single_transaction: bool,
    /// pg_restore: drop objects before recreating them.
    pub clean: bool,
    /// pg_restore: don't set object owners.
    pub no_owner: bool,
}

/// `"name"` with inner quotes doubled: a pg_dump pattern that matches only `name`.
pub fn pg_pattern(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// libpq reads a `--dbname` with `=` in it (or a URL) as a connection string: such a
/// name is given as `dbname='…'` instead.
pub fn pg_dbname(db: &str) -> String {
    if db.contains('=') || db.starts_with("postgres://") || db.starts_with("postgresql://") {
        format!("dbname='{}'", db.replace('\\', "\\\\").replace('\'', "\\'"))
    } else {
        db.to_string()
    }
}

pub fn pg_dump(e: &Endpoint, r: &BackupRequest) -> Vec<String> {
    let mut a = e.conn_args(false);
    a.push(format!("--dbname={}", pg_dbname(&e.database)));
    a.push("--verbose".into());
    a.push(match r.format {
        DumpFormat::Plain => "--format=plain".into(),
        DumpFormat::Custom => "--format=custom".into(),
    });
    match r.content {
        DumpContent::All => {}
        DumpContent::SchemaOnly => a.push("--schema-only".into()),
        DumpContent::DataOnly => a.push("--data-only".into()),
    }
    for s in &r.schemas {
        a.push(format!("--schema={}", pg_pattern(s)));
    }
    for t in &r.tables {
        a.push(format!(
            "--table={}.{}",
            pg_pattern(&t.schema),
            pg_pattern(&t.name)
        ));
    }
    let flags = [
        (r.no_owner, "--no-owner"),
        (r.no_privileges, "--no-privileges"),
        (r.create_database, "--create"),
    ];
    a.extend(flags.iter().filter(|f| f.0).map(|f| f.1.to_string()));
    if r.clean {
        a.extend(["--clean".into(), "--if-exists".into()]);
    }
    a.push(format!("--file={}", r.path));
    a
}

/// `--no-tablespaces` spares the PROCESS privilege a single-database dump doesn't need;
/// without `--set-gtid-purged=OFF` a GTID server writes GTID_PURGED, which fails on
/// restore. `server_mariadb` with a MySQL 8+ tool: MariaDB has no COLUMN_STATISTICS.
pub fn mysqldump(
    e: &Endpoint,
    r: &BackupRequest,
    tool: (u32, bool),
    server_mariadb: bool,
) -> Vec<String> {
    let (major, mariadb) = tool;
    let mut a = e.conn_args(mariadb);
    a.extend(
        [
            "--default-character-set=utf8mb4",
            "--verbose",
            "--no-tablespaces",
        ]
        .map(String::from),
    );
    if !mariadb {
        a.push("--set-gtid-purged=OFF".into());
    }
    if !mariadb && major >= 8 && server_mariadb {
        a.push("--column-statistics=0".into());
    }
    let flags = [
        (r.single_transaction, "--single-transaction"),
        (r.routines, "--routines"),
        (r.events, "--events"),
        (!r.triggers, "--skip-triggers"),
        (r.content == DumpContent::SchemaOnly, "--no-data"),
        (r.content == DumpContent::DataOnly, "--no-create-info"),
    ];
    a.extend(flags.iter().filter(|f| f.0).map(|f| f.1.to_string()));
    a.push(format!("--result-file={}", r.path));
    let tables: Vec<String> = r.tables.iter().map(|t| t.name.clone()).collect();
    if r.create_database && tables.is_empty() {
        a.extend(["--databases".into(), e.database.clone()]);
    } else {
        a.push(e.database.clone());
        a.extend(tables);
    }
    a
}

/// psql reading the script from stdin (so progress can be measured in bytes).
pub fn psql(e: &Endpoint, r: &RestoreRequest) -> Vec<String> {
    let mut a = e.conn_args(false);
    a.push(format!("--dbname={}", pg_dbname(&e.database)));
    // No ~/.psqlrc, no command tags on stdout.
    a.extend(["--no-psqlrc".into(), "--quiet".into()]);
    if r.stop_on_error {
        a.extend(["--set".into(), "ON_ERROR_STOP=1".into()]);
    }
    if r.single_transaction {
        a.push("--single-transaction".into());
    }
    a
}

pub fn pg_restore(e: &Endpoint, r: &RestoreRequest) -> Vec<String> {
    let mut a = e.conn_args(false);
    a.push(format!("--dbname={}", pg_dbname(&e.database)));
    a.push("--verbose".into());
    let flags = [
        (r.stop_on_error, "--exit-on-error"),
        (r.single_transaction, "--single-transaction"),
        (r.no_owner, "--no-owner"),
    ];
    a.extend(flags.iter().filter(|f| f.0).map(|f| f.1.to_string()));
    if r.clean {
        a.extend(["--clean".into(), "--if-exists".into()]);
    }
    a.push(r.path.clone());
    a
}

/// mysql reading the script from stdin; it stops at the first error unless `--force`.
pub fn mysql(e: &Endpoint, r: &RestoreRequest, mariadb: bool) -> Vec<String> {
    let mut a = e.conn_args(mariadb);
    a.push("--default-character-set=utf8mb4".into());
    if !r.stop_on_error {
        a.push("--force".into());
    }
    a.push(e.database.clone());
    a
}

/// A pg_dump custom archive starts with "PGDMP"; anything else is treated as SQL.
pub fn is_pg_archive(head: &[u8]) -> bool {
    head.starts_with(b"PGDMP")
}

/// The tool a restore of `head` (the file's first bytes) needs.
pub fn restore_tool(driver: Driver, head: &[u8]) -> &'static str {
    match driver {
        Driver::Postgres if is_pg_archive(head) => "pg_restore",
        Driver::Postgres => "psql",
        _ => "mysql",
    }
}

#[cfg(test)]
mod tests {
    use super::super::endpoint::tests::endpoint;
    use super::*;

    #[test]
    fn pg_dump_arguments() {
        let r = BackupRequest {
            path: "/tmp/x.dump".into(),
            format: DumpFormat::Custom,
            content: DumpContent::SchemaOnly,
            schemas: vec!["Sales".into()],
            tables: vec![TableRef {
                schema: "public".into(),
                name: "we\"ird*".into(),
            }],
            no_owner: true,
            clean: true,
            ..Default::default()
        };
        let a = pg_dump(&endpoint(Driver::Postgres), &r);
        let joined = a.join(" ");
        assert!(joined.starts_with("-h 127.0.0.1 -p 5432 -U admin --dbname=shop --verbose"));
        assert!(a.contains(&"--format=custom".to_string()));
        assert!(a.contains(&"--schema=\"Sales\"".to_string()));
        assert!(a.contains(&"--table=\"public\".\"we\"\"ird*\"".to_string()));
        assert!(joined.ends_with("--no-owner --clean --if-exists --file=/tmp/x.dump"));
        assert!(!joined.contains("s3cret"));
    }

    #[test]
    fn mysqldump_arguments() {
        let e = Endpoint {
            port: 3306,
            ..endpoint(Driver::Mysql)
        };
        let mut r = BackupRequest {
            path: "/tmp/x.sql".into(),
            single_transaction: true,
            routines: true,
            triggers: true,
            tables: vec![TableRef {
                schema: "shop".into(),
                name: "orders".into(),
            }],
            ..Default::default()
        };
        let a = mysqldump(&e, &r, (9, false), false).join(" ");
        assert!(a.contains("--no-tablespaces --set-gtid-purged=OFF"));
        assert!(a.contains("--single-transaction --routines --result-file=/tmp/x.sql shop orders"));
        assert!(!a.contains("s3cret"));
        r.tables.clear();
        r.create_database = true;
        r.triggers = false;
        let a = mysqldump(&e, &r, (8, false), true).join(" ");
        assert!(a.contains("--column-statistics=0"));
        assert!(a.contains("--skip-triggers"));
        assert!(a.ends_with("--databases shop"));
    }

    #[test]
    fn restore_arguments_and_format() {
        let r = RestoreRequest {
            path: "/tmp/x.sql".into(),
            stop_on_error: true,
            ..Default::default()
        };
        let pg = endpoint(Driver::Postgres);
        let a = psql(&pg, &r).join(" ");
        assert!(a.ends_with("--dbname=shop --no-psqlrc --quiet --set ON_ERROR_STOP=1"));
        let a = pg_restore(&pg, &r).join(" ");
        assert!(a.ends_with("--verbose --exit-on-error /tmp/x.sql"));
        let my = endpoint(Driver::Mysql);
        assert!(!mysql(&my, &r, false).contains(&"--force".to_string()));
        let keep_going = RestoreRequest {
            stop_on_error: false,
            ..r
        };
        assert!(mysql(&my, &keep_going, false).contains(&"--force".to_string()));
        assert_eq!(restore_tool(Driver::Postgres, b"PGDMP\x01"), "pg_restore");
        assert_eq!(restore_tool(Driver::Postgres, b"-- SQL"), "psql");
        assert_eq!(restore_tool(Driver::Mysql, b"PGDMP"), "mysql");
        assert_eq!(pg_dbname("shop"), "shop");
        assert_eq!(pg_dbname("a=b's"), r"dbname='a=b\'s'");
    }
}
