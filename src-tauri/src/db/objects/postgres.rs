//! Postgres schema objects from pg_catalog. Objects that belong to an extension are left
//! out, as `\df` and `\dT` do.
// query_as tuples mirror the SELECT lists; naming each one would not make them clearer.
#![allow(clippy::type_complexity)]

use sqlx::PgPool;

use crate::db::sql::{qualified, quote_ident};
use crate::error::AppResult;
use crate::models::Driver;

use super::{
    missing, terminated, ObjectDefinition, ObjectKind, ObjectRef, SchemaObject, SequenceInfo,
};

mod ddl;

pub(super) use ddl::{sequence_ddl, trigger_detail, type_ddl, type_detail, TypeRow};

const D: Driver = Driver::Postgres;

const FUNCTIONS: &str = "SELECT p.proname::text, p.prokind::text, \
        pg_get_function_identity_arguments(p.oid), pg_get_function_result(p.oid) \
    FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
    WHERE n.nspname = $1 AND p.prokind IN ('f','p') AND NOT EXISTS (SELECT 1 FROM pg_depend d \
        WHERE d.classid = 'pg_proc'::regclass AND d.objid = p.oid AND d.deptype = 'e') \
    ORDER BY 1, 3";

const RELATIONS: &str = "SELECT c.relname::text, c.relkind::text, c.relispopulated \
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND c.relkind IN ('S','m') ORDER BY 1";

const TRIGGERS: &str = "SELECT t.tgname::text, c.relname::text, t.tgtype::int4, \
        t.tgenabled::text \
    FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND NOT t.tgisinternal ORDER BY 2, 1";

const TYPES: &str = "SELECT t.typname::text, t.typtype::text, \
        ARRAY(SELECT e.enumlabel::text FROM pg_enum e WHERE e.enumtypid = t.oid \
              ORDER BY e.enumsortorder), \
        CASE WHEN t.typtype = 'd' THEN format_type(t.typbasetype, t.typtypmod) END \
    FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
    LEFT JOIN pg_class c ON c.oid = t.typrelid \
    WHERE n.nspname = $1 AND t.typtype IN ('e','c','d','r') \
        AND (t.typtype <> 'c' OR c.relkind = 'c') AND NOT EXISTS (SELECT 1 FROM pg_depend d \
        WHERE d.classid = 'pg_type'::regclass AND d.objid = t.oid AND d.deptype = 'e') \
    ORDER BY 1";

const FUNCTION_DEF: &str = "SELECT pg_get_functiondef(p.oid) FROM pg_proc p \
    JOIN pg_namespace n ON n.oid = p.pronamespace \
    WHERE n.nspname = $1 AND p.proname = $2 \
        AND ($3::text IS NULL OR pg_get_function_identity_arguments(p.oid) = $3) \
    ORDER BY p.oid LIMIT 1";

const SEQUENCE: &str = "SELECT data_type::text, start_value, min_value, max_value, \
        increment_by, cycle, cache_size, last_value \
    FROM pg_sequences WHERE schemaname = $1 AND sequencename = $2";

const MATVIEW_DEF: &str = "SELECT pg_get_viewdef(c.oid, true), c.relispopulated \
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind = 'm'";

const MATVIEW_INDEXES: &str = "SELECT pg_get_indexdef(i.indexrelid) FROM pg_index i \
    JOIN pg_class c ON c.oid = i.indrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND c.relname = $2 ORDER BY 1";

const TRIGGER_DEF: &str = "SELECT pg_get_triggerdef(t.oid, true), c.relname::text \
    FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname = $1 AND t.tgname = $2 AND ($3::text IS NULL OR c.relname = $3) \
    ORDER BY 2 LIMIT 1";

const TYPE_DEF: &str = "SELECT t.typtype::text, format_type(t.typbasetype, t.typtypmod), \
        t.typnotnull, t.typdefault, \
        ARRAY(SELECT e.enumlabel::text FROM pg_enum e WHERE e.enumtypid = t.oid \
              ORDER BY e.enumsortorder), \
        (SELECT format_type(r.rngsubtype, NULL) FROM pg_range r WHERE r.rngtypid = t.oid), \
        ARRAY(SELECT quote_ident(a.attname) || ' ' || format_type(a.atttypid, a.atttypmod) \
              FROM pg_attribute a WHERE a.attrelid = t.typrelid AND a.attnum > 0 \
                  AND NOT a.attisdropped ORDER BY a.attnum), \
        ARRAY(SELECT 'CONSTRAINT ' || quote_ident(con.conname) || ' ' \
                  || pg_get_constraintdef(con.oid) \
              FROM pg_constraint con WHERE con.contypid = t.oid ORDER BY con.conname) \
    FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
    WHERE n.nspname = $1 AND t.typname = $2";

pub(super) async fn list(p: &PgPool, schema: &str) -> AppResult<Vec<SchemaObject>> {
    let (functions, relations, triggers, types) = futures::try_join!(
        sqlx::query_as::<_, (String, String, String, Option<String>)>(FUNCTIONS)
            .bind(schema)
            .fetch_all(p),
        sqlx::query_as::<_, (String, String, bool)>(RELATIONS)
            .bind(schema)
            .fetch_all(p),
        sqlx::query_as::<_, (String, String, i32, String)>(TRIGGERS)
            .bind(schema)
            .fetch_all(p),
        sqlx::query_as::<_, (String, String, Vec<String>, Option<String>)>(TYPES)
            .bind(schema)
            .fetch_all(p),
    )?;
    let mut out: Vec<SchemaObject> = functions.into_iter().map(function_object).collect();
    out.extend(relations.into_iter().map(relation_object));
    out.extend(triggers.into_iter().map(trigger_object));
    out.extend(types.into_iter().map(type_object));
    Ok(out)
}

pub(super) fn function_object(
    (name, kind, args, result): (String, String, String, Option<String>),
) -> SchemaObject {
    let kind = if kind == "p" {
        ObjectKind::Procedure
    } else {
        ObjectKind::Function
    };
    let mut o = SchemaObject::new(kind, name);
    o.arguments = Some(args);
    o.detail = result.map(|r| format!("returns {r}"));
    o
}

pub(super) fn relation_object((name, kind, populated): (String, String, bool)) -> SchemaObject {
    if kind == "S" {
        return SchemaObject::new(ObjectKind::Sequence, name);
    }
    let mut o = SchemaObject::new(ObjectKind::MaterializedView, name);
    if !populated {
        o.detail = Some("not populated".into());
    }
    o
}

pub(super) fn trigger_object(
    (name, table, tgtype, enabled): (String, String, i32, String),
) -> SchemaObject {
    let mut o = SchemaObject::new(ObjectKind::Trigger, name);
    let disabled = if enabled == "D" { " (disabled)" } else { "" };
    o.detail = Some(format!("{}{disabled}", trigger_detail(tgtype)));
    o.table = Some(table);
    o
}

pub(super) fn type_object(
    (name, typtype, labels, base): (String, String, Vec<String>, Option<String>),
) -> SchemaObject {
    let mut o = SchemaObject::new(ObjectKind::Type, name);
    o.detail = Some(type_detail(&typtype, base.as_deref()));
    o.labels = labels;
    o
}

pub(super) async fn definition(
    p: &PgPool,
    schema: &str,
    object: &ObjectRef,
) -> AppResult<ObjectDefinition> {
    let name = &object.name;
    let not_found = || missing(object.kind, schema, name);
    match object.kind {
        ObjectKind::Function | ObjectKind::Procedure => {
            let def: Option<String> = sqlx::query_scalar(FUNCTION_DEF)
                .bind(schema)
                .bind(name)
                .bind(object.arguments.as_deref())
                .fetch_optional(p)
                .await?;
            let ddl = terminated(&def.ok_or_else(not_found)?);
            let note = "-- CREATE OR REPLACE can't change the arguments or the result type:\n\
                        -- drop the routine first for that.\n";
            Ok(ObjectDefinition {
                edit_script: format!("{note}{ddl}"),
                ddl,
            })
        }
        ObjectKind::Sequence => {
            let info = sequence(p, schema, name).await?;
            let target = qualified(D, schema, name);
            Ok(ObjectDefinition {
                ddl: sequence_ddl("CREATE", &target, &info),
                edit_script: sequence_ddl("ALTER", &target, &info),
            })
        }
        ObjectKind::MaterializedView => matview_definition(p, schema, name).await,
        ObjectKind::Trigger => trigger_definition(p, schema, object).await,
        ObjectKind::Type => {
            let row: Option<TypeRow> = sqlx::query_as(TYPE_DEF)
                .bind(schema)
                .bind(name)
                .fetch_optional(p)
                .await?;
            let row = row.ok_or_else(not_found)?;
            Ok(type_ddl(&qualified(D, schema, name), &row))
        }
        ObjectKind::Event => Err(super::unsupported(ObjectKind::Event)),
    }
}

pub(super) async fn sequence(p: &PgPool, schema: &str, name: &str) -> AppResult<SequenceInfo> {
    let row: Option<(String, i64, i64, i64, i64, bool, i64, Option<i64>)> =
        sqlx::query_as(SEQUENCE)
            .bind(schema)
            .bind(name)
            .fetch_optional(p)
            .await?;
    let (data_type, start_value, min_value, max_value, increment, cycle, cache, last_value) =
        row.ok_or_else(|| missing(ObjectKind::Sequence, schema, name))?;
    Ok(SequenceInfo {
        data_type,
        start_value,
        min_value,
        max_value,
        increment,
        cycle,
        cache,
        last_value,
    })
}

async fn matview_definition(p: &PgPool, schema: &str, name: &str) -> AppResult<ObjectDefinition> {
    let row: Option<(String, bool)> = sqlx::query_as(MATVIEW_DEF)
        .bind(schema)
        .bind(name)
        .fetch_optional(p)
        .await?;
    let (def, populated) =
        row.ok_or_else(|| missing(ObjectKind::MaterializedView, schema, name))?;
    let indexes: Vec<String> = sqlx::query_scalar(MATVIEW_INDEXES)
        .bind(schema)
        .bind(name)
        .fetch_all(p)
        .await?;
    let target = qualified(D, schema, name);
    let body = def.trim_end().trim_end_matches(';');
    let data = if populated { "" } else { "\nWITH NO DATA" };
    let mut ddl = format!("CREATE MATERIALIZED VIEW {target} AS\n{body}{data};");
    for i in indexes {
        ddl.push_str(&format!("\n{i};"));
    }
    Ok(ObjectDefinition {
        edit_script: format!("DROP MATERIALIZED VIEW {target};\n{ddl}"),
        ddl,
    })
}

async fn trigger_definition(
    p: &PgPool,
    schema: &str,
    object: &ObjectRef,
) -> AppResult<ObjectDefinition> {
    let row: Option<(String, String)> = sqlx::query_as(TRIGGER_DEF)
        .bind(schema)
        .bind(&object.name)
        .bind(object.table.as_deref())
        .fetch_optional(p)
        .await?;
    let (def, table) = row.ok_or_else(|| missing(ObjectKind::Trigger, schema, &object.name))?;
    let ddl = terminated(&def);
    let drop = format!(
        "DROP TRIGGER {} ON {};",
        quote_ident(D, &object.name),
        qualified(D, schema, &table)
    );
    Ok(ObjectDefinition {
        edit_script: format!("{drop}\n{ddl}"),
        ddl,
    })
}
