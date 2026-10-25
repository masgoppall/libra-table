//! Redis keys and values.
//!
//! How the generic engine model maps onto Redis:
//! * Databases are the numbered databases `db0…dbN` (`CONFIG GET databases`, 16 when
//!   that command is disabled). The sidebar lists the non-empty ones plus the current
//!   one; "show system schemas" lists all of them. There are no schemas.
//! * Every database has one "table", `keys`: columns `key` (the primary key), `type`,
//!   `ttl` (seconds, -1 = no expiry), `size` (bytes of a string, elements of a
//!   collection) and `value` (a short preview). Pages follow SCAN cursors; filters are
//!   `SCAN MATCH` patterns and `TYPE`; counts are `DBSIZE`.
//! * Grid edits rename keys, change TTLs, set and insert string keys and delete keys;
//!   the key view (`commands::redis`) edits the inside of the other types.
//! * The editor runs Redis commands, one per line, on a connection of its own.
//! * Redis has no read-only session (`READONLY` only concerns cluster replicas), so
//!   read-only connections are enforced in the app through `classify`.

mod args;
mod commands;
mod conn;
mod edits;
mod engine;
mod filters;
mod key_edits;
mod keys;
mod plan;
mod read;
mod results;
mod run;
mod values;

pub use engine::{DatabaseInfo, RedisEngine, ScanPage, ScanRequest};
pub use key_edits::KeyEdit;
pub use read::{KeyData, KeyDetail, ReadRequest};

use std::sync::Arc;

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::OperatorInfo;
use crate::db::split::Statement;
use crate::drivers::{server_fields, DriverInfo, Features, Group, Kind};
use crate::error::AppResult;
use crate::models::Driver;
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Redis, Group::Nosql, "");
    info.kind = Kind::KeyValue;
    info.sql_dialect = None;
    info.query_language = "redis";
    info.fields = server_fields("Database index");
    info.fields[2] = info.fields[2]
        .clone()
        .help("Redis 6+ ACL user; leave empty for the default user.");
    info.fields[2].label = "User (ACL)";
    info.fields[3].placeholder = "0";
    info
}

pub fn features() -> Features {
    Features {
        ready: true,
        switch_database: true,
        query: true,
        edit_rows: true,
        quick_look: true,
        ..Features::none()
    }
}

pub fn operators() -> Vec<OperatorInfo> {
    filters::operators()
}

pub fn classify(statement: &str) -> Option<StatementKind> {
    Some(commands::classify(statement))
}

/// One command per line (quoted arguments may span lines).
pub fn split(script: &str) -> Option<Vec<Statement>> {
    Some(args::split(script))
}

pub async fn open(a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    Ok(Arc::new(RedisEngine::open(a).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_entry() {
        let i = info();
        assert_eq!(i.kind, Kind::KeyValue);
        assert_eq!(i.query_language, "redis");
        assert_eq!(i.default_port, Some(6379));
        let f = features();
        assert!(f.ready && f.query && f.edit_rows && f.switch_database);
        assert!(!f.has_schemas && !f.import && !f.diagram);
        assert_eq!(classify("GET k"), Some(StatementKind::Read));
        assert_eq!(split("GET a\nGET b").map(|s| s.len()), Some(2));
    }
}
