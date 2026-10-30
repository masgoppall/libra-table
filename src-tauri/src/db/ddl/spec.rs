//! What the UI sends to describe a schema change. Serde shapes are part of the IPC
//! contract (`kind` / `op` tags, camelCase fields).

use serde::Deserialize;

use crate::db::objects::ObjectRef;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSpec {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    /// An SQL expression (`0`, `'text'`, `now()`); empty or missing means no default.
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub primary_key: bool,
    #[serde(default)]
    pub auto_increment: bool,
    /// Only used when the column is created.
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub comment: Option<String>,
    /// MySQL text columns: CHARACTER SET and COLLATE (ignored by other types and drivers).
    #[serde(default)]
    pub charset: Option<String>,
    #[serde(default)]
    pub collation: Option<String>,
    /// MySQL column attributes written as given ("ON UPDATE CURRENT_TIMESTAMP",
    /// "SERIAL DEFAULT VALUE"…); AUTO_INCREMENT is `auto_increment`.
    #[serde(default)]
    pub extra: Option<String>,
}

/// Where a MySQL column goes (`FIRST`, `AFTER other`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnPosition {
    First,
    After(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexSpec {
    /// Generated from the table and columns when empty.
    #[serde(default)]
    pub name: String,
    pub columns: Vec<String>,
    #[serde(default)]
    pub unique: bool,
    /// BTREE, HASH (MySQL) or an access method (Postgres); the default when empty.
    #[serde(default)]
    pub algorithm: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeySpec {
    #[serde(default)]
    pub name: String,
    pub columns: Vec<String>,
    /// Same schema as the table when empty.
    #[serde(default)]
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    #[serde(default)]
    pub on_update: String,
    #[serde(default)]
    pub on_delete: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DatabaseOptions {
    /// MySQL
    pub charset: String,
    pub collation: String,
    /// Postgres
    pub encoding: String,
    pub template: String,
    pub owner: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SchemaChange {
    CreateDatabase {
        name: String,
        #[serde(default)]
        options: DatabaseOptions,
    },
    DropDatabase {
        name: String,
        /// Postgres 13+: disconnect other sessions first.
        #[serde(default)]
        force: bool,
    },
    /// Postgres only; nobody (LibraTable included) may be connected to it.
    RenameDatabase {
        from: String,
        to: String,
    },
    CreateSchema {
        name: String,
    },
    DropSchema {
        name: String,
        #[serde(default)]
        cascade: bool,
    },
    CreateTable {
        schema: String,
        name: String,
        columns: Vec<ColumnSpec>,
        #[serde(default)]
        indexes: Vec<IndexSpec>,
        #[serde(default)]
        foreign_keys: Vec<ForeignKeySpec>,
        #[serde(default)]
        comment: Option<String>,
    },
    AlterTable {
        schema: String,
        name: String,
        ops: Vec<AlterOp>,
        /// A new name, applied after the ops.
        #[serde(default)]
        rename_to: Option<String>,
    },
    RenameTable {
        schema: String,
        from: String,
        to: String,
    },
    /// A table or a view; which one is looked up.
    DropTable {
        schema: String,
        name: String,
        #[serde(default)]
        cascade: bool,
    },
    TruncateTable {
        schema: String,
        name: String,
        #[serde(default)]
        cascade: bool,
    },
    DuplicateTable {
        schema: String,
        from: String,
        to: String,
        #[serde(default)]
        with_data: bool,
    },
    /// A function, procedure, sequence, materialized view, trigger, type or event.
    DropObject {
        schema: String,
        object: ObjectRef,
        #[serde(default)]
        cascade: bool,
    },
    RefreshMaterializedView {
        schema: String,
        name: String,
        #[serde(default)]
        concurrently: bool,
    },
    /// The sequence's next value becomes `value` (setval).
    ResetSequence {
        schema: String,
        name: String,
        value: i64,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum AlterOp {
    AddColumn {
        column: ColumnSpec,
        /// MySQL only; at the end when absent.
        #[serde(default)]
        position: Option<ColumnPosition>,
    },
    DropColumn {
        name: String,
    },
    /// `original` is the column as it was loaded; only what differs is changed. A new
    /// `column.name` renames it.
    ModifyColumn {
        original: ColumnSpec,
        column: ColumnSpec,
        /// MySQL only: move the column.
        #[serde(default)]
        position: Option<ColumnPosition>,
    },
    /// The whole new primary key (empty = none).
    SetPrimaryKey {
        columns: Vec<String>,
    },
    AddIndex {
        index: IndexSpec,
    },
    DropIndex {
        name: String,
    },
    AddForeignKey {
        foreign_key: ForeignKeySpec,
    },
    DropForeignKey {
        name: String,
    },
    SetComment {
        comment: Option<String>,
    },
}

impl AlterOp {
    /// Runs drops before renames before additions, whatever order the UI sent.
    pub(super) fn phase(&self) -> u8 {
        match self {
            AlterOp::DropForeignKey { .. } => 0,
            AlterOp::DropIndex { .. } => 1,
            AlterOp::DropColumn { .. } => 2,
            AlterOp::ModifyColumn { .. } => 3,
            AlterOp::AddColumn { .. } => 4,
            AlterOp::SetPrimaryKey { .. } => 5,
            AlterOp::AddIndex { .. } => 6,
            AlterOp::AddForeignKey { .. } => 7,
            AlterOp::SetComment { .. } => 8,
        }
    }
}
