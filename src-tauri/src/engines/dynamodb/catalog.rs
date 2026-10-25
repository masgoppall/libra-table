//! Tables and their "columns": the key attributes (from DescribeTable) first, then the
//! union of the attributes seen in a sample scan, each typed by what was seen.

use serde_json::{json, Value};

use super::client::Client;
use super::values::{kind, merge_type};
use crate::db::meta::{ColumnInfo, IndexInfo, ObjectInfo};
use crate::error::AppResult;

/// Items read to discover the attributes of a table.
const SAMPLE: u32 = 200;

pub async fn tables(c: &Client) -> AppResult<Vec<ObjectInfo>> {
    let mut out = Vec::new();
    let mut start: Option<Value> = None;
    loop {
        let mut body = json!({ "Limit": 100 });
        if let Some(s) = start.take() {
            body["ExclusiveStartTableName"] = s;
        }
        let page = c.call("ListTables", &body).await?;
        if let Some(names) = page.get("TableNames").and_then(Value::as_array) {
            out.extend(names.iter().filter_map(Value::as_str).map(|n| ObjectInfo {
                name: n.to_string(),
                kind: "table",
            }));
        }
        match page.get("LastEvaluatedTableName") {
            Some(v) if !v.is_null() => start = Some(v.clone()),
            _ => return Ok(out),
        }
    }
}

pub async fn describe(c: &Client, table: &str) -> AppResult<Value> {
    let d = c
        .call("DescribeTable", &json!({ "TableName": table }))
        .await?;
    Ok(d.get("Table").cloned().unwrap_or_default())
}

/// (attribute, HASH | RANGE) of a key schema.
fn key_schema(v: Option<&Value>) -> Vec<(String, String)> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|k| {
            let s = |f: &str| k.get(f).and_then(Value::as_str).unwrap_or("").to_string();
            (s("AttributeName"), s("KeyType"))
        })
        .collect()
}

fn attribute_type(desc: &Value, name: &str) -> String {
    desc.get("AttributeDefinitions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|d| d.get("AttributeName").and_then(Value::as_str) == Some(name))
        .and_then(|d| d.get("AttributeType").and_then(Value::as_str))
        .unwrap_or("S")
        .to_string()
}

pub async fn columns(c: &Client, table: &str) -> AppResult<Vec<ColumnInfo>> {
    let desc = describe(c, table).await?;
    let mut cols: Vec<ColumnInfo> = key_schema(desc.get("KeySchema"))
        .into_iter()
        .map(|(name, key)| {
            let hash = key == "HASH";
            ColumnInfo {
                data_type: attribute_type(&desc, &name),
                nullable: false,
                default: None,
                pk_position: Some(if hash { 1 } else { 2 }),
                auto_increment: false,
                comment: Some(if hash { "partition key" } else { "sort key" }.into()),
                name,
            }
        })
        .collect();
    cols.sort_by_key(|c| c.pk_position);
    let sample = c
        .call("Scan", &json!({ "TableName": table, "Limit": SAMPLE }))
        .await?;
    let mut seen: Vec<(String, String)> = Vec::new();
    for item in sample
        .get("Items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(map) = item.as_object() else {
            continue;
        };
        for (name, av) in map {
            if cols.iter().any(|c| &c.name == name) {
                continue;
            }
            match seen.iter_mut().find(|(n, _)| n == name) {
                Some((_, t)) => *t = merge_type(t, kind(av)),
                None => seen.push((name.clone(), kind(av).to_string())),
            }
        }
    }
    seen.sort();
    cols.extend(seen.into_iter().map(|(name, ty)| ColumnInfo {
        name,
        data_type: ty,
        nullable: true,
        default: None,
        pk_position: None,
        auto_increment: false,
        comment: None,
    }));
    Ok(cols)
}

fn index(name: &str, kind: &str, v: &Value, primary: bool) -> IndexInfo {
    let keys = key_schema(v.get("KeySchema"));
    let projection = v
        .pointer("/Projection/ProjectionType")
        .and_then(Value::as_str)
        .unwrap_or("");
    let parts: Vec<String> = keys.iter().map(|(n, k)| format!("{k} {n}")).collect();
    let mut definition = format!("{kind} ({})", parts.join(", "));
    if !projection.is_empty() {
        definition.push_str(&format!(" projection {projection}"));
    }
    IndexInfo {
        name: name.to_string(),
        columns: keys.into_iter().map(|(n, _)| n).collect(),
        unique: primary,
        primary,
        definition,
    }
}

/// The primary key and the secondary indexes of a table description.
pub fn indexes(desc: &Value) -> Vec<IndexInfo> {
    let mut out = vec![index("PRIMARY", "PRIMARY KEY", desc, true)];
    for (field, kind) in [
        ("GlobalSecondaryIndexes", "GSI"),
        ("LocalSecondaryIndexes", "LSI"),
    ] {
        for ix in desc
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = ix.get("IndexName").and_then(Value::as_str).unwrap_or("");
            out.push(index(name, kind, ix, false));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_from_a_description() {
        let desc = json!({
            "KeySchema": [{"AttributeName": "pk", "KeyType": "HASH"},
                          {"AttributeName": "sk", "KeyType": "RANGE"}],
            "GlobalSecondaryIndexes": [{
                "IndexName": "by_email",
                "KeySchema": [{"AttributeName": "email", "KeyType": "HASH"}],
                "Projection": {"ProjectionType": "ALL"}
            }]
        });
        let ix = indexes(&desc);
        assert_eq!(ix[0].columns, vec!["pk", "sk"]);
        assert_eq!(ix[0].definition, "PRIMARY KEY (HASH pk, RANGE sk)");
        assert_eq!(ix[1].definition, "GSI (HASH email) projection ALL");
        assert!(!ix[1].unique);
        assert_eq!(attribute_type(&json!({}), "x"), "S");
    }
}
