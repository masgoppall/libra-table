//! ALTER TABLE for each driver. The ops arrive sorted by `AlterOp::phase`.

mod mysql;
mod postgres;
mod sqlite;

use crate::error::{AppError, AppResult};
use crate::models::Dialect;

use super::plan::{Ctx, Step};
use super::spec::AlterOp;

/// Appends the steps for `ops`. Returns whether an SQLite table rebuild is needed.
pub(super) fn build(
    driver: Dialect,
    schema: &str,
    table: &str,
    ops: &[AlterOp],
    ctx: &Ctx,
    steps: &mut Vec<Step>,
) -> AppResult<bool> {
    let moves = ops.iter().any(|op| {
        matches!(
            op,
            AlterOp::AddColumn {
                position: Some(_),
                ..
            } | AlterOp::ModifyColumn {
                position: Some(_),
                ..
            }
        )
    });
    if moves && driver != Dialect::Mysql {
        return Err(AppError::validation(
            "Currently, this database doesn't support changing ordinal position.",
        ));
    }
    match driver {
        Dialect::Postgres => postgres::alter(schema, table, ops, ctx, steps).map(|_| false),
        Dialect::Mysql => mysql::alter(schema, table, ops, ctx, steps).map(|_| false),
        Dialect::Sqlite => sqlite::alter(schema, table, ops, steps),
    }
}
