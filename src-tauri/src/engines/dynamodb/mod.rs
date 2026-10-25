//! Amazon DynamoDB tables of items over the JSON API, signed with SigV4. Built from
//! the API documentation and tested against DynamoDB Local only: not verified against
//! the real service.

mod catalog;
mod client;
mod edits;
mod engine;
mod filter;
mod grid;
mod sigv4;
mod values;

use std::sync::Arc;

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::OperatorInfo;
use crate::db::split::Statement;
use crate::drivers::{DriverInfo, Features, Field, Group, Kind, Secret, Structure};
use crate::engines::cloud_common;
use crate::error::{AppError, AppResult};
use crate::models::{Dialect, Driver};
use crate::safe_mode::Kind as StatementKind;

/// TablePlus' comparisons plus contains / begins_with.
const OPERATORS: &[&str] = &[
    "eq",
    "neq",
    "lt",
    "gt",
    "lte",
    "gte",
    "contains",
    "not_contains",
    "has_prefix",
];

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Dynamodb, Group::Nosql, "");
    info.kind = Kind::Document;
    info.sql_dialect = None;
    info.query_language = "partiql";
    info.fields = vec![
        Field::text("extra.region", "Region", "us-east-1").required(),
        Field::text("extra.accessKeyId", "Access key ID", "").required(),
        Field::text("extra.endpoint", "Endpoint", "")
            .help("Optional, e.g. http://127.0.0.1:8000 for DynamoDB Local."),
    ];
    info.secret = Some(Secret::named("Secret access key"));
    info.ssh = false;
    info.tls = false;
    info.note = "Not verified against the real service yet";
    info
}

pub fn features() -> Features {
    Features {
        ready: true,
        query: true,
        edit_rows: true,
        structure: Structure::View,
        quick_look: true,
        ..Features::none()
    }
}

pub fn operators() -> Vec<OperatorInfo> {
    crate::db::filter::operators(Dialect::Sqlite)
        .into_iter()
        .filter(|o| OPERATORS.contains(&o.id))
        .collect()
}

pub(crate) fn op_ids() -> Vec<&'static str> {
    OPERATORS.to_vec()
}

/// PartiQL: SELECT reads; INSERT / UPDATE / DELETE (and anything else) write.
pub fn classify(statement: &str) -> Option<StatementKind> {
    let first = statement
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    Some(if first == "SELECT" {
        StatementKind::Read
    } else {
        StatementKind::Write
    })
}

/// None splits the script on `;` (PartiQL quotes like SQL).
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

pub async fn open(a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    let c = a.conn;
    let region = c.extra("region");
    if region.is_empty() {
        return Err(AppError::validation(
            "Enter the AWS region, e.g. us-east-1.",
        ));
    }
    let access_key = c.extra("accessKeyId");
    if access_key.is_empty() {
        return Err(AppError::validation("Enter the access key ID."));
    }
    let secret = a
        .secret
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::validation("Enter the secret access key."))?;
    let (url, local) = match c.extra("endpoint") {
        "" => (format!("https://dynamodb.{region}.amazonaws.com"), false),
        e => (cloud_common::endpoint(e, "endpoint")?, true),
    };
    let creds = sigv4::Credentials {
        access_key: access_key.to_string(),
        secret_key: secret.to_string(),
    };
    let http = cloud_common::client(a.max_connections)?;
    let client = client::Client::new(http, &url, region, creds)?;
    // Proves the keys, the signature and the region.
    client
        .call("ListTables", &serde_json::json!({ "Limit": 1 }))
        .await?;
    Ok(Arc::new(engine::Dynamo {
        client,
        read_only: c.options.read_only,
        local,
        pages: grid::PageCache::default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partiql_reads_and_writes() {
        assert_eq!(classify(" select * from t"), Some(StatementKind::Read));
        assert_eq!(
            classify("UPDATE t SET a = 1 WHERE pk = 'x'"),
            Some(StatementKind::Write)
        );
        assert_eq!(classify("EXISTS(SELECT 1)"), Some(StatementKind::Write));
        let ids: Vec<&str> = operators().iter().map(|o| o.id).collect();
        assert_eq!(ids, OPERATORS);
    }
}
