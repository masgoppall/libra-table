//! Drivers that don't run on sqlx, one module each. Every module offers the same
//! functions: `info` (picker and form), `features`, `operators` (filter bar), `classify`
//! (safe mode for languages that aren't SQL) and `open`, which returns its `Engine`.

pub mod bigquery;
pub mod cassandra;
pub mod clickhouse;
// HTTP client and grid SQL shared by Snowflake, BigQuery and DynamoDB.
pub mod cloud_common;
pub mod d1;
pub mod duckdb;
pub mod dynamodb;
pub mod elasticsearch;
pub mod libsql;
pub mod mongodb;
pub mod mssql;
pub mod oracle;
pub mod redis;
pub mod snowflake;

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::OperatorInfo;
use crate::db::split::{self, Statement};
use crate::drivers::{DriverInfo, Features};
use crate::error::{AppError, AppResult};
use crate::models::Driver;
use crate::safe_mode::Kind;

/// Calls `$f` of the engine module for `$driver`, or `$sqlx` for built-in drivers.
macro_rules! dispatch {
    ($driver:expr, $f:ident ( $($arg:expr),* ), $sqlx:expr) => {
        match $driver {
            Driver::Mssql => mssql::$f($($arg),*),
            Driver::Oracle => oracle::$f($($arg),*),
            Driver::Clickhouse => clickhouse::$f($($arg),*),
            Driver::Duckdb => duckdb::$f($($arg),*),
            Driver::Cassandra => cassandra::$f($($arg),*),
            Driver::Snowflake => snowflake::$f($($arg),*),
            Driver::Bigquery => bigquery::$f($($arg),*),
            Driver::Libsql => libsql::$f($($arg),*),
            Driver::CloudflareD1 => d1::$f($($arg),*),
            Driver::Redis => redis::$f($($arg),*),
            Driver::Mongodb => mongodb::$f($($arg),*),
            Driver::Dynamodb => dynamodb::$f($($arg),*),
            Driver::Elasticsearch => elasticsearch::$f($($arg),*),
            Driver::Postgres
            | Driver::Mysql
            | Driver::Sqlite
            | Driver::Mariadb
            | Driver::Cockroach
            | Driver::Redshift
            | Driver::Greenplum
            | Driver::Vertica => $sqlx,
        }
    };
}

pub fn info(driver: Driver) -> DriverInfo {
    dispatch!(driver, info(), crate::drivers::info(driver))
}

pub fn features(driver: Driver) -> Features {
    dispatch!(driver, features(), crate::drivers::features(driver))
}

pub fn operators(driver: Driver) -> Vec<OperatorInfo> {
    dispatch!(driver, operators(), crate::drivers::operators(driver))
}

/// Safe-mode kind of one statement in the driver's own language; None = SQL rules.
pub fn classify(driver: Driver, statement: &str) -> Option<Kind> {
    dispatch!(driver, classify(statement), None)
}

/// Splits an editor script into statements in the driver's language (SQL by default).
pub fn split(driver: Driver, script: &str) -> Vec<Statement> {
    dispatch!(driver, split(script), None).unwrap_or_else(|| split::split(driver.dialect(), script))
}

pub async fn open(a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    match a.conn.driver {
        Driver::Mssql => mssql::open(a).await,
        Driver::Oracle => oracle::open(a).await,
        Driver::Clickhouse => clickhouse::open(a).await,
        Driver::Duckdb => duckdb::open(a).await,
        Driver::Cassandra => cassandra::open(a).await,
        Driver::Snowflake => snowflake::open(a).await,
        Driver::Bigquery => bigquery::open(a).await,
        Driver::Libsql => libsql::open(a).await,
        Driver::CloudflareD1 => d1::open(a).await,
        Driver::Redis => redis::open(a).await,
        Driver::Mongodb => mongodb::open(a).await,
        Driver::Dynamodb => dynamodb::open(a).await,
        Driver::Elasticsearch => elasticsearch::open(a).await,
        other => Err(AppError::internal(format!(
            "{} is not an engine driver.",
            other.label()
        ))),
    }
}

/// What a module answers until its driver is built.
pub fn not_built(driver: Driver) -> AppError {
    AppError::unsupported(format!(
        "The {} driver is not available in this version of LibraTable yet.",
        driver.label()
    ))
}
