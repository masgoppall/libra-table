//! BigQuery catalog: projects (the database level), datasets (schemas), tables and
//! their schemas from tables.get, which also carries the declared (unenforced) keys.

use serde_json::Value;

use super::client::{seg, Api};
use super::sql::{literal, quote};
use super::values::Field;
use crate::db::meta::{ColumnInfo, ForeignKeyInfo, IndexInfo, ObjectInfo};
use crate::error::AppResult;

fn text(v: &Value, pointer: &str) -> String {
    v.pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub async fn projects(api: &Api) -> AppResult<Vec<String>> {
    let items = api.list("/projects", "projects", &[]).await?;
    Ok(items
        .iter()
        .map(|p| match text(p, "/projectReference/projectId") {
            id if id.is_empty() => text(p, "/id"),
            id => id,
        })
        .filter(|p| !p.is_empty())
        .collect())
}

pub async fn datasets(api: &Api, project: &str, all: bool) -> AppResult<Vec<String>> {
    let q = [("all", all.to_string())];
    let items = api
        .list(
            &format!("/projects/{}/datasets", seg(project)),
            "datasets",
            &q,
        )
        .await?;
    let mut names: Vec<String> = items
        .iter()
        .map(|d| text(d, "/datasetReference/datasetId"))
        .collect();
    names.sort();
    Ok(names)
}

pub async fn tables(api: &Api, project: &str, dataset: &str) -> AppResult<Vec<ObjectInfo>> {
    let path = format!(
        "/projects/{}/datasets/{}/tables",
        seg(project),
        seg(dataset)
    );
    let items = api
        .list(&path, "tables", &[("maxResults", "1000".into())])
        .await?;
    let mut out: Vec<ObjectInfo> = items
        .iter()
        .map(|t| ObjectInfo {
            name: text(t, "/tableReference/tableId"),
            kind: match text(t, "/type").as_str() {
                "VIEW" | "MATERIALIZED_VIEW" => "view",
                _ => "table",
            },
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// A table as tables.get describes it.
pub struct Table {
    pub kind: String,
    pub fields: Vec<Field>,
    pub primary_key: Vec<String>,
    pub num_rows: Option<i64>,
    raw: Value,
}

pub async fn table(api: &Api, project: &str, dataset: &str, table: &str) -> AppResult<Table> {
    let raw = api
        .get(
            &format!(
                "/projects/{}/datasets/{}/tables/{}",
                seg(project),
                seg(dataset),
                seg(table)
            ),
            &[],
        )
        .await?;
    let fields = serde_json::from_value(raw.pointer("/schema/fields").cloned().unwrap_or_default())
        .unwrap_or_default();
    let primary_key = raw
        .pointer("/tableConstraints/primaryKey/columns")
        .and_then(Value::as_array)
        .map(|c| {
            c.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(Table {
        kind: text(&raw, "/type"),
        num_rows: text(&raw, "/numRows").parse().ok(),
        fields,
        primary_key,
        raw,
    })
}

impl Table {
    pub fn is_view(&self) -> bool {
        matches!(self.kind.as_str(), "VIEW" | "MATERIALIZED_VIEW")
    }

    pub fn columns(&self) -> Vec<ColumnInfo> {
        self.fields
            .iter()
            .map(|f| {
                let pk = self.primary_key.iter().position(|k| k == &f.name);
                f.column(pk.map(|p| p as i32 + 1))
            })
            .collect()
    }

    pub fn indexes(&self) -> Vec<IndexInfo> {
        if self.primary_key.is_empty() {
            return Vec::new();
        }
        vec![IndexInfo {
            name: "PRIMARY KEY".into(),
            columns: self.primary_key.clone(),
            unique: true,
            primary: true,
            definition: format!("PRIMARY KEY ({}) NOT ENFORCED", list(&self.primary_key)),
            ..Default::default()
        }]
    }

    pub fn foreign_keys(&self) -> Vec<ForeignKeyInfo> {
        let Some(fks) = self
            .raw
            .pointer("/tableConstraints/foreignKeys")
            .and_then(Value::as_array)
        else {
            return Vec::new();
        };
        fks.iter()
            .map(|fk| {
                let refs = fk.get("columnReferences").and_then(Value::as_array);
                let side = |k: &str| -> Vec<String> {
                    refs.into_iter()
                        .flatten()
                        .map(|r| text(r, &format!("/{k}")))
                        .collect()
                };
                let (columns, ref_columns) = (side("referencingColumn"), side("referencedColumn"));
                let ref_table = format!(
                    "{}.{}",
                    text(fk, "/referencedTable/datasetId"),
                    text(fk, "/referencedTable/tableId")
                );
                ForeignKeyInfo {
                    name: text(fk, "/name"),
                    definition: format!(
                        "FOREIGN KEY ({}) REFERENCES {} ({}) NOT ENFORCED",
                        list(&columns),
                        ref_table,
                        list(&ref_columns)
                    ),
                    columns,
                    ref_table,
                    ref_columns,
                    on_update: String::new(),
                    on_delete: String::new(),
                }
            })
            .collect()
    }

    /// CREATE TABLE / VIEW text rebuilt from the metadata (no query job needed).
    pub fn ddl(&self, name: &str) -> String {
        if let Some(q) = self.raw.pointer("/view/query").and_then(Value::as_str) {
            return format!("CREATE VIEW {name} AS\n{q}");
        }
        if let Some(q) = self
            .raw
            .pointer("/materializedView/query")
            .and_then(Value::as_str)
        {
            return format!("CREATE MATERIALIZED VIEW {name} AS\n{q}");
        }
        let mut lines: Vec<String> = self
            .columns()
            .iter()
            .map(|c| {
                let mut l = format!("  {} {}", quote(&c.name), c.data_type);
                if let Some(d) = &c.default {
                    l.push_str(&format!(" DEFAULT {d}"));
                }
                if !c.nullable {
                    l.push_str(" NOT NULL");
                }
                if let Some(d) = &c.comment {
                    l.push_str(&format!(" OPTIONS(description={})", literal(d)));
                }
                l
            })
            .collect();
        if !self.primary_key.is_empty() {
            lines.push(format!(
                "  PRIMARY KEY ({}) NOT ENFORCED",
                list(&self.primary_key)
            ));
        }
        format!("CREATE TABLE {name}\n(\n{}\n);", lines.join(",\n"))
    }
}

fn list(names: &[String]) -> String {
    names
        .iter()
        .map(|n| quote(n))
        .collect::<Vec<_>>()
        .join(", ")
}
