//! Working out the exact statements of a plan (preview) and running them (apply).

use sqlx::{Executor, SqliteConnection};

use crate::db::sql::{qualified, quote_ident};
use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

use super::context::{sqlite_objects, sqlite_table_sql};
use super::plan::{Ctx, Exec, Plan, Rebuild, Step, SQLITE_COPY_MARKER};
use super::rebuild::{sqlite_copy_sql, sqlite_rebuild_sql};
use super::spec::SchemaChange;

/// The statements of `plan` as they will run, for the confirmation dialog.
pub async fn preview(
    pool: &DbPool,
    plan: &Plan,
    change: &SchemaChange,
    ctx: &Ctx,
) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    if plan.exec == Exec::SqliteRebuild {
        out.push("PRAGMA foreign_keys = OFF".to_string());
        out.push("BEGIN".to_string());
    }
    for step in &plan.steps {
        match step {
            Step::Sql(s) if s.starts_with(SQLITE_COPY_MARKER) => {
                if let Some(copy) = sqlite_copy_statement(pool, change).await? {
                    out.push(copy);
                }
            }
            Step::Sql(s) => out.push(s.clone()),
            Step::Rebuild(r) => {
                let DbPool::Sqlite(p) = pool else { continue };
                out.extend(preview_rebuild(p, plan, r, ctx).await?);
            }
        }
    }
    if plan.exec == Exec::SqliteRebuild {
        if let Some((schema, table)) = rebuild_target(plan) {
            out.push(foreign_key_check(schema, table));
        }
        out.push("COMMIT".to_string());
        out.push("PRAGMA foreign_keys = ON".to_string());
    }
    Ok(out)
}

/// The CREATE TABLE of an SQLite DuplicateTable, copied from the original.
async fn sqlite_copy_statement(pool: &DbPool, change: &SchemaChange) -> AppResult<Option<String>> {
    let (
        DbPool::Sqlite(p),
        SchemaChange::DuplicateTable {
            schema, from, to, ..
        },
    ) = (pool, change)
    else {
        return Ok(None);
    };
    let table_sql = sqlite_table_sql(p, schema, from).await?;
    Ok(Some(sqlite_copy_sql(&table_sql, schema, to)?))
}

async fn preview_rebuild(
    p: &sqlx::SqlitePool,
    plan: &Plan,
    r: &Rebuild,
    ctx: &Ctx,
) -> AppResult<Vec<String>> {
    let mut conn = p.acquire().await?;
    let table_sql = sqlite_table_sql(&mut *conn, &r.schema, &r.table).await?;
    let mut objects = sqlite_objects(&mut conn, &r.schema, &r.table).await?;
    // Indexes dropped earlier in the plan are not recreated.
    objects.retain(|o| {
        let drop = format!(
            "DROP INDEX {}",
            qualified(Dialect::Sqlite, &r.schema, &o.name)
        );
        !plan
            .steps
            .iter()
            .any(|s| matches!(s, Step::Sql(x) if *x == drop))
    });
    sqlite_rebuild_sql(r, &table_sql, &objects, &ctx.foreign_keys)
}

fn foreign_key_check(schema: &str, table: &str) -> String {
    format!(
        "PRAGMA {}.foreign_key_check({})",
        quote_ident(Dialect::Sqlite, schema),
        quote_ident(Dialect::Sqlite, table)
    )
}

fn rebuild_target(plan: &Plan) -> Option<(&str, &str)> {
    plan.steps.iter().find_map(|s| match s {
        Step::Rebuild(r) => Some((r.schema.as_str(), r.table.as_str())),
        _ => None,
    })
}

/// Runs the plan. Returns how many statements ran.
pub async fn apply(
    pool: &DbPool,
    plan: &Plan,
    change: &SchemaChange,
    ctx: &Ctx,
) -> AppResult<usize> {
    if let (DbPool::Sqlite(p), Exec::SqliteRebuild) = (pool, plan.exec) {
        return apply_sqlite_rebuild(p, plan, ctx).await;
    }
    let statements: Vec<String> = preview(pool, plan, change, ctx)
        .await?
        .into_iter()
        .filter(|s| !s.starts_with("--"))
        .collect();
    let sequential = plan.exec == Exec::Sequential;
    macro_rules! run_all {
        ($exec:expr) => {
            for (i, s) in statements.iter().enumerate() {
                if let Err(e) = $exec.execute(sqlx::raw_sql(s)).await {
                    return Err(partial_run_error(e.into(), i, statements.len(), sequential));
                }
            }
        };
    }
    match (pool, plan.exec) {
        (DbPool::Postgres(p), Exec::Transaction) => {
            let mut tx = p.begin().await?;
            run_all!(&mut *tx);
            tx.commit().await?;
        }
        (DbPool::Postgres(p), _) => {
            let mut c = p.acquire().await?;
            run_all!(&mut *c);
        }
        (DbPool::Mysql(p), _) => {
            let mut c = p.acquire().await?;
            run_all!(&mut *c);
        }
        (DbPool::Sqlite(p), _) => {
            let mut tx = p.begin().await?;
            run_all!(&mut *tx);
            tx.commit().await?;
        }
        (DbPool::Engine(_), _) => return Err(crate::db::engine::sqlx_only()),
    }
    Ok(statements.len())
}

/// MySQL can't undo the statements that already ran: say how far it got.
fn partial_run_error(e: AppError, ran: usize, total: usize, sequential: bool) -> AppError {
    if ran > 0 && sequential {
        e.hint(format!(
            "The first {ran} of {total} statements ran; MySQL can't undo schema changes."
        ))
    } else {
        e
    }
}

async fn raw(conn: &mut SqliteConnection, sql: &str) -> Result<(), sqlx::Error> {
    conn.execute(sqlx::raw_sql(sql)).await.map(|_| ())
}

/// The rebuild runs with foreign keys off (restored afterwards) in one transaction,
/// and fails if any row would break a foreign key.
async fn apply_sqlite_rebuild(pool: &sqlx::SqlitePool, plan: &Plan, ctx: &Ctx) -> AppResult<usize> {
    let mut conn = pool.acquire().await?;
    let fk_on: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&mut *conn)
        .await?;
    raw(&mut conn, "PRAGMA foreign_keys = OFF").await?;
    let result = run_rebuild_transaction(&mut conn, plan, ctx).await;
    if fk_on != 0 {
        let _ = raw(&mut conn, "PRAGMA foreign_keys = ON").await;
    }
    result
}

async fn run_rebuild_transaction(
    conn: &mut SqliteConnection,
    plan: &Plan,
    ctx: &Ctx,
) -> AppResult<usize> {
    raw(conn, "BEGIN").await?;
    match run_rebuild_steps(conn, plan, ctx).await {
        Ok(ran) => {
            raw(conn, "COMMIT").await?;
            Ok(ran)
        }
        Err(e) => {
            let _ = raw(conn, "ROLLBACK").await;
            let _ = raw(conn, "PRAGMA legacy_alter_table = OFF").await;
            Err(e)
        }
    }
}

async fn run_rebuild_steps(
    conn: &mut SqliteConnection,
    plan: &Plan,
    ctx: &Ctx,
) -> AppResult<usize> {
    let mut ran = 0;
    for step in &plan.steps {
        match step {
            Step::Sql(s) => {
                raw(conn, s).await?;
                ran += 1;
            }
            Step::Rebuild(r) => ran += run_rebuild(conn, r, ctx).await?,
        }
    }
    Ok(ran)
}

async fn run_rebuild(conn: &mut SqliteConnection, r: &Rebuild, ctx: &Ctx) -> AppResult<usize> {
    // Worked out now, after the renames above have run.
    let current = Rebuild {
        renames: Vec::new(),
        ..r.clone()
    };
    let table_sql = sqlite_table_sql(&mut *conn, &r.schema, &r.table).await?;
    let objects = sqlite_objects(conn, &r.schema, &r.table).await?;
    let mut ran = 0;
    for s in sqlite_rebuild_sql(&current, &table_sql, &objects, &ctx.foreign_keys)? {
        if !s.starts_with("--") {
            raw(conn, &s).await?;
            ran += 1;
        }
    }
    let violations: Vec<sqlx::sqlite::SqliteRow> =
        sqlx::query(&foreign_key_check(&r.schema, &r.table))
            .fetch_all(&mut *conn)
            .await?;
    if !violations.is_empty() {
        return Err(AppError::query(format!(
            "{} row(s) would break foreign keys after this change, so nothing was saved.",
            violations.len()
        )));
    }
    Ok(ran)
}
