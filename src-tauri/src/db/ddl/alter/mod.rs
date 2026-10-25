//! ALTER TABLE for each driver. The ops arrive sorted by `AlterOp::phase`.

mod mysql;
mod postgres;
mod sqlite;

use crate::error::AppResult;
use crate::models::Driver;

use super::plan::{Ctx, Step};
use super::spec::AlterOp;

/// Appends the steps for `ops`. Returns whether an SQLite table rebuild is needed.
pub(super) fn build(
    driver: Driver,
    schema: &str,
    table: &str,
    ops: &[AlterOp],
    ctx: &Ctx,
    steps: &mut Vec<Step>,
) -> AppResult<bool> {
    match driver {
        Driver::Postgres => postgres::alter(schema, table, ops, ctx, steps).map(|_| false),
        Driver::Mysql => mysql::alter(schema, table, ops, ctx, steps).map(|_| false),
        Driver::Sqlite => sqlite::alter(schema, table, ops, steps),
    }
}
