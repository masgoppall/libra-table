//! DuckDB database files (embedded).

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::{self, OperatorInfo};
use crate::db::split::Statement;
use crate::drivers::{DriverInfo, Features, Field, FieldKind, Group};
use crate::error::AppResult;
use crate::models::Driver;
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Duckdb, Group::Analytics, "duckdb");
    info.fields = vec![Field::text("filePath", "Database file", "")
        .kind(FieldKind::File, vec!["duckdb", "db", "ddb"])
        .required()];
    info.secret = None;
    info.ssh = false;
    info.tls = false;
    info
}

pub fn features() -> Features {
    Features::none()
}

pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Driver::Duckdb.dialect())
}

pub fn classify(_statement: &str) -> Option<StatementKind> {
    None
}

/// None splits the script as SQL.
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

pub async fn open(_a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    Err(super::not_built(Driver::Duckdb))
}
