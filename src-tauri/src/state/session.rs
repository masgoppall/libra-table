//! One open connection: its pool, SSH tunnel, editor connection, metadata cache and
//! the sessions opened next to it for other databases of the same server.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::pool::PoolConnection;
use sqlx::{MySql, Postgres, Sqlite};

use crate::db::meta::{self, ColumnInfo};
use crate::db::objects::{self, SchemaObject};
use crate::db::{open_pool, DbPool, Target};
use crate::error::AppResult;
use crate::models::{Connection, Driver};
use crate::ssh::Tunnel;

/// The dedicated connection the query editor runs on, so session state
/// (`SET search_path`, transactions, temp tables) survives between runs.
pub enum EditorConn {
    Postgres {
        conn: PoolConnection<Postgres>,
        pid: i32,
    },
    Mysql {
        conn: PoolConnection<MySql>,
        id: u64,
    },
    Sqlite {
        conn: PoolConnection<Sqlite>,
        handle: usize,
    },
}

/// (schema, table) → columns.
type ColumnCache = HashMap<(String, String), Arc<Vec<ColumnInfo>>>;

/// schema → functions, sequences, triggers… (db::objects).
type ObjectCache = HashMap<String, Arc<Vec<SchemaObject>>>;

/// Pool size of a session opened for another database.
const CHILD_POOL_SIZE: u32 = 4;

pub struct Session {
    pub connection: Connection,
    pub pool: DbPool,
    pub tunnel: Option<Arc<Tunnel>>,
    /// Database the pool is connected to (Postgres/MySQL default db; "main" for SQLite).
    pub database: String,
    pub server_version: String,
    /// Kept in memory only, to reconnect when switching databases ("Ask every time").
    pub db_password: Option<String>,
    pub editor: tokio::sync::Mutex<Option<EditorConn>>,
    columns: Mutex<ColumnCache>,
    objects: Mutex<ObjectCache>,
    /// Other databases of the same server, opened on demand so tabs from several
    /// databases can stay open at once. They reuse the settings and the SSH tunnel.
    children: tokio::sync::Mutex<HashMap<String, Arc<Session>>>,
}

impl Session {
    pub fn new(
        connection: Connection,
        pool: DbPool,
        tunnel: Option<Arc<Tunnel>>,
        database: String,
        server_version: String,
        db_password: Option<String>,
    ) -> Self {
        Self {
            connection,
            pool,
            tunnel,
            database,
            server_version,
            db_password,
            editor: tokio::sync::Mutex::new(None),
            columns: Mutex::new(HashMap::new()),
            objects: Mutex::new(HashMap::new()),
            children: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn driver(&self) -> Driver {
        self.connection.driver
    }

    /// This session when `database` is empty or its own; otherwise a session for that
    /// database, opened on first use. SQLite has one file per connection: its attached
    /// databases are schemas, so it always answers with itself.
    pub async fn in_database(self: &Arc<Self>, database: Option<&str>) -> AppResult<Arc<Session>> {
        let Some(db) = database.filter(|d| !d.is_empty() && *d != self.database) else {
            return Ok(self.clone());
        };
        if self.driver() == Driver::Sqlite {
            return Ok(self.clone());
        }
        let mut children = self.children.lock().await;
        if let Some(s) = children.get(db) {
            return Ok(s.clone());
        }
        let child = Arc::new(self.open_child(db).await?);
        children.insert(db.to_string(), child.clone());
        Ok(child)
    }

    async fn open_child(&self, db: &str) -> AppResult<Session> {
        let target = self.tunnel.as_ref().map(|t| Target {
            host: "127.0.0.1".into(),
            port: t.local_port,
            through_tunnel: true,
        });
        let pool = open_pool(
            &self.connection,
            target.as_ref(),
            self.db_password.as_deref(),
            Some(db),
            CHILD_POOL_SIZE,
        )
        .await?;
        Ok(Session::new(
            self.connection.clone(),
            pool,
            self.tunnel.clone(),
            db.to_string(),
            self.server_version.clone(),
            self.db_password.clone(),
        ))
    }

    /// Closes the session opened for `database`, if any (before dropping it, say).
    pub async fn close_database(&self, database: &str) {
        let child = self.children.lock().await.remove(database);
        if let Some(c) = child {
            c.close_pool().await;
        }
    }

    /// Clears the column cache here and in every database opened from here.
    pub async fn clear_all_caches(&self) {
        self.clear_cache();
        for c in self.children.lock().await.values() {
            c.clear_cache();
        }
    }

    /// Columns of a table, cached until `refresh` (SCH-6).
    pub async fn columns(&self, schema: &str, table: &str) -> AppResult<Arc<Vec<ColumnInfo>>> {
        let key = (schema.to_string(), table.to_string());
        if let Some(c) = self.columns.lock().unwrap().get(&key) {
            return Ok(c.clone());
        }
        let cols = Arc::new(meta::list_columns(&self.pool, schema, table).await?);
        self.columns.lock().unwrap().insert(key, cols.clone());
        Ok(cols)
    }

    /// The other schema objects of a schema, cached like the columns.
    pub async fn schema_objects(&self, schema: &str) -> AppResult<Arc<Vec<SchemaObject>>> {
        if let Some(o) = self.objects.lock().unwrap().get(schema) {
            return Ok(o.clone());
        }
        let list = Arc::new(objects::list(&self.pool, schema).await?);
        self.objects
            .lock()
            .unwrap()
            .insert(schema.to_string(), list.clone());
        Ok(list)
    }

    pub fn clear_cache(&self) {
        self.columns.lock().unwrap().clear();
        self.objects.lock().unwrap().clear();
    }

    /// Closes the pool without waiting forever: a running editor query or export keeps
    /// its connection until it finishes, and the closed pool drops it afterwards.
    pub async fn close_pool(&self) {
        if let Ok(mut editor) = self.editor.try_lock() {
            *editor = None;
        }
        let _ = tokio::time::timeout(Duration::from_secs(3), self.pool.close()).await;
    }

    pub async fn close(&self) {
        let children: Vec<Arc<Session>> =
            self.children.lock().await.drain().map(|(_, c)| c).collect();
        for c in children {
            c.close_pool().await;
        }
        self.close_pool().await;
        if let Some(t) = &self.tunnel {
            // Only the last session using this tunnel closes it.
            if Arc::strong_count(t) <= 1 {
                t.close().await;
            }
        }
    }
}
