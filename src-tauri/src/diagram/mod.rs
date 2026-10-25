//! The ER diagram's data (DiagramWindow): every table of a schema with its columns and
//! foreign keys, read in two catalog queries per driver instead of one per table.
//! Diagram layouts (table positions) are kept per connection in `diagrams/<id>.json`.

mod mysql;
mod postgres;
mod sqlite;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::AppResult;
use crate::storage::conn_files::ConnFiles;

/// Larger schemas are cut here; the diagram says so.
pub const MAX_TABLES: usize = 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramTable {
    pub name: String,
    pub columns: Vec<DiagramColumn>,
    pub foreign_keys: Vec<DiagramForeignKey>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiagram {
    pub schema: String,
    pub tables: Vec<DiagramTable>,
    pub truncated: bool,
}

/// What a driver reads: (table, column) rows and (table, foreign key) rows, by table.
type Loaded = (
    Vec<(String, DiagramColumn)>,
    Vec<(String, DiagramForeignKey)>,
);

/// Groups (table, column) rows into tables, keeping the row order.
fn group(rows: Vec<(String, DiagramColumn)>) -> Vec<DiagramTable> {
    let mut tables: Vec<DiagramTable> = Vec::new();
    for (table, col) in rows {
        match tables.last_mut() {
            Some(t) if t.name == table => t.columns.push(col),
            _ => tables.push(DiagramTable {
                name: table,
                columns: vec![col],
                foreign_keys: Vec::new(),
            }),
        }
    }
    tables
}

/// Attaches foreign keys (by owning table) to the tables.
fn attach(tables: &mut [DiagramTable], fks: Vec<(String, DiagramForeignKey)>) {
    for (table, fk) in fks {
        if let Some(t) = tables.iter_mut().find(|t| t.name == table) {
            t.foreign_keys.push(fk);
        }
    }
}

pub async fn schema_diagram(pool: &DbPool, schema: &str) -> AppResult<SchemaDiagram> {
    let (columns, fks) = match pool {
        DbPool::Postgres(p) => postgres::load(p, schema).await?,
        DbPool::Mysql(p) => mysql::load(p, schema).await?,
        DbPool::Sqlite(p) => sqlite::load(p, schema).await?,
        DbPool::Engine(_) => return Err(crate::db::engine::sqlx_only()),
    };
    let mut tables = group(columns);
    let truncated = tables.len() > MAX_TABLES;
    tables.truncate(MAX_TABLES);
    attach(&mut tables, fks);
    Ok(SchemaDiagram {
        schema: schema.to_string(),
        tables,
        truncated,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Table positions of one diagram, by table name.
pub type Layout = BTreeMap<String, Point>;

#[derive(Debug, Default, Serialize, Deserialize)]
struct LayoutsFile {
    /// `<database>/<schema>` → layout.
    #[serde(default)]
    layouts: BTreeMap<String, Layout>,
}

fn layouts(data_dir: &Path) -> ConnFiles {
    ConnFiles::new(data_dir, "diagrams")
}

pub fn load_layout(data_dir: &Path, connection_id: &str, key: &str) -> AppResult<Layout> {
    let file: LayoutsFile = layouts(data_dir).load(connection_id)?;
    Ok(file.layouts.get(key).cloned().unwrap_or_default())
}

/// Saves one diagram's positions; an empty layout forgets it (back to automatic).
pub fn save_layout(
    data_dir: &Path,
    connection_id: &str,
    key: &str,
    layout: Layout,
) -> AppResult<()> {
    let files = layouts(data_dir);
    let mut file: LayoutsFile = files.load(connection_id)?;
    if layout.is_empty() {
        file.layouts.remove(key);
    } else {
        file.layouts.insert(key.to_string(), layout);
    }
    files.save(connection_id, &file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str) -> DiagramColumn {
        DiagramColumn {
            name: name.into(),
            data_type: "int".into(),
            nullable: false,
            primary_key: name == "id",
        }
    }

    #[test]
    fn groups_rows_and_attaches_keys() {
        let rows = vec![
            ("a".to_string(), col("id")),
            ("a".to_string(), col("b_id")),
            ("b".to_string(), col("id")),
        ];
        let mut tables = group(rows);
        let fk = DiagramForeignKey {
            name: "a_b".into(),
            columns: vec!["b_id".into()],
            ref_schema: "public".into(),
            ref_table: "b".into(),
            ref_columns: vec!["id".into()],
        };
        attach(
            &mut tables,
            vec![("a".into(), fk), ("gone".into(), col_fk())],
        );
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].columns.len(), 2);
        assert_eq!(tables[0].foreign_keys[0].ref_table, "b");
    }

    fn col_fk() -> DiagramForeignKey {
        DiagramForeignKey {
            name: "x".into(),
            columns: vec![],
            ref_schema: String::new(),
            ref_table: String::new(),
            ref_columns: vec![],
        }
    }

    #[test]
    fn layouts_roundtrip_per_key() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Layout::new();
        l.insert("people".into(), Point { x: 10.0, y: 20.5 });
        save_layout(dir.path(), "c1", "app/public", l.clone()).unwrap();
        assert_eq!(load_layout(dir.path(), "c1", "app/public").unwrap(), l);
        assert!(load_layout(dir.path(), "c1", "app/other")
            .unwrap()
            .is_empty());
        save_layout(dir.path(), "c1", "app/public", Layout::new()).unwrap();
        assert!(load_layout(dir.path(), "c1", "app/public")
            .unwrap()
            .is_empty());
    }
}
