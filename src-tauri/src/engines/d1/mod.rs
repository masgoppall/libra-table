//! Cloudflare D1 over the Cloudflare API.

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::{self, OperatorInfo};
use crate::db::split::Statement;
use crate::drivers::{DriverInfo, Features, Field, Group, Secret};
use crate::error::AppResult;
use crate::models::Driver;
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::CloudflareD1, Group::Cloud, "sqlite");
    info.fields = vec![
        Field::text("extra.accountId", "Account ID", "").required(),
        Field::text("extra.databaseId", "Database ID", "").required(),
        Field::text("database", "Database name", "").help("Shown in the sidebar."),
    ];
    info.secret = Some(Secret::named("API token"));
    info.ssh = false;
    info.tls = false;
    info.note = "SQLite";
    info
}

pub fn features() -> Features {
    Features::none()
}

pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Driver::CloudflareD1.dialect())
}

pub fn classify(_statement: &str) -> Option<StatementKind> {
    None
}

/// None splits the script as SQL.
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

pub async fn open(_a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    Err(super::not_built(Driver::CloudflareD1))
}
