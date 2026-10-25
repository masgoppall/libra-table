//! LibSQL and Turso over HTTP.

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::{self, OperatorInfo};
use crate::db::split::Statement;
use crate::drivers::{DriverInfo, Features, Field, Group, Secret};
use crate::error::AppResult;
use crate::models::Driver;
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Libsql, Group::Compatible, "sqlite");
    info.fields = vec![Field::text("extra.url", "URL", "libsql://my-db-org.turso.io").required()];
    info.secret = Some(Secret {
        label: "Auth token",
        multiline: false,
        optional: true,
    });
    info.ssh = false;
    info.tls = false;
    info.note = "SQLite over HTTP";
    info
}

pub fn features() -> Features {
    Features::none()
}

pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Driver::Libsql.dialect())
}

pub fn classify(_statement: &str) -> Option<StatementKind> {
    None
}

/// None splits the script as SQL.
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

pub async fn open(_a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    Err(super::not_built(Driver::Libsql))
}
