//! ALTER TABLE on MySQL: each statement commits on its own, so a changed column is
//! rewritten in one CHANGE COLUMN.

use crate::db::sql::{literal_for, qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Dialect;

use super::super::clauses::{
    column_def, comment_of, create_index, fk_clause, quoted_list, same_default, same_type, sql,
};
use super::super::plan::{Ctx, Step};
use super::super::spec::{AlterOp, ColumnPosition, ColumnSpec};

const D: Dialect = Dialect::Mysql;

pub(super) fn alter(
    schema: &str,
    table: &str,
    ops: &[AlterOp],
    ctx: &Ctx,
    steps: &mut Vec<Step>,
) -> AppResult<()> {
    let t = qualified(D, schema, table);
    let inline_pk_col = inline_primary_key(ops, ctx);
    for op in ops {
        match op {
            AlterOp::AddColumn { column, position } => {
                let inline = inline_pk_col.as_deref() == Some(column.name.as_str());
                let def = column_def(D, column, inline, true)?;
                let at = placement(position);
                sql(steps, format!("ALTER TABLE {t} ADD COLUMN {def}{at}"));
            }
            AlterOp::DropColumn { name } => sql(
                steps,
                format!("ALTER TABLE {t} DROP COLUMN {}", quote_ident(D, name)),
            ),
            AlterOp::ModifyColumn {
                original,
                column,
                position,
            } => {
                if column_changed(original, column) || position.is_some() {
                    let from = quote_ident(D, &original.name);
                    let def = column_def(D, column, false, false)?;
                    let at = placement(position);
                    sql(
                        steps,
                        format!("ALTER TABLE {t} CHANGE COLUMN {from} {def}{at}"),
                    );
                }
            }
            AlterOp::SetPrimaryKey { columns } => {
                if inline_pk_col.is_none() {
                    set_primary_key(&t, columns, ctx, steps);
                }
            }
            AlterOp::AddIndex { index } => sql(steps, create_index(D, schema, table, index)?),
            AlterOp::DropIndex { name } => {
                if name == "PRIMARY" {
                    sql(steps, format!("ALTER TABLE {t} DROP PRIMARY KEY"));
                } else {
                    sql(steps, format!("DROP INDEX {} ON {t}", quote_ident(D, name)));
                }
            }
            AlterOp::AddForeignKey { foreign_key } => sql(
                steps,
                format!("ALTER TABLE {t} ADD {}", fk_clause(D, schema, foreign_key)?),
            ),
            AlterOp::DropForeignKey { name } => sql(
                steps,
                format!("ALTER TABLE {t} DROP FOREIGN KEY {}", quote_ident(D, name)),
            ),
            AlterOp::SetComment { comment } => {
                let v = literal_for(D, comment_of(comment).unwrap_or(""));
                sql(steps, format!("ALTER TABLE {t} COMMENT = {v}"));
            }
        }
    }
    Ok(())
}

/// An AUTO_INCREMENT column must be a key the moment it is added, so a new primary
/// key made only of such a column goes on the column itself.
fn inline_primary_key(ops: &[AlterOp], ctx: &Ctx) -> Option<String> {
    let new_pk = ops.iter().find_map(|o| match o {
        AlterOp::SetPrimaryKey { columns } => Some(columns.clone()),
        _ => None,
    })?;
    if new_pk.len() != 1 || ctx.primary_key.is_some() {
        return None;
    }
    ops.iter().find_map(|o| match o {
        AlterOp::AddColumn { column, .. } if column.auto_increment && column.name == new_pk[0] => {
            Some(column.name.clone())
        }
        _ => None,
    })
}

fn column_changed(o: &ColumnSpec, c: &ColumnSpec) -> bool {
    let text = |v: &Option<String>| v.as_deref().map(str::trim).unwrap_or("").to_lowercase();
    o.name != c.name
        || !same_type(&o.data_type, &c.data_type)
        || o.nullable != c.nullable
        || !same_default(o, c)
        || o.auto_increment != c.auto_increment
        || comment_of(&o.comment) != comment_of(&c.comment)
        || text(&o.charset) != text(&c.charset)
        || text(&o.collation) != text(&c.collation)
        || text(&o.extra) != text(&c.extra)
}

/// ` FIRST` / ` AFTER col` after a column definition.
fn placement(position: &Option<ColumnPosition>) -> String {
    match position {
        None => String::new(),
        Some(ColumnPosition::First) => " FIRST".into(),
        Some(ColumnPosition::After(col)) => format!(" AFTER {}", quote_ident(D, col)),
    }
}

fn set_primary_key(t: &str, columns: &[String], ctx: &Ctx, steps: &mut Vec<Step>) {
    let mut parts = Vec::new();
    if ctx.primary_key.is_some() {
        parts.push("DROP PRIMARY KEY".to_string());
    }
    if !columns.is_empty() {
        parts.push(format!("ADD PRIMARY KEY ({})", quoted_list(D, columns)));
    }
    if !parts.is_empty() {
        sql(steps, format!("ALTER TABLE {t} {}", parts.join(", ")));
    }
}
