//! The Redis engine: a shared, self-reconnecting connection for browsing, one for
//! edits (WATCH/MULTI need a connection nobody else writes to) and one for the editor.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use redis::aio::{ConnectionManager, MultiplexedConnection};
use redis::{Client, Value as Reply};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::Mutex;

use crate::db::cell::CellContent;
use crate::db::changes::Change;
use crate::db::data::{FilterPreview, PageRequest, RowCount};
use crate::db::engine::{Engine, OpenArgs, PageRows, TableRef};
use crate::db::exec::StatementResult;
use crate::db::meta::{ColumnInfo, ObjectInfo};
use crate::error::{AppError, AppResult};
use crate::models::Driver;

use super::conn::{self, command_error, db_name};
use super::edits;
use super::filters::{self, KeyQuery};
use super::key_edits::{self, KeyEdit};
use super::keys::{self, KeyRow, Pages};
use super::plan::{self, Plan};
use super::read::{self, KeyDetail, ReadRequest};
use super::run::Editor;
use super::values::{self, int, scalar_text};

/// The one "table" of every database.
pub const KEYS_TABLE: &str = "keys";
/// Databases Redis has unless `databases` says otherwise.
const DEFAULT_DATABASES: i64 = 16;
/// Filtered counts scan the whole database up to this many keys; above it the count
/// is DBSIZE, marked as not exact.
const COUNT_SCAN_LIMIT: i64 = 100_000;

pub struct RedisEngine {
    client: Client,
    through_tunnel: bool,
    db: i64,
    databases: i64,
    startup: String,
    browser: ConnectionManager,
    edits: Mutex<Option<MultiplexedConnection>>,
    editor: Editor,
    pages: Pages,
    closed: AtomicBool,
}

/// A database in the sidebar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub name: String,
    pub index: i64,
    pub keys: i64,
    pub expires: i64,
}

/// A page of the key list.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequest {
    /// SCAN MATCH glob; empty = every key.
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub key_type: String,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPage {
    pub keys: Vec<KeyRow>,
    pub has_more: bool,
    /// DBSIZE: every key of the database, filtered or not.
    pub total: i64,
    pub query: String,
}

fn column(name: &str, data_type: &str, pk: bool) -> ColumnInfo {
    ColumnInfo {
        name: name.into(),
        data_type: data_type.into(),
        nullable: !pk && name != "type" && name != "ttl",
        default: None,
        pk_position: pk.then_some(1),
        auto_increment: false,
        comment: None,
    }
}

pub fn key_columns() -> Vec<ColumnInfo> {
    vec![
        column("key", "text", true),
        column("type", "text", false),
        column("ttl", "integer", false),
        column("size", "integer", false),
        column("value", "text", false),
    ]
}

/// `db0:keys=1,expires=0,avg_ttl=0` lines of INFO keyspace: index → (keys, expires).
fn keyspace(info: &str) -> BTreeMap<i64, (i64, i64)> {
    let mut out = BTreeMap::new();
    for line in info.lines() {
        let Some((db, rest)) = line.trim().split_once(':') else {
            continue;
        };
        let Some(index) = db.strip_prefix("db").and_then(|i| i.parse().ok()) else {
            continue;
        };
        let field = |name: &str| {
            rest.split(',')
                .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
        };
        out.insert(index, (field("keys"), field("expires")));
    }
    out
}

fn version(info: &str) -> String {
    let field = |name: &str| {
        info.lines()
            .find_map(|l| l.trim().strip_prefix(name)?.strip_prefix(':'))
            .map(str::to_string)
    };
    match field("valkey_version") {
        Some(v) => format!("Valkey {v}"),
        None => field("redis_version").unwrap_or_default(),
    }
}

impl RedisEngine {
    pub async fn open(a: &OpenArgs<'_>) -> AppResult<Self> {
        let db = conn::parse_db(&a.database())?;
        let tunnel = a.through_tunnel();
        let client = conn::client(a, db)?;
        // A plain connection first: login and SELECT errors come back right away.
        let mut first = conn::dedicated(&client, tunnel).await?;
        let config: Reply = redis::cmd("CONFIG")
            .arg("GET")
            .arg("databases")
            .query_async(&mut first)
            .await
            .unwrap_or(Reply::Nil);
        let databases = values::items(config)
            .get(1)
            .and_then(int)
            .unwrap_or(DEFAULT_DATABASES)
            .max(db + 1);
        let browser = conn::browser(&client, tunnel).await?;
        Ok(Self {
            client,
            through_tunnel: tunnel,
            db,
            databases,
            startup: a.conn.options.startup_query.clone(),
            browser,
            edits: Mutex::new(Some(first)),
            editor: Editor::default(),
            pages: Pages::default(),
            closed: AtomicBool::new(false),
        })
    }

    fn conn(&self) -> ConnectionManager {
        self.browser.clone()
    }

    async fn query(&self, cmd: &redis::Cmd) -> AppResult<Reply> {
        cmd.query_async(&mut self.conn())
            .await
            .map_err(command_error)
    }

    async fn info(&self, section: &str) -> AppResult<String> {
        let r = self.query(redis::cmd("INFO").arg(section)).await?;
        Ok(scalar_text(&r).unwrap_or_default())
    }

    async fn dbsize(&self) -> AppResult<i64> {
        Ok(int(&self.query(&redis::cmd("DBSIZE")).await?).unwrap_or(0))
    }

    /// Every database with its key counts.
    pub async fn database_list(&self) -> AppResult<Vec<DatabaseInfo>> {
        let counts = keyspace(&self.info("keyspace").await?);
        let last = counts.keys().max().copied().unwrap_or(0);
        Ok((0..self.databases.max(last + 1))
            .map(|i| {
                let (keys, expires) = counts.get(&i).copied().unwrap_or((0, 0));
                DatabaseInfo {
                    name: db_name(i),
                    index: i,
                    keys,
                    expires,
                }
            })
            .collect())
    }

    /// A page of keys for the key list.
    pub async fn scan(&self, req: &ScanRequest) -> AppResult<ScanPage> {
        let q = KeyQuery {
            pattern: Some(req.pattern.trim().to_string()).filter(|p| !p.is_empty()),
            key_type: Some(req.key_type.trim().to_ascii_lowercase()).filter(|t| !t.is_empty()),
        };
        let size = req.page_size.clamp(1, crate::db::data::MAX_PAGE_SIZE);
        let page = keys::page(&mut self.conn(), &self.pages, &q, req.page, size).await?;
        Ok(ScanPage {
            keys: page.rows,
            has_more: page.has_more,
            total: self.dbsize().await?,
            query: q.describe(keys::count_hint(&q, size)),
        })
    }

    pub async fn read_key(&self, req: &ReadRequest) -> AppResult<KeyDetail> {
        read::read_key(&mut self.conn(), req).await
    }

    /// The plan of a key-view edit (commands shown for confirmation and history).
    pub fn edit_plan(edit: &KeyEdit) -> AppResult<Plan> {
        key_edits::key_plan(edit)
    }

    /// Runs a plan atomically on the edits connection.
    pub async fn run_plan(&self, plan: &Plan) -> AppResult<Vec<Reply>> {
        let mut slot = self.edits.lock().await;
        if slot.is_none() {
            *slot = Some(conn::dedicated(&self.client, self.through_tunnel).await?);
        }
        let c = slot.as_mut().expect("edits connection");
        let result = plan::execute(c, plan).await;
        if result.is_err() {
            // A failed WATCH/MULTI leaves nothing behind, but a dropped link must go.
            let alive: Result<Reply, _> = redis::cmd("PING").query_async(c).await;
            if alive.is_err() {
                *slot = None;
            }
        }
        result
    }

    fn check_table(t: TableRef<'_>) -> AppResult<()> {
        if t.table == KEYS_TABLE {
            Ok(())
        } else {
            Err(AppError::not_found(format!(
                "Redis databases have only the \"{KEYS_TABLE}\" table."
            )))
        }
    }

    /// Counts keys matching `q` by scanning, when the database is small enough.
    async fn count_matching(&self, q: &KeyQuery) -> AppResult<RowCount> {
        let total = self.dbsize().await?;
        if total > COUNT_SCAN_LIMIT {
            return Ok(RowCount {
                total,
                exact: false,
            });
        }
        let mut conn = self.conn();
        let (mut cursor, mut seen) = (0u64, BTreeSet::new());
        loop {
            let (next, batch) = keys::scan_once(&mut conn, q, cursor, 1000).await?;
            seen.extend(batch);
            if next == 0 {
                break;
            }
            cursor = next;
        }
        Ok(RowCount {
            total: seen.len() as i64,
            exact: true,
        })
    }
}

#[async_trait]
impl Engine for RedisEngine {
    fn driver(&self) -> Driver {
        Driver::Redis
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    async fn server_version(&self) -> AppResult<String> {
        Ok(version(&self.info("server").await?))
    }

    async fn current_database(&self) -> AppResult<String> {
        Ok(db_name(self.db))
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.editor.close().await;
        *self.edits.lock().await = None;
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    async fn databases(&self, show_system: bool) -> AppResult<Vec<String>> {
        Ok(self
            .database_list()
            .await?
            .into_iter()
            .filter(|d| show_system || d.keys > 0 || d.index == self.db)
            .map(|d| d.name)
            .collect())
    }

    async fn schemas(&self, _show_system: bool) -> AppResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn objects(&self, _schema: &str) -> AppResult<Vec<ObjectInfo>> {
        Ok(vec![ObjectInfo {
            name: KEYS_TABLE.into(),
            kind: "table",
        }])
    }

    async fn columns(&self, _schema: &str, table: &str) -> AppResult<Vec<ColumnInfo>> {
        let t = TableRef {
            schema: "",
            table,
            columns: &[],
        };
        Self::check_table(t)?;
        Ok(key_columns())
    }

    async fn fetch_page(
        &self,
        t: TableRef<'_>,
        req: &PageRequest,
        page_size: u32,
    ) -> AppResult<PageRows> {
        Self::check_table(t)?;
        let q = filters::from_filters(&req.filters, req.apply_only)?;
        let mut page = keys::page(&mut self.conn(), &self.pages, &q, req.page, page_size).await?;
        keys::sort(&mut page.rows, &req.sort);
        Ok(PageRows {
            rows: page.rows.iter().map(KeyRow::cells).collect(),
            query: q.describe(keys::count_hint(&q, page_size)),
        })
    }

    async fn count_rows(&self, t: TableRef<'_>, req: &PageRequest) -> AppResult<RowCount> {
        Self::check_table(t)?;
        let q = filters::from_filters(&req.filters, req.apply_only)?;
        if q.is_empty() {
            return Ok(RowCount {
                total: self.dbsize().await?,
                exact: true,
            });
        }
        self.count_matching(&q).await
    }

    fn preview_filter(&self, _t: TableRef<'_>, req: &PageRequest) -> AppResult<FilterPreview> {
        let all = filters::from_filters(&req.filters, None)?.clause();
        let row = req
            .apply_only
            .map(|i| filters::from_filters(&req.filters, Some(i)).map(|q| q.clause()))
            .transpose()?;
        Ok(FilterPreview { row, all })
    }

    fn preview_changes(&self, t: TableRef<'_>, changes: &[Change]) -> AppResult<Vec<String>> {
        Self::check_table(t)?;
        Ok(edits::grid_plan(changes)?.previews())
    }

    async fn apply_changes(&self, t: TableRef<'_>, changes: &[Change]) -> AppResult<u64> {
        Self::check_table(t)?;
        self.run_plan(&edits::grid_plan(changes)?).await?;
        Ok(changes.len() as u64)
    }

    async fn fetch_cell(
        &self,
        t: TableRef<'_>,
        pk: &Map<String, Value>,
        column: &str,
    ) -> AppResult<CellContent> {
        Self::check_table(t)?;
        let key = pk
            .get("key")
            .ok_or_else(|| AppError::validation("The row has no key."))
            .and_then(values::key_bytes)?;
        if column == "value" {
            return read::cell(&mut self.conn(), &key).await;
        }
        let rows = keys::details(&mut self.conn(), vec![key]).await?;
        let row = rows
            .first()
            .ok_or_else(|| AppError::not_found("This key no longer exists."))?;
        let cell = match column {
            "key" => row.key.clone(),
            "type" => Value::String(row.key_type.clone()),
            "ttl" => Value::from(row.ttl),
            "size" => row.size.map(Value::from).unwrap_or(Value::Null),
            other => return Err(AppError::validation(format!("Unknown column \"{other}\"."))),
        };
        Ok(match cell {
            Value::Null => CellContent::Null,
            Value::String(text) => CellContent::Text { text },
            other => CellContent::Text {
                text: other.to_string(),
            },
        })
    }

    async fn run(
        &self,
        statements: &[String],
        limit: usize,
        cancelled: &AtomicBool,
    ) -> Vec<StatementResult> {
        let target = (&self.client, self.through_tunnel, self.startup.as_str());
        self.editor.run(target, statements, limit, cancelled).await
    }

    /// Unblocks a blocking command on the editor connection; others can't be stopped.
    async fn cancel(&self) -> AppResult<()> {
        if let Some(id) = self.editor.client_id() {
            let cmd = redis::cmd("CLIENT")
                .arg("UNBLOCK")
                .arg(id)
                .arg("ERROR")
                .clone();
            self.query(&cmd).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_keyspace_and_version() {
        let ks =
            keyspace("# Keyspace\r\ndb0:keys=12,expires=3,avg_ttl=0\r\ndb5:keys=1,expires=0\r\n");
        assert_eq!(ks.get(&0), Some(&(12, 3)));
        assert_eq!(ks.get(&5), Some(&(1, 0)));
        assert_eq!(version("# Server\r\nredis_version:7.2.4\r\n"), "7.2.4");
        assert_eq!(
            version("redis_version:7.2.4\r\nvalkey_version:8.0.1\r\n"),
            "Valkey 8.0.1"
        );
    }

    #[test]
    fn key_columns_have_the_key_as_primary_key() {
        let cols = key_columns();
        assert_eq!(cols[0].pk_position, Some(1));
        let names: Vec<_> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["key", "type", "ttl", "size", "value"]);
    }
}
