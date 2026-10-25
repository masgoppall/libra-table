//! BigQuery REST v2 calls: jobs.query + jobs.getQueryResults (polling and paging),
//! jobs.cancel, projects.list, datasets.list, tables.list and tables.get.

use std::sync::atomic::AtomicBool;

use reqwest::{Method, RequestBuilder};
use serde::Deserialize;
use serde_json::{json, Value};

use super::auth::Auth;
use super::values::Field;
use crate::db::engine::is_cancelled;
use crate::engines::cloud_common::{bad_answer, cancelled, transport};
use crate::error::{AppError, AppResult};

const SERVICE: &str = "BigQuery";
pub const GOOGLE_API: &str = "https://bigquery.googleapis.com";
/// How long one jobs.query / getQueryResults call waits for the job before answering.
const WAIT_MS: u64 = 10_000;
/// Rows asked for per result page.
const PAGE_ROWS: usize = 10_000;

pub struct Api {
    http: reqwest::Client,
    /// `…/bigquery/v2`.
    base: String,
    auth: Auth,
    /// The project jobs run (and are billed) in.
    pub project: String,
    pub location: String,
}

/// A job, as the API names it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRef {
    pub project_id: String,
    pub job_id: String,
    #[serde(default)]
    pub location: Option<String>,
}

/// What a query should run with.
#[derive(Default)]
pub struct QueryArgs<'a> {
    pub sql: &'a str,
    /// `@p1…` values, all STRING.
    pub params: &'a [String],
    /// (project, dataset) for unqualified table names.
    pub default_dataset: Option<(&'a str, &'a str)>,
    /// Joins this session, or starts one when `create_session`.
    pub session: Option<&'a str>,
    pub create_session: bool,
}

/// A finished query: its fields, the rows kept (raw `{f: […]}`), counts, session.
pub struct Results {
    pub fields: Vec<Field>,
    pub rows: Vec<Value>,
    pub total: u64,
    pub dml_rows: Option<u64>,
    pub session: Option<String>,
}

fn num(v: Option<&Value>) -> Option<u64> {
    match v? {
        Value::String(s) => s.parse().ok(),
        other => other.as_u64(),
    }
}

/// The API error as a message, plus whether the token was refused.
fn api_error(status: reqwest::StatusCode, body: &Value) -> AppError {
    let err = body.get("error");
    let message = err
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("HTTP {status}"));
    let reason = err
        .and_then(|e| e.pointer("/errors/0/reason"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    match status.as_u16() {
        401 => AppError::auth_failed(message)
            .hint("Check the service account key; it may have been deleted or disabled."),
        403 if reason == "accessDenied" => AppError::query(message)
            .hint("Grant the service account a BigQuery role on this project or dataset."),
        _ => AppError::query(message).data(json!({ "reason": reason })),
    }
}

impl Api {
    pub fn new(http: reqwest::Client, root: &str, auth: Auth, project: &str, loc: &str) -> Self {
        Self {
            http,
            base: format!("{root}/bigquery/v2"),
            auth,
            project: project.to_string(),
            location: loc.to_string(),
        }
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        build: impl Fn(RequestBuilder) -> RequestBuilder,
    ) -> AppResult<Value> {
        let mut retried = false;
        loop {
            let mut req = build(
                self.http
                    .request(method.clone(), format!("{}{path}", self.base)),
            );
            if let Some(h) = self.auth.header(&self.http).await? {
                req = req.header("Authorization", h);
            }
            let resp = req.send().await.map_err(transport)?;
            let status = resp.status();
            let text = resp.text().await.map_err(transport)?;
            let body: Value = if text.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&text)
                    .map_err(|_| bad_answer(SERVICE, format!("HTTP {status}")))?
            };
            if status.is_success() {
                return Ok(body);
            }
            if status.as_u16() == 401 && !retried {
                // The cached token may have been revoked early: fetch a new one once.
                self.auth.forget().await;
                retried = true;
                continue;
            }
            return Err(api_error(status, &body));
        }
    }

    pub async fn get(&self, path: &str, query: &[(&str, String)]) -> AppResult<Value> {
        self.call(Method::GET, path, |r| r.query(query)).await
    }

    /// Every item of a paged list (`items_key`), following `nextPageToken`.
    pub async fn list(
        &self,
        path: &str,
        items_key: &str,
        query: &[(&str, String)],
    ) -> AppResult<Vec<Value>> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut q = query.to_vec();
            if let Some(t) = &token {
                q.push(("pageToken", t.clone()));
            }
            let page = self.get(path, &q).await?;
            if let Some(items) = page.get(items_key).and_then(Value::as_array) {
                out.extend(items.iter().cloned());
            }
            token = page
                .get("nextPageToken")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .map(str::to_string);
            if token.is_none() {
                return Ok(out);
            }
        }
    }

    fn job_query(&self, job: &JobRef) -> Vec<(&'static str, String)> {
        let mut q = vec![("timeoutMs", WAIT_MS.to_string())];
        let loc = job
            .location
            .clone()
            .unwrap_or_else(|| self.location.clone());
        if !loc.is_empty() {
            q.push(("location", loc));
        }
        q.push(("formatOptions.useInt64Timestamp", "true".into()));
        q
    }

    /// Runs a query and keeps up to `limit` rows. `on_job` learns the job id (for
    /// cancelling); `cancelled` stops the wait and cancels the job.
    pub async fn query(
        &self,
        a: &QueryArgs<'_>,
        limit: usize,
        flag: Option<&AtomicBool>,
        on_job: &(dyn Fn(Option<JobRef>) + Sync),
    ) -> AppResult<Results> {
        let mut body = json!({
            "query": a.sql,
            "useLegacySql": false,
            "maxResults": limit.clamp(1, PAGE_ROWS),
            "timeoutMs": WAIT_MS,
            "formatOptions": { "useInt64Timestamp": true },
        });
        if !self.location.is_empty() {
            body["location"] = json!(self.location);
        }
        if !a.params.is_empty() {
            body["parameterMode"] = json!("NAMED");
            body["queryParameters"] = a
                .params
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    json!({
                        "name": format!("p{}", i + 1),
                        "parameterType": { "type": "STRING" },
                        "parameterValue": { "value": v },
                    })
                })
                .collect();
        }
        if let Some((p, d)) = a.default_dataset {
            body["defaultDataset"] = json!({ "projectId": p, "datasetId": d });
        }
        if let Some(s) = a.session {
            body["connectionProperties"] = json!([{ "key": "session_id", "value": s }]);
        } else if a.create_session {
            body["createSession"] = json!(true);
        }
        let path = format!("/projects/{}/queries", seg(&self.project));
        let mut page = self.call(Method::POST, &path, |r| r.json(&body)).await?;
        let job: Option<JobRef> = page
            .get("jobReference")
            .and_then(|j| serde_json::from_value(j.clone()).ok());
        on_job(job.clone());
        let res = self.finish(&mut page, job.as_ref(), limit, flag).await;
        on_job(None);
        res
    }

    async fn finish(
        &self,
        page: &mut Value,
        job: Option<&JobRef>,
        limit: usize,
        flag: Option<&AtomicBool>,
    ) -> AppResult<Results> {
        while page.get("jobComplete").and_then(Value::as_bool) == Some(false) {
            let job = job.ok_or_else(|| bad_answer(SERVICE, "a running query without a job"))?;
            if flag.is_some_and(is_cancelled) {
                let _ = self.cancel(job).await;
                return Err(cancelled());
            }
            *page = self.results_page(job, None, limit).await?;
        }
        let fields: Vec<Field> =
            serde_json::from_value(page.pointer("/schema/fields").cloned().unwrap_or(json!([])))
                .map_err(|e| bad_answer(SERVICE, e))?;
        let mut rows: Vec<Value> = page
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let total = num(page.get("totalRows")).unwrap_or(rows.len() as u64);
        let dml_rows = num(page.get("numDmlAffectedRows"));
        let session = page
            .pointer("/sessionInfo/sessionId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut token = page
            .get("pageToken")
            .and_then(Value::as_str)
            .map(str::to_string);
        while rows.len() < limit {
            let (Some(job), Some(t)) = (job, token.take()) else {
                break;
            };
            if flag.is_some_and(is_cancelled) {
                return Err(cancelled());
            }
            let next = self.results_page(job, Some(t), limit - rows.len()).await?;
            if let Some(more) = next.get("rows").and_then(Value::as_array) {
                rows.extend(more.iter().cloned());
            }
            token = next
                .get("pageToken")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        // Some servers (the emulator) ignore maxResults.
        rows.truncate(limit);
        Ok(Results {
            fields,
            rows,
            total,
            dml_rows,
            session,
        })
    }

    async fn results_page(
        &self,
        job: &JobRef,
        token: Option<String>,
        want: usize,
    ) -> AppResult<Value> {
        let mut q = self.job_query(job);
        q.push(("maxResults", want.clamp(1, PAGE_ROWS).to_string()));
        if let Some(t) = token {
            q.push(("pageToken", t));
        }
        let path = format!(
            "/projects/{}/queries/{}",
            seg(&job.project_id),
            seg(&job.job_id)
        );
        self.get(&path, &q).await
    }

    pub async fn cancel(&self, job: &JobRef) -> AppResult<()> {
        let path = format!(
            "/projects/{}/jobs/{}/cancel",
            seg(&job.project_id),
            seg(&job.job_id)
        );
        let mut q = Vec::new();
        if let Some(l) = job.location.clone().filter(|l| !l.is_empty()) {
            q.push(("location", l));
        }
        self.call(Method::POST, &path, |r| r.query(&q))
            .await
            .map(|_| ())
    }
}

/// A path segment, percent-encoded (table names may hold spaces or Unicode).
pub fn seg(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_encoded() {
        assert_eq!(seg("my-proj"), "my-proj");
        assert_eq!(seg("a b/é"), "a%20b%2F%C3%A9");
    }
}
