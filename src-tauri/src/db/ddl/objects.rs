//! Changes to schema objects other than tables: drop a function, procedure, sequence,
//! materialized view, trigger, type or event; refresh a materialized view; reset a
//! sequence. Postgres routine arguments are read from the catalog (`context`), so the
//! text spliced into `DROP FUNCTION f(…)` never comes from the UI.

use crate::db::objects::{ObjectKind, ObjectRef};
use crate::db::sql::{literal, qualified, quote_ident};
use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::Driver;

use super::clauses::{bad, need_name, sql};
use super::plan::{Ctx, Step};
use super::spec::SchemaChange;

const PG_ROUTINE_ARGS: &str = "SELECT pg_get_function_identity_arguments(p.oid) \
    FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
    WHERE n.nspname = $1 AND p.proname = $2 AND p.prokind = $3 \
        AND ($4::text IS NULL OR pg_get_function_identity_arguments(p.oid) = $4)";

const PG_TYPTYPE: &str = "SELECT t.typtype::text FROM pg_type t \
    JOIN pg_namespace n ON n.oid = t.typnamespace WHERE n.nspname = $1 AND t.typname = $2";

/// What dropping `object` needs from a Postgres catalog: the routine's exact
/// arguments, or whether a type is a domain.
pub(super) async fn context(pool: &DbPool, schema: &str, object: &ObjectRef) -> AppResult<Ctx> {
    let mut ctx = Ctx::default();
    let DbPool::Postgres(p) = pool else {
        return Ok(ctx);
    };
    match object.kind {
        ObjectKind::Function | ObjectKind::Procedure => {
            let prokind = if object.kind == ObjectKind::Function {
                "f"
            } else {
                "p"
            };
            let found: Vec<String> = sqlx::query_scalar(PG_ROUTINE_ARGS)
                .bind(schema)
                .bind(&object.name)
                .bind(prokind)
                .bind(object.arguments.as_deref())
                .fetch_all(p)
                .await?;
            ctx.signature = Some(one_routine(found, object)?);
        }
        ObjectKind::Type => {
            let t: Option<String> = sqlx::query_scalar(PG_TYPTYPE)
                .bind(schema)
                .bind(&object.name)
                .fetch_optional(p)
                .await?;
            let domain = t.as_deref() == Some("d");
            ctx.object_kind = Some(if domain { "DOMAIN" } else { "TYPE" }.into());
        }
        _ => {}
    }
    Ok(ctx)
}

fn one_routine(mut found: Vec<String>, object: &ObjectRef) -> AppResult<String> {
    match found.len() {
        0 => Err(AppError::not_found(format!(
            "The {} \"{}\" was not found.",
            object.kind.label(),
            object.name
        ))),
        1 => Ok(found.remove(0)),
        n => Err(bad(format!(
            "\"{}\" has {n} overloads: pick one by its arguments.",
            object.name
        ))),
    }
}

/// The statements of the object changes (the other variants are handled in `plan`).
pub(super) fn steps(
    driver: Driver,
    change: &SchemaChange,
    ctx: &Ctx,
    steps: &mut Vec<Step>,
) -> AppResult<()> {
    let statement = match change {
        SchemaChange::DropObject {
            schema,
            object,
            cascade,
        } => drop_object(driver, schema, object, *cascade, ctx)?,
        SchemaChange::RefreshMaterializedView {
            schema,
            name,
            concurrently,
        } => {
            need_postgres(driver, ObjectKind::MaterializedView)?;
            need_name("The materialized view", name)?;
            let how = if *concurrently { " CONCURRENTLY" } else { "" };
            let target = qualified(driver, schema, name);
            format!("REFRESH MATERIALIZED VIEW{how} {target}")
        }
        SchemaChange::ResetSequence {
            schema,
            name,
            value,
        } => {
            need_postgres(driver, ObjectKind::Sequence)?;
            need_name("The sequence", name)?;
            // is_called = false: the next nextval() returns `value` itself.
            let target = literal(&qualified(driver, schema, name));
            format!("SELECT setval({target}, {value}, false)")
        }
        _ => unreachable!("not an object change"),
    };
    sql(steps, statement);
    Ok(())
}

fn need_postgres(driver: Driver, kind: ObjectKind) -> AppResult<()> {
    if driver == Driver::Postgres {
        Ok(())
    } else {
        Err(no_such(kind))
    }
}

fn no_such(kind: ObjectKind) -> AppError {
    bad(format!("This database has no {}s.", kind.label()))
}

fn drop_object(
    driver: Driver,
    schema: &str,
    object: &ObjectRef,
    cascade: bool,
    ctx: &Ctx,
) -> AppResult<String> {
    use Driver::{Mysql, Postgres, Sqlite};
    use ObjectKind::*;
    need_name("The object", &object.name)?;
    let target = qualified(driver, schema, &object.name);
    let cascade = if cascade && driver == Postgres {
        " CASCADE"
    } else {
        ""
    };
    let kw = routine_keyword(object.kind);
    Ok(match (driver, object.kind) {
        (Postgres, Function | Procedure) => {
            let args = ctx
                .signature
                .as_deref()
                .ok_or_else(|| bad("The routine's arguments weren't looked up."))?;
            format!("DROP {kw} {target}({args}){cascade}")
        }
        (Mysql, Function | Procedure) => format!("DROP {kw} {target}"),
        (Postgres, Sequence) => format!("DROP SEQUENCE {target}{cascade}"),
        (Postgres, MaterializedView) => format!("DROP MATERIALIZED VIEW {target}{cascade}"),
        (Postgres, Trigger) => {
            let table = object.table.as_deref().unwrap_or_default();
            need_name("The trigger's table", table)?;
            let on = qualified(driver, schema, table);
            let name = quote_ident(driver, &object.name);
            format!("DROP TRIGGER {name} ON {on}{cascade}")
        }
        (Mysql | Sqlite, Trigger) => format!("DROP TRIGGER {target}"),
        (Postgres, Type) => {
            let what = ctx.object_kind.as_deref().unwrap_or("TYPE");
            format!("DROP {what} {target}{cascade}")
        }
        (Mysql, Event) => format!("DROP EVENT {target}"),
        _ => return Err(no_such(object.kind)),
    })
}

fn routine_keyword(kind: ObjectKind) -> &'static str {
    if kind == ObjectKind::Procedure {
        "PROCEDURE"
    } else {
        "FUNCTION"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::{build, Ctx, Step};
    use super::*;

    fn change(v: serde_json::Value) -> SchemaChange {
        serde_json::from_value(v).unwrap()
    }

    fn one(driver: Driver, c: serde_json::Value, ctx: &Ctx) -> AppResult<String> {
        let plan = build(driver, &change(c), ctx)?;
        match plan.steps.as_slice() {
            [Step::Sql(s)] => Ok(s.clone()),
            other => panic!("expected one statement, got {other:?}"),
        }
    }

    fn drop(kind: &str, name: &str) -> serde_json::Value {
        json!({ "kind": "drop_object", "schema": "app", "object": { "kind": kind, "name": name } })
    }

    #[test]
    fn drops_routines_with_catalog_arguments() {
        let ctx = Ctx {
            signature: Some("a integer, b text".into()),
            ..Ctx::default()
        };
        let mut c = drop("function", "add");
        c["cascade"] = json!(true);
        assert_eq!(
            one(Driver::Postgres, c, &ctx).unwrap(),
            r#"DROP FUNCTION "app"."add"(a integer, b text) CASCADE"#
        );
        // Without the catalog lookup there is nothing to splice in.
        assert!(one(Driver::Postgres, drop("procedure", "p"), &Ctx::default()).is_err());
        assert_eq!(
            one(Driver::Mysql, drop("procedure", "p"), &Ctx::default()).unwrap(),
            "DROP PROCEDURE `app`.`p`"
        );
    }

    #[test]
    fn drops_other_objects_per_driver() {
        let none = Ctx::default();
        let pg = |c| one(Driver::Postgres, c, &none).unwrap();
        assert_eq!(pg(drop("sequence", "ids")), r#"DROP SEQUENCE "app"."ids""#);
        assert_eq!(
            pg(drop("materialized_view", "totals")),
            r#"DROP MATERIALIZED VIEW "app"."totals""#
        );
        let mut trg = drop("trigger", "audit");
        trg["object"]["table"] = json!("people");
        assert_eq!(pg(trg.clone()), r#"DROP TRIGGER "audit" ON "app"."people""#);
        assert_eq!(
            one(Driver::Sqlite, trg.clone(), &none).unwrap(),
            r#"DROP TRIGGER "app"."audit""#
        );
        assert_eq!(
            one(Driver::Mysql, trg, &none).unwrap(),
            "DROP TRIGGER `app`.`audit`"
        );
        let domain = Ctx {
            object_kind: Some("DOMAIN".into()),
            ..Ctx::default()
        };
        assert_eq!(
            one(Driver::Postgres, drop("type", "pos"), &domain).unwrap(),
            r#"DROP DOMAIN "app"."pos""#
        );
        assert_eq!(
            one(Driver::Mysql, drop("event", "nightly"), &none).unwrap(),
            "DROP EVENT `app`.`nightly`"
        );
        // Kinds the driver doesn't have are refused.
        assert!(one(Driver::Sqlite, drop("function", "f"), &none).is_err());
        assert!(one(Driver::Mysql, drop("sequence", "s"), &none).is_err());
        assert!(one(Driver::Postgres, drop("trigger", "t"), &none).is_err());
    }

    #[test]
    fn refreshes_and_resets() {
        let none = Ctx::default();
        let refresh = json!({
            "kind": "refresh_materialized_view", "schema": "app", "name": "totals",
            "concurrently": true
        });
        assert_eq!(
            one(Driver::Postgres, refresh.clone(), &none).unwrap(),
            r#"REFRESH MATERIALIZED VIEW CONCURRENTLY "app"."totals""#
        );
        assert!(one(Driver::Mysql, refresh, &none).is_err());
        let reset = json!({
            "kind": "reset_sequence", "schema": "app", "name": "it's", "value": 100
        });
        assert_eq!(
            one(Driver::Postgres, reset, &none).unwrap(),
            r#"SELECT setval('"app"."it''s"', 100, false)"#
        );
    }

    #[test]
    fn overloads_need_arguments() {
        let r = ObjectRef {
            kind: ObjectKind::Function,
            name: "f".into(),
            arguments: None,
            table: None,
        };
        assert_eq!(one_routine(vec!["x int".into()], &r).unwrap(), "x int");
        assert!(one_routine(vec![], &r).is_err());
        assert!(one_routine(vec!["".into(), "x int".into()], &r).is_err());
    }
}
