//! SQLite table rebuild, the way the SQLite docs describe it: new table, copy rows,
//! drop, rename. It starts from the original `CREATE TABLE` so CHECK, COLLATE,
//! REFERENCES and other constraints survive.

use sqlparser::ast::{
    ColumnDef, ColumnOption, CreateTable, Ident, ObjectName, Statement as Ast, TableConstraint,
};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

use crate::db::meta::ForeignKeyInfo;
use crate::db::sql::{qualified, quote_ident};
use crate::error::{AppError, AppResult};
use crate::models::Driver;

use super::clauses::{bad, column_def, fk_clause, quoted_list, SQLITE_AUTOINCREMENT_RULE};
use super::plan::Rebuild;
use super::spec::{AlterOp, ColumnSpec, ForeignKeySpec};

const D: Driver = Driver::Sqlite;

/// Name of the new table while it is being filled.
const TMP_PREFIX: &str = "_libratable_new_";

/// An index or trigger of the table, recreated after the rebuild.
#[derive(Debug, Clone)]
pub struct SqliteObject {
    pub name: String,
    pub sql: String,
    /// Columns an index covers (empty for triggers).
    pub columns: Vec<String>,
}

enum Col {
    /// From the original CREATE TABLE, with its primary key options removed.
    Original(ColumnDef),
    /// Written from what the user entered, keeping the original's extra options
    /// (CHECK, COLLATE, REFERENCES, UNIQUE, GENERATED).
    Spec(ColumnSpec, Option<ColumnDef>),
}

impl Col {
    /// The parsed definition this column keeps its extra options from, if any.
    fn def_mut(&mut self) -> Option<&mut ColumnDef> {
        match self {
            Col::Original(def) => Some(def),
            Col::Spec(_, def) => def.as_mut(),
        }
    }

    fn is_integer(&self) -> bool {
        match self {
            Col::Original(def) => def.data_type.to_string().eq_ignore_ascii_case("integer"),
            Col::Spec(s, _) => s.data_type.trim().eq_ignore_ascii_case("integer"),
        }
    }
}

struct RebuildCol {
    name: String,
    /// Column it is copied from, if any.
    source: Option<String>,
    generated: bool,
    col: Col,
}

fn is_autoincrement(o: &ColumnOption) -> bool {
    let ColumnOption::DialectSpecific(tokens) = o else {
        return false;
    };
    tokens
        .iter()
        .any(|t| matches!(t, Token::Word(w) if w.value.eq_ignore_ascii_case("autoincrement")))
}

fn is_primary(o: &ColumnOption) -> bool {
    matches!(
        o,
        ColumnOption::Unique {
            is_primary: true,
            ..
        }
    )
}

fn is_plain_unique(o: &ColumnOption) -> bool {
    matches!(
        o,
        ColumnOption::Unique {
            is_primary: false,
            ..
        }
    )
}

fn ident_eq(a: &Ident, b: &str) -> bool {
    a.value.eq_ignore_ascii_case(b)
}

fn names_table(name: &ObjectName, table: &str) -> bool {
    name.0.last().is_some_and(|t| ident_eq(t, table))
}

fn cant(why: &str) -> AppError {
    AppError::validation(format!("LibraTable can't rebuild this SQLite table: {why}"))
        .hint("Change it with ALTER TABLE statements in a query tab.")
}

fn parse_create_table(sql: &str) -> AppResult<CreateTable> {
    let parsed = Parser::parse_sql(&SQLiteDialect {}, sql).map_err(|e| cant(&e.to_string()))?;
    match parsed.into_iter().next() {
        Some(Ast::CreateTable(ct)) => Ok(ct),
        _ => Err(cant("the table definition isn't a CREATE TABLE statement.")),
    }
}

/// The table as it will be after the change.
struct Model {
    cols: Vec<RebuildCol>,
    pk: Vec<String>,
    autoinc: bool,
    constraints: Vec<TableConstraint>,
    /// Columns dropped by the change (their indexes are not recreated).
    dropped: Vec<String>,
    without_rowid: bool,
}

impl Model {
    /// The table as it is now, with the renames done just before applied.
    fn load(ct: &CreateTable, renames: &[(String, String)]) -> Self {
        // Primary key as it is now: an inline PRIMARY KEY or a table constraint.
        let mut pk: Vec<String> = Vec::new();
        let mut autoinc = false;
        for c in &ct.columns {
            if c.options.iter().any(|o| is_primary(&o.option)) {
                pk = vec![c.name.value.clone()];
                autoinc = c.options.iter().any(|o| is_autoincrement(&o.option));
            }
        }
        let mut constraints = Vec::new();
        for tc in ct.constraints.iter().cloned() {
            match tc {
                TableConstraint::PrimaryKey { columns, .. } => {
                    pk = columns.iter().map(|i| i.value.clone()).collect()
                }
                other => constraints.push(other),
            }
        }
        for (from, to) in renames {
            for p in pk.iter_mut() {
                if p.eq_ignore_ascii_case(from) {
                    *p = to.clone();
                }
            }
        }
        let cols = ct
            .columns
            .iter()
            .cloned()
            .map(|c| original_column(c, renames))
            .collect();
        Self {
            cols,
            pk,
            autoinc,
            constraints,
            dropped: Vec::new(),
            without_rowid: ct.without_rowid,
        }
    }

    fn find(&mut self, name: &str) -> Option<&mut RebuildCol> {
        self.cols
            .iter_mut()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }

    fn apply(&mut self, op: &AlterOp, schema: &str, fks: &[ForeignKeyInfo]) -> AppResult<()> {
        match op {
            AlterOp::AddColumn { column } => self.add_column(column),
            AlterOp::DropColumn { name } => self.drop_column(name),
            AlterOp::ModifyColumn { column, .. } => self.modify_column(column),
            AlterOp::SetPrimaryKey { columns } => {
                self.pk = columns.clone();
                if self.pk.len() != 1 {
                    self.autoinc = false;
                }
                Ok(())
            }
            AlterOp::AddForeignKey { foreign_key } => self.add_foreign_key(schema, foreign_key),
            AlterOp::DropForeignKey { name } => {
                let fk = fks
                    .iter()
                    .find(|f| &f.name == name)
                    .ok_or_else(|| bad(format!("Foreign key \"{name}\" doesn't exist.")))?;
                self.drop_foreign_key(fk);
                Ok(())
            }
            AlterOp::DropIndex { .. } | AlterOp::AddIndex { .. } | AlterOp::SetComment { .. } => {
                Ok(())
            }
        }
    }

    fn add_column(&mut self, column: &ColumnSpec) -> AppResult<()> {
        if self.find(&column.name).is_some() {
            return Err(bad(format!("Column \"{}\" already exists.", column.name)));
        }
        if column.primary_key {
            self.pk = vec![column.name.clone()];
            self.autoinc = column.auto_increment;
        }
        self.cols.push(RebuildCol {
            name: column.name.clone(),
            source: None,
            generated: false,
            col: Col::Spec(column.clone(), None),
        });
        Ok(())
    }

    fn drop_column(&mut self, name: &str) -> AppResult<()> {
        let before = self.cols.len();
        self.cols.retain(|c| !c.name.eq_ignore_ascii_case(name));
        if self.cols.len() == before {
            return Err(bad(format!("Column \"{name}\" doesn't exist.")));
        }
        if self.pk.iter().any(|p| p.eq_ignore_ascii_case(name)) {
            return Err(bad(format!(
                "Column \"{name}\" is part of the primary key; change the primary key first."
            )));
        }
        self.dropped.push(name.to_string());
        Ok(())
    }

    fn modify_column(&mut self, column: &ColumnSpec) -> AppResult<()> {
        let Some(target) = self.find(&column.name) else {
            return Err(bad(format!("Column \"{}\" doesn't exist.", column.name)));
        };
        let placeholder = Col::Spec(column.clone(), None);
        let original = match std::mem::replace(&mut target.col, placeholder) {
            Col::Original(def) => Some(def),
            Col::Spec(_, def) => def,
        };
        target.col = Col::Spec(column.clone(), original);
        if self.pk.len() == 1 && self.pk[0].eq_ignore_ascii_case(&column.name) {
            self.autoinc = column.auto_increment;
        } else if column.auto_increment {
            return Err(bad(SQLITE_AUTOINCREMENT_RULE));
        }
        Ok(())
    }

    /// Validated and rendered the same way as in CREATE TABLE.
    fn add_foreign_key(&mut self, schema: &str, fk: &ForeignKeySpec) -> AppResult<()> {
        let text = fk_clause(D, schema, fk)?;
        let parsed = Parser::parse_sql(
            &SQLiteDialect {},
            &format!("CREATE TABLE x (a INT, {text})"),
        )
        .map_err(|e| cant(&e.to_string()))?;
        if let Some(Ast::CreateTable(x)) = parsed.into_iter().next() {
            self.constraints.extend(x.constraints);
        }
        Ok(())
    }

    /// Drops a table-level FOREIGN KEY, or else a column-level REFERENCES clause.
    fn drop_foreign_key(&mut self, fk: &ForeignKeyInfo) {
        let matches = |cols: &[Ident], table: &ObjectName| {
            cols.len() == fk.columns.len()
                && cols.iter().zip(&fk.columns).all(|(a, b)| ident_eq(a, b))
                && names_table(table, &fk.ref_table)
        };
        let before = self.constraints.len();
        self.constraints.retain(|c| match c {
            TableConstraint::ForeignKey {
                columns,
                foreign_table,
                ..
            } => !matches(columns, foreign_table),
            _ => true,
        });
        if self.constraints.len() != before || fk.columns.len() != 1 {
            return;
        }
        let references_target = |o: &ColumnOption| match o {
            ColumnOption::ForeignKey { foreign_table, .. } => {
                names_table(foreign_table, &fk.ref_table)
            }
            _ => false,
        };
        for c in self.cols.iter_mut() {
            if c.name.eq_ignore_ascii_case(&fk.columns[0]) {
                if let Some(def) = c.col.def_mut() {
                    def.options.retain(|o| !references_target(&o.option));
                }
            }
        }
    }

    fn validate(&self) -> AppResult<()> {
        for p in &self.pk {
            if !self.cols.iter().any(|c| c.name.eq_ignore_ascii_case(p)) {
                return Err(bad(format!("Primary key column \"{p}\" doesn't exist.")));
            }
        }
        if self.autoinc {
            let ok = self.pk.len() == 1
                && self
                    .cols
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(&self.pk[0]))
                    .is_some_and(|c| c.col.is_integer());
            if !ok {
                return Err(bad(SQLITE_AUTOINCREMENT_RULE));
            }
        }
        Ok(())
    }

    /// The CREATE TABLE of the new table, named `tmp`.
    fn create_sql(&self, schema: &str, tmp: &str) -> AppResult<String> {
        let inline_pk = self.pk.len() == 1 && self.autoinc;
        let mut lines = Vec::new();
        for c in &self.cols {
            let is_pk = inline_pk && c.name.eq_ignore_ascii_case(&self.pk[0]);
            lines.push(format!("  {}", column_line(&c.col, is_pk)?));
        }
        if !self.pk.is_empty() && !inline_pk {
            lines.push(format!("  PRIMARY KEY ({})", quoted_list(D, &self.pk)));
        }
        for tc in &self.constraints {
            lines.push(format!("  {tc}"));
        }
        let mut create = format!(
            "CREATE TABLE {} (\n{}\n)",
            qualified(D, schema, tmp),
            lines.join(",\n")
        );
        if self.without_rowid {
            create.push_str(" WITHOUT ROWID");
        }
        Ok(create)
    }

    /// Copies the rows of every column that existed before (generated ones compute).
    fn copy_sql(&self, schema: &str, tmp: &str, table: &str) -> Option<String> {
        let copied: Vec<&RebuildCol> = self
            .cols
            .iter()
            .filter(|c| c.source.is_some() && !c.generated)
            .collect();
        if copied.is_empty() {
            return None;
        }
        let list = |f: &dyn Fn(&RebuildCol) -> &str| {
            copied
                .iter()
                .map(|c| quote_ident(D, f(c)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        Some(format!(
            "INSERT INTO {} ({}) SELECT {} FROM {}",
            qualified(D, schema, tmp),
            list(&|c| c.name.as_str()),
            list(&|c| c.source.as_deref().unwrap_or_default()),
            qualified(D, schema, table)
        ))
    }

    fn uses_dropped_column(&self, o: &SqliteObject) -> bool {
        o.columns
            .iter()
            .any(|c| self.dropped.iter().any(|x| x.eq_ignore_ascii_case(c)))
    }
}

fn original_column(mut c: ColumnDef, renames: &[(String, String)]) -> RebuildCol {
    if let Some((_, to)) = renames.iter().find(|(from, _)| ident_eq(&c.name, from)) {
        c.name.value = to.clone();
    }
    let generated = c
        .options
        .iter()
        .any(|o| matches!(o.option, ColumnOption::Generated { .. }));
    let name = c.name.value.clone();
    c.options
        .retain(|o| !is_primary(&o.option) && !is_autoincrement(&o.option));
    RebuildCol {
        source: Some(name.clone()),
        name,
        generated,
        col: Col::Original(c),
    }
}

/// One column of the new CREATE TABLE.
fn column_line(col: &Col, is_pk: bool) -> AppResult<String> {
    match col {
        Col::Original(def) if is_pk => Ok(pk_autoincrement_line(def)),
        Col::Original(def) => Ok(def.to_string()),
        Col::Spec(spec, original) => {
            let mut s = column_def(D, spec, is_pk, true)?;
            if let Some(def) = original {
                push_kept_options(&mut s, spec, def);
            }
            Ok(s)
        }
    }
}

/// An original column with PRIMARY KEY AUTOINCREMENT right after the type.
fn pk_autoincrement_line(def: &ColumnDef) -> String {
    let mut s = format!("{} {} PRIMARY KEY AUTOINCREMENT", def.name, def.data_type);
    if let Some(coll) = &def.collation {
        s.push_str(&format!(" COLLATE {coll}"));
    }
    for o in &def.options {
        s.push(' ');
        s.push_str(&o.to_string());
    }
    s
}

/// The options of the original column that the user's spec doesn't describe.
fn push_kept_options(s: &mut String, spec: &ColumnSpec, def: &ColumnDef) {
    if let Some(coll) = &def.collation {
        s.push_str(&format!(" COLLATE {coll}"));
    }
    for o in &def.options {
        let keep = matches!(
            o.option,
            ColumnOption::Check(_)
                | ColumnOption::ForeignKey { .. }
                | ColumnOption::Generated { .. }
        ) || is_plain_unique(&o.option);
        // An explicit `unique` from the spec is already there.
        let dup_unique = spec.unique && is_plain_unique(&o.option);
        if keep && !dup_unique {
            s.push(' ');
            s.push_str(&o.to_string());
        }
    }
}

/// Index/trigger SQL names the table without a schema; put it back in the same one.
fn object_sql(o: &SqliteObject, schema: &str) -> String {
    let s = o.sql.clone();
    if schema != "main"
        && s.to_ascii_uppercase().starts_with("CREATE")
        && !s.contains(&quote_ident(D, schema))
    {
        return s.replacen(
            &format!("INDEX {}", quote_ident(D, &o.name)),
            &format!("INDEX {}", qualified(D, schema, &o.name)),
            1,
        );
    }
    s
}

/// The statements that rebuild `spec.table` from its current `CREATE TABLE`.
pub fn sqlite_rebuild_sql(
    spec: &Rebuild,
    table_sql: &str,
    objects: &[SqliteObject],
    foreign_keys: &[ForeignKeyInfo],
) -> AppResult<Vec<String>> {
    let ct = parse_create_table(table_sql)?;
    let mut model = Model::load(&ct, &spec.renames);
    for op in &spec.ops {
        model.apply(op, &spec.schema, foreign_keys)?;
    }
    model.validate()?;

    let (schema, table) = (spec.schema.as_str(), spec.table.as_str());
    let tmp = format!("{TMP_PREFIX}{table}");
    let mut out = vec![model.create_sql(schema, &tmp)?];
    out.extend(model.copy_sql(schema, &tmp, table));
    out.push(format!("DROP TABLE {}", qualified(D, schema, table)));
    // Legacy mode keeps SQLite from re-checking views and triggers mid-rename.
    out.push("PRAGMA legacy_alter_table = ON".into());
    out.push(format!(
        "ALTER TABLE {} RENAME TO {}",
        qualified(D, schema, &tmp),
        quote_ident(D, table)
    ));
    out.push("PRAGMA legacy_alter_table = OFF".into());
    for o in objects {
        if model.uses_dropped_column(o) {
            out.push(format!(
                "-- index {} is not recreated: it used a dropped column",
                quote_ident(D, &o.name)
            ));
        } else {
            out.push(object_sql(o, schema));
        }
    }
    Ok(out)
}

/// The `CREATE TABLE` of an SQLite table renamed to `to` (DuplicateTable).
pub fn sqlite_copy_sql(table_sql: &str, schema: &str, to: &str) -> AppResult<String> {
    let parsed = Parser::parse_sql(&SQLiteDialect {}, table_sql)
        .map_err(|e| AppError::validation(format!("Can't read the table definition: {e}")))?;
    let Some(Ast::CreateTable(mut ct)) = parsed.into_iter().next() else {
        return Err(AppError::validation(
            "The table definition isn't a CREATE TABLE statement.",
        ));
    };
    ct.name = ObjectName(vec![
        Ident::with_quote('"', schema),
        Ident::with_quote('"', to),
    ]);
    Ok(Ast::CreateTable(ct).to_string())
}
