//! Google BigQuery over its REST API v2 with a service-account key. Built from the
//! API documentation and tested against the goccy emulator and a mock token endpoint
//! only: not verified against the real service.

mod auth;
mod catalog;
mod client;
mod engine;
mod split;
mod sql;
mod values;

use std::sync::Arc;

use crate::db::engine::{OpenArgs, SharedEngine};
use crate::db::filter::{self, OperatorInfo};
use crate::db::split::Statement;
use crate::drivers::{DriverInfo, Features, Field, Group, Secret, Structure};
use crate::engines::cloud_common;
use crate::error::{AppError, AppResult};
use crate::models::{Dialect, Driver};
use crate::safe_mode::Kind as StatementKind;

pub fn info() -> DriverInfo {
    let mut info = DriverInfo::server(Driver::Bigquery, Group::Cloud, "bigquery");
    info.fields = vec![
        Field::text("extra.project", "Project ID", "my-project")
            .required()
            .help("Queries run (and are billed) in this project."),
        Field::text("database", "Dataset", "").help("Optional default dataset."),
        Field::text("extra.location", "Location", "US"),
        Field::text("extra.endpoint", "API endpoint", "")
            .help("Leave empty for Google; set it for an emulator (the key may then be empty)."),
    ];
    info.secret = Some(Secret {
        label: "Service account key (JSON)",
        multiline: true,
        optional: true,
    });
    info.ssh = false;
    info.tls = false;
    info.note = "Not verified against the real service yet";
    info
}

pub fn features() -> Features {
    Features {
        ready: true,
        has_schemas: true,
        switch_database: true,
        query: true,
        edit_rows: true,
        structure: Structure::View,
        quick_look: true,
        ..Features::none()
    }
}

/// TablePlus' BigQuery list: the SQLite operators without LIKE, plus case-sensitive
/// text variants (the plain ones ignore case).
pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Dialect::Mysql)
        .into_iter()
        .filter(|o| o.id != "like")
        .collect()
}

pub(crate) fn op_ids() -> Vec<&'static str> {
    operators().into_iter().map(|o| o.id).collect()
}

/// SELECT / WITH only read in GoogleSQL; DECLARE / SET and transaction control change
/// the session. Everything else follows the SQL rules (fail safe: a write).
pub fn classify(statement: &str) -> Option<StatementKind> {
    let mut words = statement
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_uppercase);
    let first = words.next().unwrap_or_default();
    let second = words.next().unwrap_or_default();
    let opens_with_paren = statement.trim_start().starts_with('(');
    match first.as_str() {
        _ if opens_with_paren => Some(StatementKind::Read),
        "SELECT" | "WITH" => Some(StatementKind::Read),
        "DECLARE" | "SET" | "COMMIT" | "ROLLBACK" => Some(StatementKind::Session),
        "BEGIN" if second.is_empty() || second == "TRANSACTION" => Some(StatementKind::Session),
        _ => None,
    }
}

pub fn split(script: &str) -> Option<Vec<Statement>> {
    Some(split::split(script))
}

pub async fn open(a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    let c = a.conn;
    let endpoint = match c.extra("endpoint") {
        "" => None,
        e => Some(cloud_common::endpoint(e, "API endpoint")?),
    };
    let secret = a.secret.map(str::trim).filter(|s| !s.is_empty());
    let key = match (secret, &endpoint) {
        (Some(json), _) => Some(auth::ServiceKey::parse(json)?),
        (None, Some(_)) => None,
        (None, None) => {
            return Err(AppError::validation(
                "Paste the service account key (JSON).",
            ))
        }
    };
    let project = match c.extra("project") {
        "" => key
            .as_ref()
            .map(|k| k.project_id.clone())
            .unwrap_or_default(),
        p => p.to_string(),
    };
    if project.is_empty() {
        return Err(AppError::validation("Enter the Google Cloud project ID."));
    }
    let http = cloud_common::client(a.max_connections)?;
    let root = endpoint.unwrap_or_else(|| client::GOOGLE_API.to_string());
    let api = client::Api::new(
        http,
        &root,
        auth::Auth::new(key),
        &project,
        c.extra("location"),
    );
    let data_project = a
        .database
        .filter(|d| !d.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| project.clone());
    // One cheap call proves the key and the project access.
    let probe = format!("/projects/{}/datasets", client::seg(&data_project));
    api.get(&probe, &[("maxResults", "1".into())]).await?;
    Ok(Arc::new(engine::BigQuery {
        api,
        data_project,
        default_dataset: c.db.database.trim().to_string(),
        startup: c.options.startup_query.trim().to_string(),
        read_only: c.options.read_only,
        session: tokio::sync::Mutex::new(None),
        running: std::sync::Mutex::new(None),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_googlesql() {
        assert_eq!(classify("select 1"), Some(StatementKind::Read));
        assert_eq!(
            classify("(SELECT 1) UNION ALL (SELECT 2)"),
            Some(StatementKind::Read)
        );
        assert_eq!(
            classify("WITH a AS (SELECT 1) SELECT * FROM a"),
            Some(StatementKind::Read)
        );
        assert_eq!(classify("BEGIN TRANSACTION"), Some(StatementKind::Session));
        assert_eq!(classify("BEGIN"), Some(StatementKind::Session));
        assert_eq!(classify("BEGIN SELECT 1; END"), None);
        assert_eq!(classify("DELETE FROM t WHERE true"), None);
        let ids = op_ids();
        assert!(ids.contains(&"contains_cs") && !ids.contains(&"like"));
    }
}
