//! What building and running a plan needs from the database: the kind of object,
//! existing keys, and for SQLite the table's own SQL with its indexes and triggers.

use crate::db::sql::quote_ident;
use crate::db::{meta, DbPool};
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

use super::plan::Ctx;
use super::rebuild::SqliteObject;
use super::spec::SchemaChange;

const PG_RELKIND: &str = "SELECT c.relkind::text FROM pg_class c \
    JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1 AND c.relname = $2";

const PG_KEY_CONSTRAINTS: &str = "SELECT con.conname::text, con.contype::text, \
    i.relname::text FROM pg_constraint con \
    JOIN pg_class t ON t.oid = con.conrelid JOIN pg_namespace n ON n.oid = t.relnamespace \
    LEFT JOIN pg_class i ON i.oid = con.conindid \
    WHERE n.nspname = $1 AND t.relname = $2 AND con.contype IN ('p','u')";

const MYSQL_TABLE_TYPE: &str = "SELECT CAST(TABLE_TYPE AS CHAR) FROM information_schema.TABLES \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?";

const MYSQL_HAS_PRIMARY_KEY: &str = "SELECT CAST(COUNT(*) AS SIGNED) \
    FROM information_schema.TABLE_CONSTRAINTS \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_TYPE = 'PRIMARY KEY'";

/// Loads what `build` needs for `change`.
pub async fn context(pool: &DbPool, change: &SchemaChange) -> AppResult<Ctx> {
    let (schema, table) = match change {
        SchemaChange::DropTable { schema, name, .. } => (schema, name),
        SchemaChange::AlterTable { schema, name, .. } => (schema, name),
        SchemaChange::DropObject { schema, object, .. } => {
            return super::objects::context(pool, schema, object).await
        }
        _ => return Ok(Ctx::default()),
    };
    match pool {
        DbPool::Postgres(p) => pg_context(p, schema, table).await,
        DbPool::Mysql(p) => mysql_context(p, schema, table).await,
        DbPool::Sqlite(p) => {
            let altering = matches!(change, SchemaChange::AlterTable { .. });
            sqlite_context(pool, p, schema, table, altering).await
        }
        DbPool::Engine(_) => Err(crate::db::engine::sqlx_only()),
    }
}

async fn pg_context(p: &sqlx::PgPool, schema: &str, table: &str) -> AppResult<Ctx> {
    let mut ctx = Ctx::default();
    let kind: Option<String> = sqlx::query_scalar(PG_RELKIND)
        .bind(schema)
        .bind(table)
        .fetch_optional(p)
        .await?;
    let kind = match kind.as_deref() {
        Some("v") => "VIEW",
        Some("m") => "MATERIALIZED VIEW",
        Some("f") => "FOREIGN TABLE",
        _ => "TABLE",
    };
    ctx.object_kind = Some(kind.into());
    let cons: Vec<(String, String, Option<String>)> = sqlx::query_as(PG_KEY_CONSTRAINTS)
        .bind(schema)
        .bind(table)
        .fetch_all(p)
        .await?;
    for (name, kind, index) in cons {
        if kind == "p" {
            ctx.primary_key = Some(name.clone());
        }
        if let Some(i) = index {
            ctx.index_constraints.insert(i, name);
        }
    }
    Ok(ctx)
}

async fn mysql_context(p: &sqlx::MySqlPool, schema: &str, table: &str) -> AppResult<Ctx> {
    let mut ctx = Ctx::default();
    let t: Option<String> = sqlx::query_scalar(MYSQL_TABLE_TYPE)
        .bind(schema)
        .bind(table)
        .fetch_optional(p)
        .await?;
    let is_view = t.as_deref().is_some_and(|t| t.contains("VIEW"));
    ctx.object_kind = Some(if is_view { "VIEW" } else { "TABLE" }.into());
    let pk: Option<i64> = sqlx::query_scalar(MYSQL_HAS_PRIMARY_KEY)
        .bind(schema)
        .bind(table)
        .fetch_optional(p)
        .await?;
    if pk.unwrap_or(0) > 0 {
        ctx.primary_key = Some("PRIMARY".into());
    }
    Ok(ctx)
}

async fn sqlite_context(
    pool: &DbPool,
    p: &sqlx::SqlitePool,
    schema: &str,
    table: &str,
    altering: bool,
) -> AppResult<Ctx> {
    let mut ctx = Ctx::default();
    let sql = format!(
        "SELECT type FROM {}.sqlite_master WHERE name = ?1 AND type IN ('table','view')",
        quote_ident(Dialect::Sqlite, schema)
    );
    let t: Option<String> = sqlx::query_scalar(&sql)
        .bind(table)
        .fetch_optional(p)
        .await?;
    let is_view = t.as_deref() == Some("view");
    ctx.object_kind = Some(if is_view { "VIEW" } else { "TABLE" }.into());
    if altering && t.as_deref() == Some("table") {
        ctx.foreign_keys = meta::table_structure(pool, schema, table)
            .await?
            .foreign_keys;
    }
    Ok(ctx)
}

/// The `CREATE TABLE` of an SQLite table, as stored in `sqlite_master`.
pub(super) async fn sqlite_table_sql<'c, E>(exec: E, schema: &str, table: &str) -> AppResult<String>
where
    E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
{
    let sql: Option<String> = sqlx::query_scalar(&format!(
        "SELECT sql FROM {}.sqlite_master WHERE type = 'table' AND name = ?1",
        quote_ident(Dialect::Sqlite, schema)
    ))
    .bind(table)
    .fetch_optional(exec)
    .await?
    .flatten();
    sql.ok_or_else(|| AppError::not_found(format!("Table \"{schema}.{table}\" was not found.")))
}

/// The indexes and triggers of an SQLite table, recreated after a rebuild.
pub(super) async fn sqlite_objects(
    conn: &mut sqlx::SqliteConnection,
    schema: &str,
    table: &str,
) -> AppResult<Vec<SqliteObject>> {
    let sql = format!(
        "SELECT type, name, sql FROM {}.sqlite_master \
         WHERE tbl_name = ?1 AND type IN ('index','trigger') AND sql IS NOT NULL \
         ORDER BY type, name",
        quote_ident(Dialect::Sqlite, schema)
    );
    let rows: Vec<(String, String, String)> = sqlx::query_as(&sql)
        .bind(table)
        .fetch_all(&mut *conn)
        .await?;
    let mut out = Vec::new();
    for (kind, name, sql) in rows {
        let columns = if kind == "index" {
            index_columns(conn, schema, &name).await?
        } else {
            Vec::new()
        };
        out.push(SqliteObject { name, sql, columns });
    }
    Ok(out)
}

async fn index_columns(
    conn: &mut sqlx::SqliteConnection,
    schema: &str,
    index: &str,
) -> AppResult<Vec<String>> {
    let names: Vec<Option<String>> =
        sqlx::query_scalar("SELECT name FROM pragma_index_info(?1, ?2)")
            .bind(index)
            .bind(schema)
            .fetch_all(&mut *conn)
            .await?;
    Ok(names.into_iter().flatten().collect())
}
