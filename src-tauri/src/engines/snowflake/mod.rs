//! Snowflake over the REST API its official connectors use (password login, JSON
//! result sets). Built from the connectors' protocol and tested against a mock server
//! only: not verified against the real service.

mod catalog;
mod client;
mod editor;
mod engine;
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
    let mut info = DriverInfo::server(Driver::Snowflake, Group::Cloud, "snowflake");
    info.fields = vec![
        Field::text("extra.account", "Account", "xy12345.eu-central-1")
            .required()
            .help("Account identifier, or the full host name (….snowflakecomputing.com)."),
        Field::text("user", "User", "").required(),
        Field::text("database", "Database", ""),
        Field::text("extra.warehouse", "Warehouse", ""),
        Field::text("extra.role", "Role", ""),
        Field::text("extra.endpoint", "Endpoint override", "")
            .help("Advanced, for tests and proxies: a base URL used instead of the account host."),
    ];
    info.secret = Some(Secret::named("Password"));
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

/// TablePlus' Snowflake list: the SQLite operators without LIKE.
pub fn operators() -> Vec<OperatorInfo> {
    filter::operators(Dialect::Sqlite)
        .into_iter()
        .filter(|o| o.id != "like")
        .collect()
}

pub(crate) fn op_ids() -> Vec<&'static str> {
    operators().into_iter().map(|o| o.id).collect()
}

/// SHOW / DESCRIBE / LIST read, USE changes the session; the rest follows SQL rules.
pub fn classify(statement: &str) -> Option<StatementKind> {
    let first = statement
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == ';')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    match first.as_str() {
        "SHOW" | "DESC" | "DESCRIBE" | "LIST" | "LS" => Some(StatementKind::Read),
        "USE" => Some(StatementKind::Session),
        _ => None,
    }
}

/// None splits the script as SQL (Postgres rules handle `$$` bodies).
pub fn split(_script: &str) -> Option<Vec<Statement>> {
    None
}

/// Base URL and the ACCOUNT_NAME of the login for an account identifier or host name.
fn account_host(account: &str) -> (String, String) {
    let bare = account
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let host = if bare.to_ascii_lowercase().contains(".snowflakecomputing.") {
        bare.to_string()
    } else {
        format!("{bare}.snowflakecomputing.com")
    };
    let name = bare.split('.').next().unwrap_or(bare).to_string();
    (format!("https://{host}"), name)
}

fn login(a: &OpenArgs<'_>) -> AppResult<client::Login> {
    let c = a.conn;
    let account = c.extra("account");
    if account.is_empty() {
        return Err(AppError::validation(
            "Enter the Snowflake account identifier.",
        ));
    }
    let user = c.db.user.trim();
    if user.is_empty() {
        return Err(AppError::validation("Enter the Snowflake user name."));
    }
    let password = a
        .secret
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::validation("Enter the password for this Snowflake user."))?;
    let (host, name) = account_host(account);
    let base = match c.extra("endpoint") {
        "" => host,
        e => cloud_common::endpoint(e, "endpoint override")?,
    };
    Ok(client::Login {
        base,
        account: name,
        user: user.to_string(),
        password: password.to_string(),
        database: a.database(),
        warehouse: c.extra("warehouse").to_string(),
        role: c.extra("role").to_string(),
    })
}

pub async fn open(a: &OpenArgs<'_>) -> AppResult<SharedEngine> {
    let login = login(a)?;
    let http = cloud_common::client(a.max_connections)?;
    let main = client::Conn::login(http.clone(), &login).await?;
    let engine = engine::Snowflake::new(http, login, main, a.conn);
    if let Err(e) = engine.startup(&engine.main).await {
        engine.main.logout().await;
        return Err(e);
    }
    Ok(Arc::new(engine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_from_accounts() {
        assert_eq!(
            account_host("xy12345.eu-central-1"),
            (
                "https://xy12345.eu-central-1.snowflakecomputing.com".into(),
                "xy12345".into()
            )
        );
        assert_eq!(
            account_host("https://myorg-acct.snowflakecomputing.com/"),
            (
                "https://myorg-acct.snowflakecomputing.com".into(),
                "myorg-acct".into()
            )
        );
    }

    #[test]
    fn classifies_snowflake_commands() {
        assert_eq!(classify("show tables"), Some(StatementKind::Read));
        assert_eq!(classify("  DESC TABLE t"), Some(StatementKind::Read));
        assert_eq!(classify("use warehouse w"), Some(StatementKind::Session));
        assert_eq!(classify("select 1"), None);
        assert!(operators().iter().all(|o| o.id != "like"));
    }
}
