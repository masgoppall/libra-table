//! Users and privileges: Postgres roles, MySQL `user@host` accounts, and GRANT/REVOKE.
//!
//! Every change becomes a list of statements built here. Each has the text that runs
//! and the text that is shown (and written to history), which differ only where a
//! password goes: the shown one has `'********'`. Passwords can't be bind parameters in
//! these utility statements, so they are sent as escaped literals and never logged.

mod mysql;
mod postgres;
mod privileges;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use sqlx::Executor;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::Driver;

pub use privileges::{privilege_options, PrivilegeOption};

/// What a password looks like in previews and history.
pub const MASK: &str = "'********'";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PgRole {
    pub name: String,
    pub login: bool,
    pub superuser: bool,
    pub create_db: bool,
    pub create_role: bool,
    pub replication: bool,
    /// -1 means no limit.
    pub conn_limit: i32,
    pub valid_until: Option<String>,
    pub member_of: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MysqlAccount {
    pub user: String,
    pub host: String,
    /// Global (`*.*`) privileges, e.g. ["SELECT", "PROCESS", "GRANT OPTION"].
    pub privileges: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserList {
    /// Postgres only.
    pub roles: Vec<PgRole>,
    /// MySQL only.
    pub accounts: Vec<MysqlAccount>,
    /// Which privileges each kind of grant target accepts.
    pub privilege_options: Vec<PrivilegeOption>,
}

/// Role attributes as the form sends them (Postgres).
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct RoleSpec {
    pub name: String,
    pub login: bool,
    pub superuser: bool,
    pub create_db: bool,
    pub create_role: bool,
    pub replication: bool,
    /// None means no limit.
    pub conn_limit: Option<i32>,
    /// A timestamp; None means forever.
    pub valid_until: Option<String>,
    pub member_of: Vec<String>,
}

/// Who receives a grant: a role (Postgres) or `user@host` (MySQL).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Grantee {
    pub name: String,
    #[serde(default)]
    pub host: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Global,
    Database,
    Schema,
    AllTables,
    Table,
}

/// What a grant is on. MySQL: global `*.*`, database `db.*`, table `db.table` (the
/// database goes in `schema`). Postgres: database, schema, all tables in a schema, table.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum GrantTarget {
    Global,
    Database { name: String },
    Schema { name: String },
    AllTables { schema: String },
    Table { schema: String, name: String },
}

impl GrantTarget {
    pub fn kind(&self) -> TargetKind {
        match self {
            GrantTarget::Global => TargetKind::Global,
            GrantTarget::Database { .. } => TargetKind::Database,
            GrantTarget::Schema { .. } => TargetKind::Schema,
            GrantTarget::AllTables { .. } => TargetKind::AllTables,
            GrantTarget::Table { .. } => TargetKind::Table,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum UserChange {
    CreateRole {
        role: RoleSpec,
        #[serde(default)]
        password: Option<String>,
    },
    /// `name` is the role as it is now; `role` is how it should be.
    AlterRole {
        name: String,
        role: RoleSpec,
        #[serde(default)]
        password: Option<String>,
    },
    DropRole {
        name: String,
    },
    CreateUser {
        user: String,
        host: String,
        #[serde(default)]
        password: Option<String>,
    },
    DropUser {
        user: String,
        host: String,
    },
    SetPassword {
        grantee: Grantee,
        password: String,
    },
    Grant {
        grantee: Grantee,
        privileges: Vec<String>,
        target: GrantTarget,
        #[serde(default)]
        with_grant_option: bool,
    },
    Revoke {
        grantee: Grantee,
        privileges: Vec<String>,
        target: GrantTarget,
    },
}

/// One statement: what runs and what is shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub run: String,
    pub shown: String,
}

impl Stmt {
    /// A statement without secrets.
    pub fn plain(sql: String) -> Self {
        Self {
            run: sql.clone(),
            shown: sql,
        }
    }
}

pub async fn list(pool: &DbPool) -> AppResult<UserList> {
    let driver = pool.driver();
    let (roles, accounts) = match pool {
        DbPool::Postgres(p) => (postgres::list(p).await?, Vec::new()),
        DbPool::Mysql(p) => (Vec::new(), mysql::list(p).await?),
        DbPool::Sqlite(_) => return Err(no_users()),
    };
    Ok(UserList {
        roles,
        accounts,
        privilege_options: privilege_options(driver),
    })
}

/// What `grantee` has been granted, one line per object (`SHOW GRANTS` on MySQL).
pub async fn grants(pool: &DbPool, grantee: &Grantee) -> AppResult<Vec<String>> {
    match pool {
        DbPool::Postgres(p) => postgres::grants(p, &grantee.name).await,
        DbPool::Mysql(p) => mysql::grants(p, grantee).await,
        DbPool::Sqlite(_) => Err(no_users()),
    }
}

/// The statements of `change`. Postgres compares an altered role with the server.
pub async fn plan(pool: &DbPool, change: &UserChange) -> AppResult<Vec<Stmt>> {
    let quoting = match pool {
        DbPool::Mysql(p) => mysql::quoting(p).await?,
        _ => mysql::Quoting::default(),
    };
    match (pool, change) {
        (DbPool::Postgres(p), UserChange::AlterRole { name, .. }) => {
            let current = postgres::role(p, name).await?;
            postgres::plan(change, Some(&current))
        }
        (DbPool::Postgres(_), _) => postgres::plan(change, None),
        (DbPool::Mysql(_), _) => mysql::plan(change, quoting),
        (DbPool::Sqlite(_), _) => Err(no_users()),
    }
}

/// Runs the statements: in one transaction on Postgres (role DDL is transactional
/// there), one by one on MySQL (account statements commit implicitly).
pub async fn apply(pool: &DbPool, stmts: &[Stmt]) -> AppResult<()> {
    match pool {
        DbPool::Postgres(p) => {
            let mut tx = p.begin().await?;
            for s in stmts {
                (&mut *tx).execute(sqlx::raw_sql(&s.run)).await?;
            }
            tx.commit().await?;
        }
        DbPool::Mysql(p) => {
            let mut conn = p.acquire().await?;
            for s in stmts {
                (&mut *conn).execute(sqlx::raw_sql(&s.run)).await?;
            }
        }
        DbPool::Sqlite(_) => return Err(no_users()),
    }
    Ok(())
}

fn no_users() -> AppError {
    AppError::validation("SQLite has no users or privileges.")
}

/// Names are quoted, but an empty one is always a mistake.
fn require(value: &str, what: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::validation(format!("Enter the {what}.")));
    }
    Ok(())
}

/// Postgres can't store NUL in text; MySQL would cut the literal there.
fn check_password(p: &str) -> AppResult<()> {
    if p.contains('\0') {
        return Err(AppError::validation(
            "A password can't contain a NUL character.",
        ));
    }
    Ok(())
}

/// Grantee, target and privileges must fit the driver.
fn check_grant(driver: Driver, privileges: &[String], target: &GrantTarget) -> AppResult<()> {
    let allowed = privileges::allowed(driver, target.kind()).ok_or_else(|| {
        AppError::validation(format!(
            "{} can't grant on that kind of object.",
            driver.label()
        ))
    })?;
    if privileges.is_empty() {
        return Err(AppError::validation("Choose at least one privilege."));
    }
    if let Some(bad) = privileges.iter().find(|p| !allowed.contains(&p.as_str())) {
        return Err(AppError::validation(format!(
            "\"{bad}\" is not a privilege for this object."
        )));
    }
    if privileges.len() > 1 && privileges.iter().any(|p| p == "ALL PRIVILEGES") {
        return Err(AppError::validation(
            "ALL PRIVILEGES already includes the others: choose it alone.",
        ));
    }
    Ok(())
}
