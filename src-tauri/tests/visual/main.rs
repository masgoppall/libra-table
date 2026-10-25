//! End-to-end checks for imports, SQL files, multi-table export, the ER diagram's
//! catalog and metrics widget queries.
//!
//! * SQLite always runs (temporary file).
//! * Postgres / MySQL run when an admin URL is given. They create `libratable_vis_pg` /
//!   `libratable_vis_mysql`, use it, and drop it; nothing else is touched:
//!   LIBRATABLE_TEST_PG_URL=postgres://postgres@127.0.0.1:5432/postgres
//!   LIBRATABLE_TEST_MYSQL_URL=mysql://root@127.0.0.1:3306/mysql

mod files;
mod scenarios;

use std::sync::Arc;

use libratable_lib::db::open_pool;
use libratable_lib::models::{Connection, Driver};
use libratable_lib::state::Session;
use serde_json::{json, Value};

fn connection(driver: Driver, settings: Value) -> Connection {
    let mut v = json!({ "id": "vis-conn", "name": "Visual test", "driver": driver });
    v["db"] = settings;
    serde_json::from_value(v).unwrap()
}

async fn session_for(conn: Connection, database: &str, password: Option<String>) -> Arc<Session> {
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

#[tokio::test]
async fn sqlite_import_and_visualization() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vis.db");
    std::fs::File::create(&path).unwrap();
    let conn = connection(
        Driver::Sqlite,
        json!({ "filePath": path.display().to_string() }),
    );
    let s = session_for(conn, "main", None).await;
    scenarios::run(s, "main".into()).await;
}

#[tokio::test]
async fn postgres_import_and_visualization() {
    Server {
        driver: Driver::Postgres,
        admin_env: "LIBRATABLE_TEST_PG_URL",
        database: "libratable_vis_pg",
        force: " WITH (FORCE)",
    }
    .run("public")
    .await;
}

#[tokio::test]
async fn mysql_import_and_visualization() {
    let db = "libratable_vis_mysql";
    Server {
        driver: Driver::Mysql,
        admin_env: "LIBRATABLE_TEST_MYSQL_URL",
        database: db,
        force: "",
    }
    .run(db)
    .await;
}

struct Server {
    driver: Driver,
    admin_env: &'static str,
    database: &'static str,
    force: &'static str,
}

impl Server {
    async fn run(&self, schema: &str) {
        let Ok(admin) = std::env::var(self.admin_env) else {
            eprintln!("skipped: set {} to run", self.admin_env);
            return;
        };
        let u = url::Url::parse(&admin).unwrap();
        let password = u.password().map(str::to_string);
        let drop = format!("DROP DATABASE IF EXISTS {}{}", self.database, self.force);
        self.admin_exec(&admin, &drop).await;
        self.admin_exec(&admin, &format!("CREATE DATABASE {}", self.database))
            .await;
        let conn = connection(
            self.driver,
            json!({
                "host": u.host_str().unwrap(), "port": u.port().unwrap(),
                "user": u.username(), "database": self.database, "passwordMode": "none"
            }),
        );
        let s = session_for(conn, self.database, password).await;
        let pool = s.pool.clone();
        // Drop the test database even when a check fails.
        let outcome = tokio::spawn(scenarios::run(s, schema.to_string())).await;
        pool.close().await;
        self.admin_exec(&admin, &drop).await;
        if let Err(e) = outcome {
            std::panic::resume_unwind(e.into_panic());
        }
    }

    async fn admin_exec(&self, admin: &str, sql: &str) {
        match self.driver {
            Driver::Postgres => {
                let pool = sqlx::PgPool::connect(admin).await.unwrap();
                sqlx::query(sql).execute(&pool).await.unwrap();
                pool.close().await;
            }
            Driver::Mysql => {
                let pool = sqlx::MySqlPool::connect(admin).await.unwrap();
                sqlx::query(sql).execute(&pool).await.unwrap();
                pool.close().await;
            }
            Driver::Sqlite => unreachable!("SQLite has no server"),
        }
    }
}
