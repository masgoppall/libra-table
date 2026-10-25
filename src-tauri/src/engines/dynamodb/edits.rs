//! Grid edits as PutItem / UpdateItem / DeleteItem requests. Updates and deletes carry
//! `attribute_exists(<key>)` so they hit exactly the one existing item; inserts carry
//! `attribute_not_exists(<key>)` so they never overwrite one. Several changes commit
//! together through TransactWriteItems.

use serde_json::{json, Map, Value};

use super::client::{error_code, Client};
use super::filter::Expr;
use super::values::{from_json, typed_text};
use crate::db::changes::Change;
use crate::db::meta::ColumnInfo;
use crate::engines::cloud_common::not_one_row;
use crate::error::{AppError, AppResult};

/// TransactWriteItems takes at most this many actions.
const MAX_TRANSACTION: usize = 100;

/// One request: the action (`PutItem`…) and its body.
pub struct Write {
    pub action: &'static str,
    pub body: Value,
    pub insert: bool,
}

fn keys(cols: &[ColumnInfo]) -> Vec<&ColumnInfo> {
    let mut k: Vec<&ColumnInfo> = cols.iter().filter(|c| c.pk_position.is_some()).collect();
    k.sort_by_key(|c| c.pk_position);
    k
}

fn column<'a>(cols: &'a [ColumnInfo], name: &str) -> AppResult<&'a ColumnInfo> {
    cols.iter()
        .find(|c| c.name == name)
        .ok_or_else(|| AppError::validation(format!("Unknown column \"{name}\".")))
}

fn key_value(c: &ColumnInfo, v: Option<&Value>) -> AppResult<Value> {
    let text = match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => {
            return Err(AppError::validation(format!(
                "Missing key value for \"{}\".",
                c.name
            )))
        }
    };
    typed_text(&text, &c.data_type)
}

fn key_map(cols: &[ColumnInfo], pk: &Map<String, Value>) -> AppResult<Map<String, Value>> {
    keys(cols)
        .into_iter()
        .map(|c| Ok((c.name.clone(), key_value(c, pk.get(&c.name))?)))
        .collect()
}

/// `attribute_exists(#k0) AND …` (or not_exists) over the key attributes.
fn key_check(cols: &[ColumnInfo], e: &mut Expr, exists: bool) -> String {
    let f = if exists {
        "attribute_exists"
    } else {
        "attribute_not_exists"
    };
    keys(cols)
        .iter()
        .map(|c| format!("{f}({})", e.name(&c.name)))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn with_expr(mut body: Map<String, Value>, e: &Expr) -> Value {
    e.apply(&mut body);
    Value::Object(body)
}

fn not_default(name: &str, v: &Value) -> AppResult<()> {
    if v.get("$default").and_then(Value::as_bool) == Some(true) {
        return Err(AppError::validation(format!(
            "DynamoDB has no default values (attribute \"{name}\")."
        )));
    }
    Ok(())
}

fn update(
    t: &str,
    cols: &[ColumnInfo],
    pk: &Map<String, Value>,
    values: &Map<String, Value>,
) -> AppResult<Write> {
    let mut e = Expr::default();
    let (mut sets, mut removes) = (Vec::new(), Vec::new());
    for (name, v) in values {
        let c = column(cols, name)?;
        if c.pk_position.is_some() {
            return Err(AppError::validation(format!(
                "Key attribute \"{name}\" can't be changed; add a new item and delete this one."
            )));
        }
        not_default(name, v)?;
        if v.is_null() {
            removes.push(e.name(name));
        } else {
            let av = from_json(v, &c.data_type)?;
            let n = e.name(name);
            sets.push(format!("{n} = {}", e.value(av)));
        }
    }
    let mut update = Vec::new();
    if !sets.is_empty() {
        update.push(format!("SET {}", sets.join(", ")));
    }
    if !removes.is_empty() {
        update.push(format!("REMOVE {}", removes.join(", ")));
    }
    let mut body = Map::new();
    body.insert("TableName".into(), t.into());
    body.insert("Key".into(), Value::Object(key_map(cols, pk)?));
    body.insert("UpdateExpression".into(), update.join(" ").into());
    body.insert(
        "ConditionExpression".into(),
        key_check(cols, &mut e, true).into(),
    );
    Ok(Write {
        action: "UpdateItem",
        body: with_expr(body, &e),
        insert: false,
    })
}

fn insert(t: &str, cols: &[ColumnInfo], values: &Map<String, Value>) -> AppResult<Write> {
    let mut item = Map::new();
    for (name, v) in values {
        let c = column(cols, name)?;
        not_default(name, v)?;
        if c.pk_position.is_some() {
            item.insert(name.clone(), key_value(c, Some(v))?);
        } else if !v.is_null() {
            item.insert(name.clone(), from_json(v, &c.data_type)?);
        }
    }
    for k in keys(cols) {
        if !item.contains_key(&k.name) {
            return Err(AppError::validation(format!(
                "Enter a value for the key attribute \"{}\".",
                k.name
            )));
        }
    }
    let mut e = Expr::default();
    let mut body = Map::new();
    body.insert("TableName".into(), t.into());
    body.insert("Item".into(), Value::Object(item));
    body.insert(
        "ConditionExpression".into(),
        key_check(cols, &mut e, false).into(),
    );
    Ok(Write {
        action: "PutItem",
        body: with_expr(body, &e),
        insert: true,
    })
}

fn delete(t: &str, cols: &[ColumnInfo], pk: &Map<String, Value>) -> AppResult<Write> {
    let mut e = Expr::default();
    let mut body = Map::new();
    body.insert("TableName".into(), t.into());
    body.insert("Key".into(), Value::Object(key_map(cols, pk)?));
    body.insert(
        "ConditionExpression".into(),
        key_check(cols, &mut e, true).into(),
    );
    Ok(Write {
        action: "DeleteItem",
        body: with_expr(body, &e),
        insert: false,
    })
}

pub fn build(table: &str, cols: &[ColumnInfo], changes: &[Change]) -> AppResult<Vec<Write>> {
    if keys(cols).is_empty() {
        return Err(AppError::validation(
            "The table's key schema is unknown; refresh it.",
        ));
    }
    let mut out = Vec::new();
    for ch in changes {
        out.push(match ch {
            Change::Update { values, .. } if values.is_empty() => continue,
            Change::Update { pk, values } => update(table, cols, pk, values)?,
            Change::Insert { values } => insert(table, cols, values)?,
            Change::Delete { pk } => delete(table, cols, pk)?,
        });
    }
    Ok(out)
}

/// The preview line of one request.
pub fn preview(w: &Write) -> String {
    format!("{} {}", w.action, w.body)
}

fn duplicate(change: usize) -> AppError {
    AppError::query(format!(
        "Change {change} adds an item whose key already exists, so nothing was saved."
    ))
    .hint("Edit the existing item instead, or use another key.")
}

/// Sends the writes: one request, or one transaction for several.
pub async fn apply(c: &Client, writes: &[Write]) -> AppResult<u64> {
    match writes {
        [] => Ok(0),
        [w] => match c.call(w.action, &w.body).await {
            Ok(_) => Ok(1),
            Err(e) if error_code(&e) == "ConditionalCheckFailedException" => Err(if w.insert {
                duplicate(1)
            } else {
                not_one_row(1, 0)
            }),
            Err(e) => Err(e),
        },
        _ if writes.len() > MAX_TRANSACTION => Err(AppError::validation(format!(
            "DynamoDB commits at most {MAX_TRANSACTION} changes at once; save in smaller steps."
        ))),
        _ => {
            let items: Vec<Value> = writes
                .iter()
                .map(|w| {
                    let key = match w.action {
                        "PutItem" => "Put",
                        "UpdateItem" => "Update",
                        _ => "Delete",
                    };
                    json!({ key: w.body })
                })
                .collect();
            match c
                .call("TransactWriteItems", &json!({ "TransactItems": items }))
                .await
            {
                Ok(_) => Ok(writes.len() as u64),
                Err(e) => Err(cancelled_change(&e, writes).unwrap_or(e)),
            }
        }
    }
}

/// The change whose condition cancelled a transaction.
fn cancelled_change(e: &AppError, writes: &[Write]) -> Option<AppError> {
    let reasons = e.data.as_ref()?.get("reasons")?.as_array()?;
    let i = reasons
        .iter()
        .position(|r| r.get("Code").and_then(Value::as_str) == Some("ConditionalCheckFailed"))?;
    Some(if writes.get(i)?.insert {
        duplicate(i + 1)
    } else {
        not_one_row(i + 1, 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str, pk: Option<i32>) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            data_type: ty.into(),
            nullable: pk.is_none(),
            default: None,
            pk_position: pk,
            auto_increment: false,
            comment: None,
        }
    }

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn requests_with_one_item_conditions() {
        let cols = [
            col("pk", "S", Some(1)),
            col("n", "N", None),
            col("doc", "M", None),
        ];
        let w = build(
            "t",
            &cols,
            &[
                Change::Update {
                    pk: map(json!({"pk": "a"})),
                    values: map(json!({"n": "5", "doc": null})),
                },
                Change::Insert {
                    values: map(json!({"pk": "b", "doc": "{\"x\": 1}"})),
                },
                Change::Delete {
                    pk: map(json!({"pk": "c"})),
                },
            ],
        )
        .unwrap();
        assert_eq!(
            w[0].body,
            json!({
                "TableName": "t", "Key": {"pk": {"S": "a"}},
                "UpdateExpression": "SET #n1 = :v0 REMOVE #n0",
                "ConditionExpression": "attribute_exists(#n2)",
                "ExpressionAttributeNames": {"#n0": "doc", "#n1": "n", "#n2": "pk"},
                "ExpressionAttributeValues": {":v0": {"N": "5"}}
            })
        );
        assert_eq!(
            w[1].body["Item"],
            json!({"pk": {"S": "b"}, "doc": {"M": {"x": {"N": "1"}}}})
        );
        assert_eq!(
            w[1].body["ConditionExpression"],
            "attribute_not_exists(#n0)"
        );
        assert_eq!(w[2].action, "DeleteItem");
        assert!(preview(&w[2]).starts_with("DeleteItem {"));
    }

    #[test]
    fn refuses_key_changes_defaults_and_missing_keys() {
        let cols = [col("pk", "S", Some(1)), col("n", "N", None)];
        let up = |v: Value| Change::Update {
            pk: map(json!({"pk": "a"})),
            values: map(v),
        };
        assert!(build("t", &cols, &[up(json!({"pk": "z"}))]).is_err());
        assert!(build("t", &cols, &[up(json!({"n": {"$default": true}}))]).is_err());
        let ins = Change::Insert {
            values: map(json!({"n": 1})),
        };
        assert!(build("t", &cols, &[ins]).is_err());
        let e = AppError::query("x").data(json!({"reasons": [{"Code": "None"},
            {"Code": "ConditionalCheckFailed"}]}));
        let w = build("t", &cols, &[up(json!({"n": 1})), up(json!({"n": 2}))]).unwrap();
        let err = cancelled_change(&e, &w).unwrap();
        assert!(err.message.starts_with("Change 2 affected 0 rows"));
    }
}
