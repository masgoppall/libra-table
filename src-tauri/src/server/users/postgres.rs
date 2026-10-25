//! Postgres roles: listing, CREATE/ALTER/DROP ROLE, memberships and GRANT/REVOKE.

use sqlx::{PgPool, Row};

use crate::db::sql::{literal, quote_ident};
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

use super::{check_grant, check_password, require, GrantTarget, PgRole, RoleSpec, Stmt};
use super::{UserChange, MASK};

const ROLES: &str = "SELECT r.rolname::text AS name, r.rolcanlogin, r.rolsuper, \
    r.rolcreatedb, r.rolcreaterole, r.rolreplication, r.rolconnlimit, \
    r.rolvaliduntil::text AS valid_until, \
    ARRAY(SELECT b.rolname::text FROM pg_auth_members m \
          JOIN pg_roles b ON b.oid = m.roleid WHERE m.member = r.oid ORDER BY 1) AS member_of \
    FROM pg_roles r";

pub async fn list(p: &PgPool) -> AppResult<Vec<PgRole>> {
    let sql = format!("{ROLES} WHERE r.rolname !~ '^pg_' ORDER BY 1");
    let rows = sqlx::query(&sql).fetch_all(p).await?;
    rows.iter().map(role_row).collect()
}

pub async fn role(p: &PgPool, name: &str) -> AppResult<PgRole> {
    let sql = format!("{ROLES} WHERE r.rolname = $1");
    let row = sqlx::query(&sql).bind(name).fetch_optional(p).await?;
    match row {
        Some(r) => role_row(&r),
        None => Err(AppError::not_found(format!(
            "The role \"{name}\" no longer exists."
        ))),
    }
}

fn role_row(r: &sqlx::postgres::PgRow) -> AppResult<PgRole> {
    Ok(PgRole {
        name: r.try_get("name")?,
        login: r.try_get("rolcanlogin")?,
        superuser: r.try_get("rolsuper")?,
        create_db: r.try_get("rolcreatedb")?,
        create_role: r.try_get("rolcreaterole")?,
        replication: r.try_get("rolreplication")?,
        conn_limit: r.try_get("rolconnlimit")?,
        valid_until: r.try_get("valid_until")?,
        member_of: r.try_get("member_of")?,
    })
}

/// Privileges granted explicitly to the role on databases, schemas and tables of the
/// current database, one line per object ("SELECT, INSERT ON TABLE public.t").
const GRANTS: &str = "WITH r AS (SELECT oid FROM pg_roles WHERE rolname = $1), g AS ( \
      SELECT 'DATABASE ' || quote_ident(d.datname) AS obj, a.privilege_type, a.is_grantable \
      FROM pg_database d, aclexplode(d.datacl) a WHERE a.grantee = (SELECT oid FROM r) \
    UNION ALL \
      SELECT 'SCHEMA ' || quote_ident(n.nspname), a.privilege_type, a.is_grantable \
      FROM pg_namespace n, aclexplode(n.nspacl) a WHERE a.grantee = (SELECT oid FROM r) \
    UNION ALL \
      SELECT 'TABLE ' || quote_ident(n.nspname) || '.' || quote_ident(c.relname), \
             a.privilege_type, a.is_grantable \
      FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace, aclexplode(c.relacl) a \
      WHERE c.relkind IN ('r', 'v', 'm', 'p', 'f') AND a.grantee = (SELECT oid FROM r)) \
    SELECT string_agg(privilege_type || CASE WHEN is_grantable THEN ' (grantable)' \
           ELSE '' END, ', ' ORDER BY privilege_type) || ' ON ' || obj \
    FROM g GROUP BY obj ORDER BY obj";

pub async fn grants(p: &PgPool, role: &str) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar(GRANTS).bind(role).fetch_all(p).await?)
}

pub fn plan(change: &UserChange, current: Option<&PgRole>) -> AppResult<Vec<Stmt>> {
    match change {
        UserChange::CreateRole { role, password } => create(role, password.as_deref()),
        UserChange::AlterRole { role, password, .. } => {
            let current = current.ok_or_else(|| AppError::internal("Role not loaded."))?;
            alter(current, role, password.as_deref())
        }
        UserChange::DropRole { name } => {
            require(name, "role name")?;
            Ok(vec![Stmt::plain(format!("DROP ROLE {}", q(name)))])
        }
        UserChange::SetPassword { grantee, password } => {
            require(&grantee.name, "role name")?;
            let head = format!("ALTER ROLE {} WITH", q(&grantee.name));
            Ok(vec![with_password(head, Some(password))?])
        }
        UserChange::Grant {
            grantee,
            privileges,
            target,
            with_grant_option,
        } => {
            let on = grant_head(privileges, target, &grantee.name)?;
            let opt = if *with_grant_option {
                " WITH GRANT OPTION"
            } else {
                ""
            };
            let sql = format!("GRANT {on} TO {}{opt}", q(&grantee.name));
            Ok(vec![Stmt::plain(sql)])
        }
        UserChange::Revoke {
            grantee,
            privileges,
            target,
        } => {
            let on = grant_head(privileges, target, &grantee.name)?;
            Ok(vec![Stmt::plain(format!(
                "REVOKE {on} FROM {}",
                q(&grantee.name)
            ))])
        }
        _ => Err(AppError::validation("That change is for MySQL accounts.")),
    }
}

fn q(name: &str) -> String {
    quote_ident(Dialect::Postgres, name)
}

/// A string literal that means the same whatever `standard_conforming_strings` is:
/// with a backslash in it, an escape string with the backslashes doubled.
pub fn pg_literal(v: &str) -> String {
    if v.contains('\\') {
        format!("E'{}'", v.replace('\\', "\\\\").replace('\'', "''"))
    } else {
        literal(v)
    }
}

fn flag(on: bool, name: &str) -> String {
    if on {
        name.to_string()
    } else {
        format!("NO{name}")
    }
}

fn attributes(r: &RoleSpec) -> Vec<String> {
    vec![
        flag(r.login, "LOGIN"),
        flag(r.superuser, "SUPERUSER"),
        flag(r.create_db, "CREATEDB"),
        flag(r.create_role, "CREATEROLE"),
        flag(r.replication, "REPLICATION"),
    ]
}

fn conn_limit(limit: Option<i32>) -> String {
    format!("CONNECTION LIMIT {}", limit.unwrap_or(-1).max(-1))
}

fn valid_until(v: &Option<String>) -> String {
    let when = v.as_deref().map(str::trim).filter(|s| !s.is_empty());
    format!("VALID UNTIL {}", pg_literal(when.unwrap_or("infinity")))
}

/// `head` followed by ` PASSWORD '…'` (masked in the shown text) when there is one.
fn with_password(head: String, password: Option<&str>) -> AppResult<Stmt> {
    let Some(pw) = password else {
        return Ok(Stmt::plain(head));
    };
    check_password(pw)?;
    Ok(Stmt {
        run: format!("{head} PASSWORD {}", pg_literal(pw)),
        shown: format!("{head} PASSWORD {MASK}"),
    })
}

fn create(r: &RoleSpec, password: Option<&str>) -> AppResult<Vec<Stmt>> {
    require(&r.name, "role name")?;
    let mut head = format!(
        "CREATE ROLE {} WITH {}",
        q(&r.name),
        attributes(r).join(" ")
    );
    if r.conn_limit.is_some() {
        head = format!("{head} {}", conn_limit(r.conn_limit));
    }
    if r.valid_until
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty())
    {
        head = format!("{head} {}", valid_until(&r.valid_until));
    }
    let mut out = vec![with_password(head, password)?];
    for parent in &r.member_of {
        out.push(Stmt::plain(format!(
            "GRANT {} TO {}",
            q(parent),
            q(&r.name)
        )));
    }
    Ok(out)
}

/// Only what changed: a rename first, then attributes, password and memberships.
fn alter(cur: &PgRole, r: &RoleSpec, password: Option<&str>) -> AppResult<Vec<Stmt>> {
    require(&r.name, "role name")?;
    let mut out = Vec::new();
    if r.name != cur.name {
        let sql = format!("ALTER ROLE {} RENAME TO {}", q(&cur.name), q(&r.name));
        out.push(Stmt::plain(sql));
    }
    let mut changes = Vec::new();
    let was = RoleSpec {
        name: cur.name.clone(),
        login: cur.login,
        superuser: cur.superuser,
        create_db: cur.create_db,
        create_role: cur.create_role,
        replication: cur.replication,
        conn_limit: (cur.conn_limit >= 0).then_some(cur.conn_limit),
        valid_until: cur.valid_until.clone().filter(|v| v != "infinity"),
        member_of: cur.member_of.clone(),
    };
    for (new, old) in attributes(r).into_iter().zip(attributes(&was)) {
        if new != old {
            changes.push(new);
        }
    }
    if r.conn_limit.map(|l| l.max(-1)).filter(|l| *l >= 0) != was.conn_limit {
        changes.push(conn_limit(r.conn_limit));
    }
    let until = |v: &Option<String>| v.as_deref().map(str::trim).unwrap_or("").to_string();
    if until(&r.valid_until) != until(&was.valid_until) {
        changes.push(valid_until(&r.valid_until));
    }
    if !changes.is_empty() || password.is_some() {
        let head = format!("ALTER ROLE {} WITH {}", q(&r.name), changes.join(" "));
        out.push(with_password(head.trim_end().to_string(), password)?);
    }
    for parent in r.member_of.iter().filter(|p| !was.member_of.contains(p)) {
        out.push(Stmt::plain(format!(
            "GRANT {} TO {}",
            q(parent),
            q(&r.name)
        )));
    }
    for parent in was.member_of.iter().filter(|p| !r.member_of.contains(p)) {
        out.push(Stmt::plain(format!(
            "REVOKE {} FROM {}",
            q(parent),
            q(&r.name)
        )));
    }
    Ok(out)
}

/// `SELECT, INSERT ON TABLE "s"."t"`: the part GRANT and REVOKE share.
fn grant_head(privileges: &[String], target: &GrantTarget, grantee: &str) -> AppResult<String> {
    require(grantee, "role name")?;
    check_grant(Dialect::Postgres, privileges, target)?;
    let on = match target {
        GrantTarget::Database { name } => format!("DATABASE {}", q(name)),
        GrantTarget::Schema { name } => format!("SCHEMA {}", q(name)),
        GrantTarget::AllTables { schema } => format!("ALL TABLES IN SCHEMA {}", q(schema)),
        GrantTarget::Table { schema, name } => format!("TABLE {}.{}", q(schema), q(name)),
        GrantTarget::Global => return Err(AppError::validation("Postgres has no global grants.")),
    };
    Ok(format!("{} ON {on}", privileges.join(", ")))
}
