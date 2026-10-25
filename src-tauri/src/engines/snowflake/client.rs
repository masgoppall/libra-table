//! One Snowflake session over the REST API the official connectors use: password login
//! (`/session/v1/login-request`), `query-request` with JSON result sets (extra chunks
//! downloaded from their presigned URLs), polling of long queries, token renewal,
//! `abort-request` and logout.

use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::db::engine::is_cancelled;
use crate::engines::cloud_common::{bad_answer, detail, transport};
use crate::error::{AppError, AppResult, ErrorKind, Layer};

const SERVICE: &str = "Snowflake";
const ACCEPT: &str = "application/snowflake";
/// The query is still running: poll `getResultUrl`.
const IN_PROGRESS: &[&str] = &["333333", "333334"];
const SESSION_EXPIRED: &str = "390112";
/// DML statement types (insert, update, delete, merge, multi-table insert).
const DML_TYPES: std::ops::Range<u64> = 0x3000..0x4000;

/// What a login needs. The password never leaves this struct except in the login body.
#[derive(Clone)]
pub struct Login {
    pub base: String,
    pub account: String,
    pub user: String,
    pub password: String,
    pub database: String,
    pub warehouse: String,
    pub role: String,
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    success: bool,
}

/// A column of a result (`rowtype`).
#[derive(Debug, Clone, Deserialize)]
pub struct RowType {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub precision: Option<i64>,
    #[serde(default)]
    pub scale: Option<i64>,
}

#[derive(Deserialize)]
struct Chunk {
    url: String,
}

/// A finished statement: columns, the rows kept (raw JSON cells), counts.
pub struct Output {
    pub columns: Vec<RowType>,
    pub rows: Vec<Vec<Value>>,
    pub total: u64,
    pub affected: Option<u64>,
}

struct Tokens {
    session: String,
    master: String,
}

pub struct Conn {
    http: reqwest::Client,
    base: String,
    tokens: RwLock<Tokens>,
    seq: AtomicU64,
    pub database: String,
    pub version: String,
}

fn auth(token: &str) -> String {
    format!("Snowflake Token=\"{token}\"")
}

fn request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

async fn read_answer(resp: reqwest::Response) -> AppResult<Answer> {
    let status = resp.status();
    let body = resp.text().await.map_err(transport)?;
    serde_json::from_str(&body).map_err(|_| match status.as_u16() {
        401 | 403 => AppError::auth_failed(format!("HTTP {status}"))
            .hint("Check the account identifier, user and password."),
        404 => AppError::validation("Snowflake doesn't know this account (HTTP 404).")
            .hint("Check the account identifier, e.g. xy12345.eu-central-1 or orgname-account."),
        _ => bad_answer(SERVICE, format!("HTTP {status}")),
    })
}

/// A failed answer as an error with Snowflake's code.
fn failure(a: &Answer) -> AppError {
    let code = a.code.clone().unwrap_or_default();
    let message = a.message.clone().unwrap_or_else(|| "unknown error".into());
    let data = a.data.as_ref();
    let sql_state = data.and_then(|d| d.get("sqlState")).cloned();
    let query_id = data.and_then(|d| d.get("queryId")).cloned();
    AppError::query(message).data(json!({
        "code": code, "sqlState": sql_state, "queryId": query_id,
    }))
}

impl Conn {
    /// Logs in with the password; fails with `auth_failed` on a refused login.
    pub async fn login(http: reqwest::Client, l: &Login) -> AppResult<Conn> {
        let body = json!({ "data": {
            "CLIENT_APP_ID": "LibraTable",
            "CLIENT_APP_VERSION": env!("CARGO_PKG_VERSION"),
            "SVN_REVISION": "",
            "ACCOUNT_NAME": l.account,
            "LOGIN_NAME": l.user,
            "PASSWORD": l.password,
            "CLIENT_ENVIRONMENT": {
                "APPLICATION": "LibraTable",
                "OS": std::env::consts::OS,
            },
            "SESSION_PARAMETERS": {
                "QUERY_RESULT_FORMAT": "JSON",
                "CLIENT_SESSION_KEEP_ALIVE": false,
            },
        }});
        let mut query = vec![("request_id", request_id())];
        for (key, v) in [
            ("databaseName", &l.database),
            ("warehouse", &l.warehouse),
            ("roleName", &l.role),
        ] {
            if !v.is_empty() {
                query.push((key, v.clone()));
            }
        }
        let resp = http
            .post(format!("{}/session/v1/login-request", l.base))
            .query(&query)
            .header("Accept", ACCEPT)
            .json(&body)
            .send()
            .await
            .map_err(transport)?;
        let a = read_answer(resp).await?;
        if !a.success {
            let code = a.code.clone().unwrap_or_default();
            let message = a.message.clone().unwrap_or_default();
            return Err(if code.starts_with("3901") || code.starts_with("3902") {
                AppError::auth_failed(message)
            } else {
                AppError::new(ErrorKind::DbConnect, Layer::Database, message)
                    .hint("Check the database, warehouse and role names.")
            });
        }
        let data = a.data.unwrap_or_default();
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let info = data.get("sessionInfo").cloned().unwrap_or_default();
        Ok(Conn {
            http,
            base: l.base.clone(),
            tokens: RwLock::new(Tokens {
                session: text(&data, "token"),
                master: text(&data, "masterToken"),
            }),
            seq: AtomicU64::new(0),
            database: text(&info, "databaseName"),
            version: text(&data, "serverVersion"),
        })
    }

    fn token(&self) -> String {
        self.tokens.read().unwrap().session.clone()
    }

    /// Swaps an expired session token for a new one with the master token.
    async fn renew(&self) -> AppResult<()> {
        let (old, master) = {
            let t = self.tokens.read().unwrap();
            (t.session.clone(), t.master.clone())
        };
        let resp = self
            .http
            .post(format!("{}/session/token-request", self.base))
            .query(&[("requestId", request_id())])
            .header("Authorization", auth(&master))
            .header("Accept", ACCEPT)
            .json(&json!({ "oldSessionToken": old, "requestType": "RENEW" }))
            .send()
            .await
            .map_err(transport)?;
        let a = read_answer(resp).await?;
        if !a.success {
            return Err(AppError::auth_failed(a.message.unwrap_or_default())
                .hint("The Snowflake session expired; reconnect."));
        }
        let data = a.data.unwrap_or_default();
        let get = |k: &str| data.get(k).and_then(Value::as_str).map(str::to_string);
        let mut t = self.tokens.write().unwrap();
        if let Some(s) = get("sessionToken") {
            t.session = s;
        }
        if let Some(m) = get("masterToken") {
            t.master = m;
        }
        Ok(())
    }

    async fn post_query(&self, rid: &str, sql: &str, binds: &[String]) -> AppResult<Answer> {
        let mut body = json!({
            "sqlText": sql,
            "asyncExec": false,
            "sequenceId": self.seq.fetch_add(1, Ordering::SeqCst) + 1,
            "querySubmissionTime": now_ms(),
            "isInternal": false,
        });
        if !binds.is_empty() {
            let b: Map<String, Value> = binds
                .iter()
                .enumerate()
                .map(|(i, v)| ((i + 1).to_string(), json!({ "type": "TEXT", "value": v })))
                .collect();
            body["bindings"] = Value::Object(b);
        }
        let resp = self
            .http
            .post(format!("{}/queries/v1/query-request", self.base))
            .query(&[("requestId", rid)])
            .header("Authorization", auth(&self.token()))
            .header("Accept", ACCEPT)
            .json(&body)
            .send()
            .await
            .map_err(transport)?;
        read_answer(resp).await
    }

    async fn get_result(&self, path: &str) -> AppResult<Answer> {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}{path}", self.base)
        };
        let resp = self
            .http
            .get(url)
            .header("Authorization", auth(&self.token()))
            .header("Accept", ACCEPT)
            .send()
            .await
            .map_err(transport)?;
        read_answer(resp).await
    }

    /// Runs one statement (request id `rid`, so it can be aborted), keeping up to
    /// `limit` rows. Polls while it runs and stops when `cancelled` is set.
    pub async fn query(
        &self,
        rid: &str,
        sql: &str,
        binds: &[String],
        limit: usize,
        cancelled: Option<&AtomicBool>,
    ) -> AppResult<Output> {
        let mut a = self.post_query(rid, sql, binds).await?;
        if a.code.as_deref() == Some(SESSION_EXPIRED) {
            self.renew().await?;
            a = self.post_query(rid, sql, binds).await?;
        }
        let mut wait = Duration::from_millis(250);
        while a.code.as_deref().is_some_and(|c| IN_PROGRESS.contains(&c)) {
            if cancelled.is_some_and(is_cancelled) {
                return Err(AppError::cancelled());
            }
            let path = a
                .data
                .as_ref()
                .and_then(|d| d.get("getResultUrl"))
                .and_then(Value::as_str)
                .ok_or_else(|| bad_answer(SERVICE, "a running query without a result URL"))?
                .to_string();
            tokio::time::sleep(wait).await;
            wait = (wait * 2).min(Duration::from_secs(2));
            a = self.get_result(&path).await?;
        }
        if !a.success {
            return Err(failure(&a));
        }
        self.output(a.data.unwrap_or_default(), limit).await
    }

    async fn output(&self, data: Value, limit: usize) -> AppResult<Output> {
        let columns: Vec<RowType> =
            serde_json::from_value(data.get("rowtype").cloned().unwrap_or(json!([])))
                .map_err(|e| bad_answer(SERVICE, e))?;
        let mut rows: Vec<Vec<Value>> =
            serde_json::from_value(data.get("rowset").cloned().unwrap_or(json!([])))
                .map_err(|e| bad_answer(SERVICE, e))?;
        rows.truncate(limit);
        let chunks: Vec<Chunk> =
            serde_json::from_value(data.get("chunks").cloned().unwrap_or(json!([])))
                .map_err(|e| bad_answer(SERVICE, e))?;
        for chunk in &chunks {
            if rows.len() >= limit {
                break;
            }
            let mut more = self.chunk(&data, &chunk.url).await?;
            more.truncate(limit - rows.len());
            rows.append(&mut more);
        }
        let statement = data.get("statementTypeId").and_then(Value::as_u64);
        let affected = statement
            .filter(|t| DML_TYPES.contains(t))
            .map(|_| dml_count(&columns, rows.first()));
        let total = data
            .get("total")
            .and_then(Value::as_u64)
            .unwrap_or(rows.len() as u64);
        Ok(Output {
            columns,
            rows,
            total,
            affected,
        })
    }

    /// One result chunk: gzip'd JSON rows without the outer brackets.
    async fn chunk(&self, data: &Value, url: &str) -> AppResult<Vec<Vec<Value>>> {
        let mut req = self.http.get(url);
        match data.get("chunkHeaders").and_then(Value::as_object) {
            Some(headers) => {
                for (k, v) in headers {
                    req = req.header(k.as_str(), v.as_str().unwrap_or_default());
                }
            }
            None => {
                if let Some(key) = data.get("qrmk").and_then(Value::as_str) {
                    req = req
                        .header("x-amz-server-side-encryption-customer-algorithm", "AES256")
                        .header("x-amz-server-side-encryption-customer-key", key);
                }
            }
        }
        let resp = req.send().await.map_err(transport)?;
        if !resp.status().is_success() {
            let what = format!("result chunk download failed (HTTP {})", resp.status());
            return Err(bad_answer(SERVICE, what));
        }
        let bytes = resp.bytes().await.map_err(transport)?;
        parse_chunk(&bytes)
    }

    /// Stops the statement sent with request id `rid`.
    pub async fn abort(&self, rid: &str, sql: &str) -> AppResult<()> {
        let resp = self
            .http
            .post(format!("{}/queries/v1/abort-request", self.base))
            .query(&[("requestId", request_id())])
            .header("Authorization", auth(&self.token()))
            .header("Accept", ACCEPT)
            .json(&json!({ "sqlText": sql, "requestId": rid }))
            .send()
            .await
            .map_err(transport)?;
        let a = read_answer(resp).await?;
        if a.success {
            Ok(())
        } else {
            Err(failure(&a))
        }
    }

    /// Ends the session on the server (best effort).
    pub async fn logout(&self) {
        let sent = self
            .http
            .post(format!("{}/session", self.base))
            .query(&[("delete", "true"), ("requestId", &request_id())])
            .header("Authorization", auth(&self.token()))
            .header("Accept", ACCEPT)
            .json(&json!({}))
            .send()
            .await;
        if let Err(e) = sent {
            tracing::debug!("snowflake logout failed: {}", detail(&e));
        }
    }
}

pub fn new_request_id() -> String {
    request_id()
}

/// Rows changed by a DML statement, as its result columns report them.
fn dml_count(columns: &[RowType], row: Option<&Vec<Value>>) -> u64 {
    let Some(row) = row else { return 0 };
    columns
        .iter()
        .zip(row)
        .filter(|(c, _)| {
            let n = c.name.as_str();
            n.starts_with("number of rows inserted")
                || n == "number of rows updated"
                || n == "number of multi-joined rows updated"
                || n == "number of rows deleted"
        })
        .filter_map(|(_, v)| match v {
            Value::String(s) => s.parse::<u64>().ok(),
            other => other.as_u64(),
        })
        .sum()
}

/// A JSON result chunk (`[..],[..]`, gzip'd or not) as rows.
pub fn parse_chunk(bytes: &[u8]) -> AppResult<Vec<Vec<Value>>> {
    let mut text = String::new();
    if bytes.starts_with(&[0x1f, 0x8b]) {
        flate2::read::MultiGzDecoder::new(bytes)
            .read_to_string(&mut text)
            .map_err(|e| bad_answer(SERVICE, e))?;
    } else {
        text = String::from_utf8_lossy(bytes).into_owned();
    }
    serde_json::from_str(&format!("[{text}]")).map_err(|e| bad_answer(SERVICE, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn chunks_may_be_gzipped() {
        let raw = br#"["1","a"],["2",null]"#;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(raw).unwrap();
        let packed = gz.finish().unwrap();
        let rows = parse_chunk(&packed).unwrap();
        assert_eq!(
            rows,
            vec![vec![json!("1"), json!("a")], vec![json!("2"), Value::Null]]
        );
        assert_eq!(parse_chunk(raw).unwrap().len(), 2);
    }

    #[test]
    fn dml_counts_add_up() {
        let col = |n: &str| RowType {
            name: n.into(),
            kind: "fixed".into(),
            precision: None,
            scale: None,
        };
        let cols = vec![
            col("number of rows updated"),
            col("number of multi-joined rows updated"),
        ];
        assert_eq!(dml_count(&cols, Some(&vec![json!("2"), json!("1")])), 3);
        assert_eq!(dml_count(&cols, None), 0);
    }
}
