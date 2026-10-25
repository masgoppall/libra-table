//! End-to-end checks against real databases.
//!
//! * SQLite always runs (temporary file).
//! * Postgres / MySQL run when an admin URL is given; the test creates the databases
//!   `libratable_test` and `libratable_test_other`, uses them, and drops them:
//!   LIBRATABLE_TEST_PG_URL=postgres://postgres@127.0.0.1:5432/postgres
//!   LIBRATABLE_TEST_MYSQL_URL=mysql://root@127.0.0.1:3306/mysql

mod common;
mod data;
mod multi_db;
mod schema;

use libratable_lib::models::Driver;
use serde_json::json;

use common::{connection, session_for, OTHER_DB, TEST_DB};

const SQLITE_SLEEP: &str =
    "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c";

#[tokio::test]
async fn sqlite_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    std::fs::File::create(&path).unwrap();
    let conn = connection(
        Driver::Sqlite,
        json!({ "filePath": path.display().to_string() }),
    );
    let session = session_for(conn, "main", None).await;
    data::run(session, "main", SQLITE_SLEEP).await;
}

#[tokio::test]
async fn postgres_end_to_end() {
    let server = Server {
        driver: Driver::Postgres,
        admin_env: "LIBRATABLE_TEST_PG_URL",
        sleep_sql: "SELECT pg_sleep(10)",
        force: " WITH (FORCE)",
    };
    server.run(|_| "public".to_string()).await;
}

#[tokio::test]
async fn mysql_end_to_end() {
    let server = Server {
        driver: Driver::Mysql,
        admin_env: "LIBRATABLE_TEST_MYSQL_URL",
        sleep_sql: "SELECT SLEEP(10)",
        force: "",
    };
    server.run(|db| db.to_string()).await;
}

/// A Postgres or MySQL server reached through an admin URL.
struct Server {
    driver: Driver,
    admin_env: &'static str,
    /// A query that runs long enough to be cancelled.
    sleep_sql: &'static str,
    /// Added to DROP DATABASE to disconnect other sessions first.
    force: &'static str,
}

impl Server {
    /// Creates `libratable_test`, runs the scenarios in it, and drops both test
    /// databases. `schema_of` names the schema of a database's tables.
    async fn run(&self, schema_of: fn(&str) -> String) {
        let Ok(admin) = std::env::var(self.admin_env) else {
            eprintln!("skipped: set {} to run", self.admin_env);
            return;
        };
        let (host, port, user, password) = url_parts(&admin);
        self.admin_exec(&admin, &format!("DROP DATABASE IF EXISTS {TEST_DB}"))
            .await;
        self.admin_exec(&admin, &format!("CREATE DATABASE {TEST_DB}"))
            .await;

        let conn = connection(
            self.driver,
            json!({
                "host": host, "port": port, "user": user, "database": TEST_DB,
                "passwordMode": "none"
            }),
        );
        let session = session_for(conn.clone(), TEST_DB, password.clone()).await;
        data::run(session, &schema_of(TEST_DB), self.sleep_sql).await;

        let drop_other = format!("DROP DATABASE IF EXISTS {OTHER_DB}{}", self.force);
        self.admin_exec(&admin, &drop_other).await;
        let primary = session_for(conn, TEST_DB, password).await;
        multi_db::run(primary, schema_of).await;

        let drop_test = format!("DROP DATABASE IF EXISTS {TEST_DB}{}", self.force);
        self.admin_exec(&admin, &drop_test).await;
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

/// (host, port, user, password) of a database URL.
fn url_parts(url: &str) -> (String, u16, String, Option<String>) {
    let u = url::Url::parse(url).unwrap();
    (
        u.host_str().unwrap().to_string(),
        u.port().unwrap(),
        u.username().to_string(),
        u.password().map(str::to_string),
    )
}
