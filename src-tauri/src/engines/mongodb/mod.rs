//! MongoDB collections of documents.

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::{self, OperatorInfo};
use crate::db::split::Statement;
use crate::drivers::{DriverInfo, Features, Field, FieldKind, Group, Kind};
use crate::error::AppResult;
use crate::models::Driver;
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Mongodb, Group::Nosql, "");
    info.kind = Kind::Document;
    info.sql_dialect = None;
    info.query_language = "mongo";
    info.fields
        .push(Field::text("extra.authSource", "Auth database", "admin"));
    info.fields
        .push(Field::text("extra.replicaSet", "Replica set", ""));
    info.fields.push(
        Field::text("extra.srv", "DNS seed list (mongodb+srv)", "")
            .kind(FieldKind::Bool, Vec::new()),
    );
    info
}

pub fn features() -> Features {
    Features::none()
}

pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Driver::Mongodb.dialect())
}

pub fn classify(_statement: &str) -> Option<StatementKind> {
    None
}

/// None splits the script as SQL.
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

pub async fn open(_a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    Err(super::not_built(Driver::Mongodb))
}
