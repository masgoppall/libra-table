//! Schema changes: databases, schemas, tables, columns, indexes, foreign keys, and
//! dropping/refreshing/resetting other schema objects (functions, sequences, triggers…).
//! The UI describes the change; Rust writes the DDL, so the preview shows exactly what
//! runs. Names are always quoted. Types and default expressions are SQL the user typed,
//! and each generated statement must still be a single statement.
//!
//! SQLite can only rename, add and drop columns in place. Other column changes rebuild
//! the table (see `rebuild`).
//!
//! Flow: `context` reads what the change needs from the database, `build` turns the
//! change into a `Plan`, `preview` lists its exact statements and `apply` runs them.

mod alter;
mod clauses;
mod context;
mod create;
mod objects;
mod plan;
mod rebuild;
mod run;
mod spec;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_structure;

pub use context::context;
pub use plan::{build, Ctx, Exec, Plan, Rebuild, Step};
pub use rebuild::{sqlite_copy_sql, sqlite_rebuild_sql, SqliteObject};
pub use run::{apply, preview};
pub use spec::{AlterOp, ColumnSpec, DatabaseOptions, ForeignKeySpec, IndexSpec, SchemaChange};
