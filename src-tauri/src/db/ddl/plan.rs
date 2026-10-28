//! Turning a `SchemaChange` into a plan: the statements to run and how to run them.

use std::collections::HashMap;

use crate::db::meta::ForeignKeyInfo;
use crate::db::split;
use crate::db::sql::{qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Dialect;

use super::alter;
use super::clauses::{bad, need_name, need_word, sql};
use super::create::{build_create_table, TableDef};
use super::objects;
use super::spec::{AlterOp, DatabaseOptions, SchemaChange};

/// How the statements of a plan run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exec {
    /// All or nothing (Postgres and SQLite DDL is transactional).
    Transaction,
    /// One by one: MySQL commits each DDL statement, and Postgres can't create or drop
    /// a database inside a transaction.
    Sequential,
    /// SQLite with a table rebuild: foreign keys off, one transaction, foreign key check.
    SqliteRebuild,
}

#[derive(Debug, Clone)]
pub enum Step {
    Sql(String),
    /// Rebuild of an SQLite table; its statements are worked out against the table as it
    /// is when the step runs (after the renames before it).
    Rebuild(Rebuild),
}

#[derive(Debug, Clone)]
pub struct Rebuild {
    pub schema: String,
    pub table: String,
    /// Column, primary key and foreign key changes, with columns already renamed.
    pub ops: Vec<AlterOp>,
    /// Renames done just before, so the preview can show the new names.
    pub renames: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub exec: Exec,
}

/// What building a plan needs to know about the database, fetched up front.
#[derive(Debug, Clone, Default)]
pub struct Ctx {
    /// For DropTable: "TABLE", "VIEW", "MATERIALIZED VIEW" or "FOREIGN TABLE".
    pub object_kind: Option<String>,
    /// Postgres: name of the primary key constraint. MySQL/SQLite: "PRIMARY" if any.
    pub primary_key: Option<String>,
    /// Postgres: index name → the constraint that owns it (UNIQUE / PRIMARY KEY).
    pub index_constraints: HashMap<String, String>,
    /// Existing foreign keys (SQLite names them `fk_<id>`).
    pub foreign_keys: Vec<ForeignKeyInfo>,
    /// For DropObject of a Postgres routine: its identity arguments, from the catalog.
    pub signature: Option<String>,
}

/// Marks the place of an SQLite DuplicateTable's CREATE TABLE, which is copied from the
/// original table when the statements are worked out.
pub(super) const SQLITE_COPY_MARKER: &str = "-- copy of";

pub fn build(driver: Dialect, change: &SchemaChange, ctx: &Ctx) -> AppResult<Plan> {
    let mut steps = Vec::new();
    let exec = match change {
        SchemaChange::CreateDatabase { name, options } => {
            sql(&mut steps, create_database(driver, name, options)?);
            Exec::Sequential
        }
        SchemaChange::DropDatabase { name, force } => {
            sql(&mut steps, drop_database(driver, name, *force)?);
            Exec::Sequential
        }
        SchemaChange::AlterTable {
            schema,
            name,
            ops,
            rename_to,
        } => {
            need_name("The table", name)?;
            let mut ops = ops.clone();
            ops.sort_by_key(AlterOp::phase);
            let rebuild = alter::build(driver, schema, name, &ops, ctx, &mut steps)?;
            let rename = rename_to.as_deref().map(str::trim);
            if let Some(to) = rename.filter(|t| !t.is_empty() && t != name) {
                if rebuild {
                    return Err(bad(
                        "SQLite can't rename the table together with these column changes. \
                         Apply them first, then rename the table.",
                    ));
                }
                sql(&mut steps, rename_table(driver, schema, name, to)?);
            }
            if rebuild {
                Exec::SqliteRebuild
            } else {
                default_exec(driver)
            }
        }
        other => {
            object_steps(driver, other, ctx, &mut steps)?;
            default_exec(driver)
        }
    };
    check_single_statements(driver, &steps)?;
    Ok(Plan { steps, exec })
}

/// MySQL commits each DDL statement, so its plans run one by one.
fn default_exec(driver: Dialect) -> Exec {
    if driver == Dialect::Mysql {
        Exec::Sequential
    } else {
        Exec::Transaction
    }
}

/// Schema and table changes other than ALTER TABLE.
fn object_steps(
    driver: Dialect,
    change: &SchemaChange,
    ctx: &Ctx,
    steps: &mut Vec<Step>,
) -> AppResult<()> {
    match change {
        SchemaChange::CreateSchema { name } => {
            need_schemas(driver, name)?;
            sql(
                steps,
                format!("CREATE SCHEMA {}", quote_ident(driver, name)),
            );
        }
        SchemaChange::DropSchema { name, cascade } => {
            need_schemas(driver, name)?;
            let cascade = if *cascade { " CASCADE" } else { "" };
            let schema = quote_ident(driver, name);
            sql(steps, format!("DROP SCHEMA {schema}{cascade}"));
        }
        SchemaChange::CreateTable {
            schema,
            name,
            columns,
            indexes,
            foreign_keys,
            comment,
        } => {
            let def = TableDef {
                schema,
                name,
                columns,
                indexes,
                foreign_keys,
                comment,
            };
            build_create_table(driver, &def, steps)?;
        }
        SchemaChange::RenameTable { schema, from, to } => {
            sql(steps, rename_table(driver, schema, from, to)?);
        }
        SchemaChange::DropTable {
            schema,
            name,
            cascade,
        } => sql(steps, drop_table(driver, schema, name, *cascade, ctx)?),
        SchemaChange::TruncateTable {
            schema,
            name,
            cascade,
        } => sql(steps, truncate_table(driver, schema, name, *cascade)?),
        SchemaChange::DuplicateTable {
            schema,
            from,
            to,
            with_data,
        } => duplicate_table(driver, schema, from, to, *with_data, steps)?,
        SchemaChange::DropObject { .. }
        | SchemaChange::RefreshMaterializedView { .. }
        | SchemaChange::ResetSequence { .. } => objects::steps(driver, change, ctx, steps)?,
        SchemaChange::CreateDatabase { .. }
        | SchemaChange::DropDatabase { .. }
        | SchemaChange::AlterTable { .. } => unreachable!("handled by build"),
    }
    Ok(())
}

/// DROP TABLE / VIEW / MATERIALIZED VIEW / FOREIGN TABLE, whichever the object is.
fn drop_table(
    driver: Dialect,
    schema: &str,
    name: &str,
    cascade: bool,
    ctx: &Ctx,
) -> AppResult<String> {
    need_name("The table", name)?;
    let kind = ctx.object_kind.clone().unwrap_or_else(|| "TABLE".into());
    let cascade = if cascade && driver == Dialect::Postgres {
        " CASCADE"
    } else {
        ""
    };
    let target = qualified(driver, schema, name);
    Ok(format!("DROP {kind} {target}{cascade}"))
}

/// Types and defaults are SQL the user typed: each generated statement must still be
/// exactly one statement.
fn check_single_statements(driver: Dialect, steps: &[Step]) -> AppResult<()> {
    for s in steps {
        if let Step::Sql(text) = s {
            if !text.starts_with("--") && split::split(driver, text).len() != 1 {
                return Err(bad(
                    "A type or default value ends the statement early (a stray ';'?).",
                )
                .hint(format!("Generated SQL: {text}")));
            }
        }
    }
    Ok(())
}

fn need_schemas(driver: Dialect, name: &str) -> AppResult<()> {
    need_name("The schema", name)?;
    if driver != Dialect::Postgres {
        return Err(bad("Only PostgreSQL has schemas inside a database."));
    }
    Ok(())
}

fn create_database(driver: Dialect, name: &str, options: &DatabaseOptions) -> AppResult<String> {
    need_name("The database", name)?;
    let mut s = format!("CREATE DATABASE {}", quote_ident(driver, name));
    let set = |v: &str| !v.trim().is_empty();
    match driver {
        Dialect::Postgres => {
            if set(&options.owner) {
                let owner = quote_ident(driver, options.owner.trim());
                s.push_str(&format!(" OWNER {owner}"));
            }
            if set(&options.template) {
                let template = quote_ident(driver, options.template.trim());
                s.push_str(&format!(" TEMPLATE {template}"));
            }
            if set(&options.encoding) {
                need_word("Encoding", options.encoding.trim())?;
                s.push_str(&format!(" ENCODING '{}'", options.encoding.trim()));
            }
        }
        Dialect::Mysql => {
            if set(&options.charset) {
                need_word("Character set", options.charset.trim())?;
                s.push_str(&format!(" CHARACTER SET {}", options.charset.trim()));
            }
            if set(&options.collation) {
                need_word("Collation", options.collation.trim())?;
                s.push_str(&format!(" COLLATE {}", options.collation.trim()));
            }
        }
        Dialect::Sqlite => {
            return Err(bad(
                "An SQLite database is a file: create it from the new connection form.",
            ));
        }
    }
    Ok(s)
}

fn drop_database(driver: Dialect, name: &str, force: bool) -> AppResult<String> {
    need_name("The database", name)?;
    let db = quote_ident(driver, name);
    match driver {
        Dialect::Postgres => {
            let force = if force { " WITH (FORCE)" } else { "" };
            Ok(format!("DROP DATABASE {db}{force}"))
        }
        Dialect::Mysql => Ok(format!("DROP DATABASE {db}")),
        Dialect::Sqlite => Err(bad(
            "An SQLite database is a file: delete the file to remove it.",
        )),
    }
}

fn rename_table(driver: Dialect, schema: &str, from: &str, to: &str) -> AppResult<String> {
    need_name("The table", from)?;
    need_name("The new table", to)?;
    let src = qualified(driver, schema, from);
    Ok(match driver {
        Dialect::Mysql => format!("RENAME TABLE {src} TO {}", qualified(driver, schema, to)),
        _ => format!("ALTER TABLE {src} RENAME TO {}", quote_ident(driver, to)),
    })
}

fn truncate_table(driver: Dialect, schema: &str, name: &str, cascade: bool) -> AppResult<String> {
    need_name("The table", name)?;
    let t = qualified(driver, schema, name);
    Ok(match driver {
        Dialect::Postgres => {
            let cascade = if cascade { " CASCADE" } else { "" };
            format!("TRUNCATE TABLE {t}{cascade}")
        }
        Dialect::Mysql => format!("TRUNCATE TABLE {t}"),
        Dialect::Sqlite => format!("DELETE FROM {t}"),
    })
}

fn duplicate_table(
    driver: Dialect,
    schema: &str,
    from: &str,
    to: &str,
    with_data: bool,
    steps: &mut Vec<Step>,
) -> AppResult<()> {
    need_name("The table", from)?;
    need_name("The new table", to)?;
    let src = qualified(driver, schema, from);
    let dst = qualified(driver, schema, to);
    let (create, insert) = match driver {
        Dialect::Postgres => (
            format!("CREATE TABLE {dst} (LIKE {src} INCLUDING ALL)"),
            format!("INSERT INTO {dst} OVERRIDING SYSTEM VALUE SELECT * FROM {src}"),
        ),
        Dialect::Mysql => (
            format!("CREATE TABLE {dst} LIKE {src}"),
            format!("INSERT INTO {dst} SELECT * FROM {src}"),
        ),
        Dialect::Sqlite => (
            format!("{SQLITE_COPY_MARKER} {src}"),
            format!("INSERT INTO {dst} SELECT * FROM {src}"),
        ),
    };
    sql(steps, create);
    if with_data {
        sql(steps, insert);
    }
    Ok(())
}
