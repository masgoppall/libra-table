//! One pooled connection of any driver, for jobs that must run every statement on the
//! same connection (transactions, savepoints, `SET` in SQL files).

use futures::TryStreamExt;
use sqlx::pool::PoolConnection;
use sqlx::{Either, MySql, Postgres, Sqlite};

use crate::db::{bind_all, DbPool};
use crate::error::AppResult;

/// Runs `sql` with `fetch_many` (as the editor does): `raw_sql(..).execute` futures are
/// not `Send` for every lifetime, which spawned jobs need.
macro_rules! raw_affected {
    ($conn:expr, $sql:expr) => {{
        let mut affected = 0u64;
        let mut stream = sqlx::raw_sql($sql).fetch_many($conn);
        while let Some(item) = stream.try_next().await? {
            if let Either::Left(done) = item {
                affected += done.rows_affected();
            }
        }
        affected
    }};
}

pub enum Conn {
    Postgres(PoolConnection<Postgres>),
    Mysql(PoolConnection<MySql>),
    Sqlite(PoolConnection<Sqlite>),
}

impl Conn {
    pub async fn acquire(pool: &DbPool) -> AppResult<Self> {
        Ok(match pool {
            DbPool::Postgres(p) => Conn::Postgres(p.acquire().await?),
            DbPool::Mysql(p) => Conn::Mysql(p.acquire().await?),
            DbPool::Sqlite(p) => Conn::Sqlite(p.acquire().await?),
            DbPool::Engine(_) => return Err(crate::db::engine::sqlx_only()),
        })
    }

    /// A prepared statement with text binds (`None` is NULL). Returns rows affected.
    pub async fn exec(&mut self, sql: &str, binds: &[Option<String>]) -> AppResult<u64> {
        Ok(match self {
            Conn::Postgres(c) => bind_all!(sqlx::query(sql), binds)
                .execute(&mut **c)
                .await?
                .rows_affected(),
            Conn::Mysql(c) => bind_all!(sqlx::query(sql), binds)
                .execute(&mut **c)
                .await?
                .rows_affected(),
            Conn::Sqlite(c) => bind_all!(sqlx::query(sql), binds)
                .execute(&mut **c)
                .await?
                .rows_affected(),
        })
    }

    /// SQL as written (simple protocol): transaction control and statements of a file.
    pub async fn raw(&mut self, sql: &str) -> AppResult<u64> {
        Ok(match self {
            Conn::Postgres(c) => raw_affected!(&mut **c, sql),
            Conn::Mysql(c) => raw_affected!(&mut **c, sql),
            Conn::Sqlite(c) => raw_affected!(&mut **c, sql),
        })
    }

    /// Closes the connection instead of returning it to the pool, when its state can't
    /// be trusted (an interrupted statement, session settings a file changed).
    pub fn discard(&mut self) {
        match self {
            Conn::Postgres(c) => c.close_on_drop(),
            Conn::Mysql(c) => c.close_on_drop(),
            Conn::Sqlite(c) => c.close_on_drop(),
        }
    }
}
