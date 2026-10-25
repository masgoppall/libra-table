//! Elasticsearch (and OpenSearch) indices of documents.

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::{self, OperatorInfo};
use crate::db::split::Statement;
use crate::drivers::{server_fields, DriverInfo, Features, Group, Kind};
use crate::error::AppResult;
use crate::models::Driver;
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Elasticsearch, Group::Nosql, "");
    info.kind = Kind::Document;
    info.sql_dialect = None;
    info.query_language = "elastic";
    info.fields = server_fields("Index pattern");
    info.fields[3].placeholder = "*";
    info.note = "SSL = HTTPS";
    info
}

pub fn features() -> Features {
    Features::none()
}

pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Driver::Elasticsearch.dialect())
}

pub fn classify(_statement: &str) -> Option<StatementKind> {
    None
}

/// None splits the script as SQL.
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

pub async fn open(_a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    Err(super::not_built(Driver::Elasticsearch))
}
