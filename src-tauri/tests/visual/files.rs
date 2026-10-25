//! Helpers: tables to import into, files to import, queries to check the results.

use std::path::{Path, PathBuf};

use libratable_lib::db::DbPool;
use libratable_lib::import::{
    ColumnMap, CsvOptions, FileFormat, ImportRequest, OnError, SourceSpec, TxScope,
};
use libratable_lib::models::Dialect;

const TABLES_PG: &str = "CREATE TABLE authors (id integer PRIMARY KEY, name text NOT NULL, \
        born date);
    CREATE TABLE books (id integer PRIMARY KEY, author_id integer REFERENCES authors(id), \
        title text NOT NULL, price numeric(8,2), in_stock boolean, tags jsonb);";

const TABLES_MYSQL: &str = "CREATE TABLE authors (id int PRIMARY KEY, \
        name varchar(100) NOT NULL, born date);
    CREATE TABLE books (id int PRIMARY KEY, author_id int, title varchar(200) NOT NULL, \
        price decimal(8,2), in_stock tinyint(1), tags json, \
        FOREIGN KEY (author_id) REFERENCES authors(id));";

const TABLES_SQLITE: &str = "CREATE TABLE authors (id INTEGER PRIMARY KEY, \
        name TEXT NOT NULL, born DATE);
    CREATE TABLE books (id INTEGER PRIMARY KEY, author_id INTEGER REFERENCES authors(id), \
        title TEXT NOT NULL, price NUMERIC, in_stock BOOLEAN, tags TEXT);";

pub fn tables_sql(driver: Dialect) -> &'static str {
    match driver {
        Dialect::Postgres => TABLES_PG,
        Dialect::Mysql => TABLES_MYSQL,
        Dialect::Sqlite => TABLES_SQLITE,
    }
}

pub async fn exec(pool: &DbPool, sql: &str) {
    match pool {
        DbPool::Postgres(p) => drop(sqlx::raw_sql(sql).execute(p).await.unwrap()),
        DbPool::Mysql(p) => drop(sqlx::raw_sql(sql).execute(p).await.unwrap()),
        DbPool::Sqlite(p) => drop(sqlx::raw_sql(sql).execute(p).await.unwrap()),
        DbPool::Engine(_) => unreachable!("the tests use built-in drivers"),
    }
}

/// A single integer (`count(*)` and the like).
pub async fn int(pool: &DbPool, sql: &str) -> i64 {
    match pool {
        DbPool::Postgres(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Mysql(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Sqlite(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Engine(_) => unreachable!("the tests use built-in drivers"),
    }
}

/// A single text value (NULL is None).
pub async fn text(pool: &DbPool, sql: &str) -> Option<String> {
    match pool {
        DbPool::Postgres(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Mysql(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Sqlite(p) => sqlx::query_scalar(sql).fetch_one(p).await.unwrap(),
        DbPool::Engine(_) => unreachable!("the tests use built-in drivers"),
    }
}

pub fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    path
}

/// An import of `path` into `table`, source column i → `targets[i]`.
pub fn request(
    path: &Path,
    format: FileFormat,
    schema: &str,
    table: &str,
    targets: &[&str],
) -> ImportRequest {
    ImportRequest {
        source: SourceSpec {
            path: path.display().to_string(),
            format,
            csv: CsvOptions::default(),
        },
        source_columns: targets.iter().map(|t| t.to_string()).collect(),
        schema: schema.into(),
        table: table.into(),
        mapping: targets
            .iter()
            .enumerate()
            .map(|(source, t)| ColumnMap {
                source,
                target: t.to_string(),
            })
            .collect(),
        batch_size: 500,
        on_error: OnError::Stop,
        transaction: TxScope::Run,
    }
}
