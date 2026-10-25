//! Apache Cassandra (and ScyllaDB) over the CQL protocol.

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::{self, OperatorInfo};
use crate::db::split::Statement;
use crate::drivers::{server_fields, DriverInfo, Features, Field, Group};
use crate::error::AppResult;
use crate::models::Driver;
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Cassandra, Group::Nosql, "cql");
    info.fields = server_fields("Keyspace");
    info.fields.push(Field::text(
        "extra.datacenter",
        "Local datacenter",
        "datacenter1",
    ));
    info.note = "CQL";
    info
}

pub fn features() -> Features {
    Features::none()
}

pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Driver::Cassandra.dialect())
}

pub fn classify(_statement: &str) -> Option<StatementKind> {
    None
}

/// None splits the script as SQL.
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

pub async fn open(_a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    Err(super::not_built(Driver::Cassandra))
}
