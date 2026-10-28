//! ALTER TABLE on SQLite: renames, simple added/dropped columns and indexes run in
//! place; anything else becomes one rebuild step.

use crate::db::sql::{qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Dialect;

use super::super::clauses::{
    bad, column_def, comment_of, create_index, need_name, same_default, same_type, sql,
};
use super::super::plan::{Rebuild, Step};
use super::super::spec::{AlterOp, ColumnSpec};

const D: Dialect = Dialect::Sqlite;

fn needs_rebuild(op: &AlterOp) -> bool {
    match op {
        AlterOp::ModifyColumn {
            original: o,
            column: c,
            ..
        } => {
            !same_type(&o.data_type, &c.data_type)
                || o.nullable != c.nullable
                || !same_default(o, c)
                || o.auto_increment != c.auto_increment
        }
        AlterOp::AddColumn { column, .. } => column.unique || column.primary_key,
        AlterOp::SetPrimaryKey { .. }
        | AlterOp::AddForeignKey { .. }
        | AlterOp::DropForeignKey { .. } => true,
        _ => false,
    }
}

/// Returns whether a rebuild is needed.
pub(super) fn alter(
    schema: &str,
    table: &str,
    ops: &[AlterOp],
    steps: &mut Vec<Step>,
) -> AppResult<bool> {
    let mut b = Alter {
        schema,
        table: qualified(D, schema, table),
        rebuild: ops.iter().any(needs_rebuild),
        rebuild_spec: Rebuild {
            schema: schema.into(),
            table: table.into(),
            ops: Vec::new(),
            renames: Vec::new(),
        },
        steps,
    };
    for op in ops {
        b.op(op)?;
    }
    let Alter {
        rebuild,
        rebuild_spec,
        ..
    } = b;
    if rebuild {
        steps.push(Step::Rebuild(rebuild_spec));
    }
    for op in ops {
        if let AlterOp::AddIndex { index } = op {
            sql(steps, create_index(D, schema, table, index)?);
        }
    }
    Ok(rebuild)
}

/// Statements that run in place go to `steps`; the rest wait in `rebuild_spec`.
struct Alter<'a> {
    schema: &'a str,
    /// The table, qualified and quoted.
    table: String,
    rebuild: bool,
    rebuild_spec: Rebuild,
    steps: &'a mut Vec<Step>,
}

impl Alter<'_> {
    fn op(&mut self, op: &AlterOp) -> AppResult<()> {
        let t = &self.table;
        match op {
            AlterOp::SetComment { comment } => {
                if comment_of(comment).is_some() {
                    return Err(bad("SQLite doesn't store table comments."));
                }
            }
            AlterOp::DropIndex { name } => sql(
                self.steps,
                format!("DROP INDEX {}", qualified(D, self.schema, name)),
            ),
            // Indexes are created at the end, after any rebuild.
            AlterOp::AddIndex { .. } => {}
            AlterOp::ModifyColumn {
                original, column, ..
            } => self.modify_column(op, original, column)?,
            AlterOp::AddColumn { column, .. } if self.rebuild => {
                column_def(D, column, false, true)?;
                self.rebuild_spec.ops.push(op.clone());
            }
            AlterOp::AddColumn { column, .. } => {
                let def = column_def(D, column, false, true)?;
                sql(self.steps, format!("ALTER TABLE {t} ADD COLUMN {def}"));
            }
            AlterOp::DropColumn { .. } if self.rebuild => self.rebuild_spec.ops.push(op.clone()),
            AlterOp::DropColumn { name } => {
                let col = quote_ident(D, name);
                sql(self.steps, format!("ALTER TABLE {t} DROP COLUMN {col}"));
            }
            AlterOp::SetPrimaryKey { .. }
            | AlterOp::AddForeignKey { .. }
            | AlterOp::DropForeignKey { .. } => self.rebuild_spec.ops.push(op.clone()),
        }
        Ok(())
    }

    /// A rename runs in place; other changes to the column go into the rebuild.
    fn modify_column(&mut self, op: &AlterOp, o: &ColumnSpec, c: &ColumnSpec) -> AppResult<()> {
        if let Some(rename) = rename_column(&self.table, o, c, self.steps)? {
            self.rebuild_spec.renames.push(rename);
        }
        if self.rebuild && needs_rebuild(op) {
            // The rebuild sees the table after the rename.
            let mut original = o.clone();
            original.name = c.name.clone();
            self.rebuild_spec.ops.push(AlterOp::ModifyColumn {
                original,
                column: c.clone(),
                position: None,
            });
        }
        Ok(())
    }
}

/// Renames in place when the name changed; returns (from, to).
fn rename_column(
    t: &str,
    o: &ColumnSpec,
    c: &ColumnSpec,
    steps: &mut Vec<Step>,
) -> AppResult<Option<(String, String)>> {
    need_name("Every column", &c.name)?;
    let mut rename = None;
    if o.name != c.name {
        sql(
            steps,
            format!(
                "ALTER TABLE {t} RENAME COLUMN {} TO {}",
                quote_ident(D, &o.name),
                quote_ident(D, &c.name)
            ),
        );
        rename = Some((o.name.clone(), c.name.clone()));
    }
    if comment_of(&o.comment) != comment_of(&c.comment) && comment_of(&c.comment).is_some() {
        return Err(bad("SQLite doesn't store column comments."));
    }
    Ok(rename)
}
