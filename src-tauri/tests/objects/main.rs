//! Schema objects end to end: create functions, procedures, sequences, materialized
//! views, triggers, types and events, list them, read their definitions (and run the
//! edit scripts), then refresh/reset/drop them through `db::ddl`.
//!
//! * SQLite always runs (temporary file).
//! * Postgres / MySQL run when an admin URL is given. They only create and drop the
//!   database `libratable_obj_test`:
//!   LIBRATABLE_TEST_PG_URL=postgres://postgres@127.0.0.1:5432/postgres
//!   LIBRATABLE_TEST_MYSQL_URL=mysql://root@127.0.0.1:3306/mysql

use std::sync::Arc;

use libratable_lib::db::objects::{self, ObjectKind, ObjectRef, SchemaObject};
use libratable_lib::db::{ddl, open_pool, split, DbPool};
use libratable_lib::error::AppError;
use libratable_lib::models::{Connection, Driver};
use libratable_lib::state::Session;
use serde_json::{json, Value};

const DB: &str = "libratable_obj_test";

fn connection(driver: Driver, settings: Value) -> Connection {
    let mut v = json!({ "id": "obj-test", "name": "Objects", "driver": driver });
    v["db"] = settings;
    serde_json::from_value(v).unwrap()
}

async fn session(conn: Connection, database: &str, password: Option<String>) -> Arc<Session> {
    let pool = open_pool(&conn, None, password.as_deref(), None, 4)
        .await
        .expect("connect");
    let version = pool.server_version().await.unwrap();
    Arc::new(Session::new(
        conn,
        pool,
        None,
        database.into(),
        version,
        password,
    ))
}

/// Runs a script statement by statement on one connection, split the way the query
/// editor splits it. (One connection like the editor's: an SQLite connection whose
/// schema is stale reports "already exists" for an object another one just dropped.)
async fn run_script(s: &Session, script: &str) {
    macro_rules! run_all {
        ($pool:expr) => {{
            let mut c = $pool.acquire().await.unwrap();
            for st in split::split(s.driver(), script) {
                sqlx::raw_sql(&st.text)
                    .execute(&mut *c)
                    .await
                    .unwrap_or_else(|e| panic!("{e}\n--- in ---\n{}", st.text));
            }
        }};
    }
    match &s.pool {
        DbPool::Postgres(p) => run_all!(p),
        DbPool::Mysql(p) => run_all!(p),
        DbPool::Sqlite(p) => run_all!(p),
    }
}

async fn scalar_i64(s: &Session, sql: &str) -> i64 {
    match &s.pool {
        DbPool::Postgres(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Mysql(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Sqlite(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
    }
}

/// Builds, previews and runs a schema change the way `apply_schema_change` does.
async fn change(s: &Session, change: Value) -> Result<Vec<String>, AppError> {
    let change: ddl::SchemaChange = serde_json::from_value(change).expect("change JSON");
    let ctx = ddl::context(&s.pool, &change).await?;
    let plan = ddl::build(s.driver(), &change, &ctx)?;
    let preview = ddl::preview(&s.pool, &plan, &change, &ctx).await?;
    ddl::apply(&s.pool, &plan, &change, &ctx).await?;
    s.clear_cache();
    Ok(preview)
}

fn drop(schema: &str, kind: &str, name: &str, extra: Value) -> Value {
    let mut object = json!({ "kind": kind, "name": name });
    if let (Some(o), Some(e)) = (object.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    json!({ "kind": "drop_object", "schema": schema, "object": object })
}

async fn listed(s: &Session, schema: &str) -> Vec<SchemaObject> {
    s.schema_objects(schema).await.unwrap().as_ref().clone()
}

fn find<'a>(list: &'a [SchemaObject], kind: ObjectKind, name: &str) -> &'a SchemaObject {
    list.iter()
        .find(|o| o.kind == kind && o.name == name)
        .unwrap_or_else(|| panic!("{kind:?} {name} not listed in {list:?}"))
}

fn object(kind: ObjectKind, name: &str) -> ObjectRef {
    ObjectRef {
        kind,
        name: name.into(),
        arguments: None,
        table: None,
    }
}

async fn definition(s: &Session, schema: &str, o: &ObjectRef) -> objects::ObjectDefinition {
    objects::definition(&s.pool, schema, o)
        .await
        .unwrap_or_else(|e| panic!("definition of {}: {e:?}", o.name))
}

#[tokio::test]
async fn sqlite_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("objects.db");
    std::fs::File::create(&path).unwrap();
    let conn = connection(
        Driver::Sqlite,
        json!({ "filePath": path.display().to_string() }),
    );
    let s = session(conn, "main", None).await;
    run_script(
        &s,
        "CREATE TABLE people (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE log (msg TEXT);
         CREATE TRIGGER people_ai AFTER INSERT ON people
         BEGIN INSERT INTO log VALUES (new.name); END;",
    )
    .await;
    let list = listed(&s, "main").await;
    let t = find(&list, ObjectKind::Trigger, "people_ai");
    assert_eq!(t.table.as_deref(), Some("people"));
    assert_eq!(t.detail.as_deref(), Some("AFTER INSERT · row"));

    let def = definition(&s, "main", &object(ObjectKind::Trigger, "people_ai")).await;
    assert!(def.ddl.starts_with("CREATE TRIGGER people_ai"));
    run_script(&s, &def.edit_script).await;
    run_script(&s, "INSERT INTO people (name) VALUES ('Ann')").await;
    assert_eq!(scalar_i64(&s, "SELECT count(*) FROM log").await, 1);

    let preview = change(&s, drop("main", "trigger", "people_ai", json!({})))
        .await
        .unwrap();
    assert_eq!(preview, vec![r#"DROP TRIGGER "main"."people_ai""#]);
    assert!(listed(&s, "main").await.is_empty());
}

mod mysql;
mod postgres;

#[tokio::test]
async fn postgres_objects() {
    let Some(server) = Server::from_env(Driver::Postgres, "LIBRATABLE_TEST_PG_URL") else {
        return;
    };
    let s = server.open().await;
    run_script(&s, postgres::PG_SETUP).await;
    postgres::pg_listing(&s).await;
    postgres::pg_definitions(&s).await;
    postgres::pg_changes(&s).await;
    s.close().await;
    server.drop_db().await;
}

#[tokio::test]
async fn mysql_objects_end_to_end() {
    let Some(server) = Server::from_env(Driver::Mysql, "LIBRATABLE_TEST_MYSQL_URL") else {
        return;
    };
    let s = server.open().await;
    mysql::mysql_objects(&s).await;
    s.close().await;
    server.drop_db().await;
}

/// A Postgres or MySQL server reached through an admin URL.
struct Server {
    driver: Driver,
    admin: String,
}

impl Server {
    fn from_env(driver: Driver, var: &str) -> Option<Self> {
        let Ok(admin) = std::env::var(var) else {
            eprintln!("skipped: set {var} to run");
            return None;
        };
        Some(Self { driver, admin })
    }

    /// Recreates the test database and opens a session in it.
    async fn open(&self) -> Arc<Session> {
        self.drop_db().await;
        self.exec(&format!("CREATE DATABASE {DB}")).await;
        let u = url::Url::parse(&self.admin).unwrap();
        let conn = connection(
            self.driver,
            json!({
                "host": u.host_str().unwrap(), "port": u.port().unwrap(),
                "user": u.username(), "database": DB, "passwordMode": "none"
            }),
        );
        session(conn, DB, u.password().map(str::to_string)).await
    }

    async fn drop_db(&self) {
        let force = if self.driver == Driver::Postgres {
            " WITH (FORCE)"
        } else {
            ""
        };
        self.exec(&format!("DROP DATABASE IF EXISTS {DB}{force}"))
            .await;
    }

    async fn exec(&self, sql: &str) {
        match self.driver {
            Driver::Postgres => {
                let pool = sqlx::PgPool::connect(&self.admin).await.unwrap();
                sqlx::query(sql).execute(&pool).await.unwrap();
                pool.close().await;
            }
            _ => {
                let pool = sqlx::MySqlPool::connect(&self.admin).await.unwrap();
                sqlx::query(sql).execute(&pool).await.unwrap();
                pool.close().await;
            }
        }
    }
}
