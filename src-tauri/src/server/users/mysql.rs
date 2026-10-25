//! MySQL / MariaDB accounts (`user@host`): listing with global privileges, CREATE/DROP
//! USER, passwords and GRANT/REVOKE on `*.*`, `db.*` and `db.table`.

use serde_json::Value;
use sqlx::MySqlPool;

use crate::db::sql::{literal, mysql_literal, quote_ident};
use crate::db::value;
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

use super::privileges::MY_GLOBAL_COLUMNS;
use super::{check_grant, check_password, require, GrantTarget, Grantee, MysqlAccount, Stmt};
use super::{UserChange, MASK};

/// How string literals must be escaped on this server.
#[derive(Debug, Clone, Copy)]
pub struct Quoting {
    /// False under `NO_BACKSLASH_ESCAPES`: a backslash is then an ordinary character.
    pub backslash_escapes: bool,
}

impl Default for Quoting {
    fn default() -> Self {
        Self {
            backslash_escapes: true,
        }
    }
}

impl Quoting {
    fn literal(self, v: &str) -> String {
        if self.backslash_escapes {
            mysql_literal(v)
        } else {
            literal(v)
        }
    }
}

pub async fn quoting(p: &MySqlPool) -> AppResult<Quoting> {
    let mode: String = sqlx::query_scalar("SELECT CAST(@@SESSION.sql_mode AS CHAR)")
        .fetch_one(p)
        .await?;
    Ok(Quoting {
        backslash_escapes: !mode.to_ascii_uppercase().contains("NO_BACKSLASH_ESCAPES"),
    })
}

pub(super) fn list_sql() -> String {
    let privs: Vec<String> = MY_GLOBAL_COLUMNS
        .iter()
        .map(|(col, name)| format!("IF({col} = 'Y', '{name}', NULL)"))
        .collect();
    format!(
        "SELECT CAST(User AS CHAR), CAST(Host AS CHAR), CONCAT_WS(',', {}) \
         FROM mysql.user ORDER BY 1, 2",
        privs.join(", ")
    )
}

pub async fn list(p: &MySqlPool) -> AppResult<Vec<MysqlAccount>> {
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as(&list_sql()).fetch_all(p).await?;
    Ok(rows
        .into_iter()
        .map(|(user, host, privs)| MysqlAccount {
            user,
            host,
            privileges: privs
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        })
        .collect())
}

pub async fn grants(p: &MySqlPool, g: &Grantee) -> AppResult<Vec<String>> {
    let sql = format!("SHOW GRANTS FOR {}", account(&g.name, host_of(g)));
    let rows = sqlx::query(&sql).fetch_all(p).await?;
    Ok(rows
        .iter()
        .filter_map(
            |r| match value::mysql::row(r, value::PREVIEW).into_iter().next() {
                Some(Value::String(s)) => Some(s),
                _ => None,
            },
        )
        .collect())
}

/// `` `user`@`host` ``: backticks mean the same whatever the SQL mode.
fn account(user: &str, host: &str) -> String {
    let host = if host.trim().is_empty() { "%" } else { host };
    format!(
        "{}@{}",
        quote_ident(Dialect::Mysql, user),
        quote_ident(Dialect::Mysql, host)
    )
}

fn host_of(g: &Grantee) -> &str {
    g.host.as_deref().unwrap_or("%")
}

/// `head` followed by ` IDENTIFIED BY '…'` (masked in the shown text).
fn identified(head: String, password: Option<&str>, quoting: Quoting) -> AppResult<Stmt> {
    let Some(pw) = password else {
        return Ok(Stmt::plain(head));
    };
    check_password(pw)?;
    Ok(Stmt {
        run: format!("{head} IDENTIFIED BY {}", quoting.literal(pw)),
        shown: format!("{head} IDENTIFIED BY {MASK}"),
    })
}

pub fn plan(change: &UserChange, quoting: Quoting) -> AppResult<Vec<Stmt>> {
    let stmt = match change {
        UserChange::CreateUser {
            user,
            host,
            password,
        } => {
            require(user, "user name")?;
            let head = format!("CREATE USER {}", account(user, host));
            identified(head, password.as_deref(), quoting)?
        }
        UserChange::DropUser { user, host } => {
            require(user, "user name")?;
            Stmt::plain(format!("DROP USER {}", account(user, host)))
        }
        UserChange::SetPassword { grantee, password } => {
            require(&grantee.name, "user name")?;
            let head = format!("ALTER USER {}", account(&grantee.name, host_of(grantee)));
            identified(head, Some(password), quoting)?
        }
        UserChange::Grant {
            grantee,
            privileges,
            target,
            with_grant_option,
        } => {
            let on = grant_head(privileges, target, grantee)?;
            let opt = if *with_grant_option {
                " WITH GRANT OPTION"
            } else {
                ""
            };
            let to = account(&grantee.name, host_of(grantee));
            Stmt::plain(format!("GRANT {on} TO {to}{opt}"))
        }
        UserChange::Revoke {
            grantee,
            privileges,
            target,
        } => {
            let on = grant_head(privileges, target, grantee)?;
            let from = account(&grantee.name, host_of(grantee));
            Stmt::plain(format!("REVOKE {on} FROM {from}"))
        }
        _ => return Err(AppError::validation("That change is for Postgres roles.")),
    };
    Ok(vec![stmt])
}

fn grant_head(privileges: &[String], target: &GrantTarget, g: &Grantee) -> AppResult<String> {
    require(&g.name, "user name")?;
    check_grant(Dialect::Mysql, privileges, target)?;
    let q = |n: &str| quote_ident(Dialect::Mysql, n);
    let on = match target {
        GrantTarget::Global => "*.*".to_string(),
        GrantTarget::Database { name } => format!("{}.*", q(name)),
        GrantTarget::Table { schema, name } => format!("{}.{}", q(schema), q(name)),
        _ => {
            return Err(AppError::validation(
                "MySQL has no schemas inside a database.",
            ))
        }
    };
    Ok(format!("{} ON {on}", privileges.join(", ")))
}
