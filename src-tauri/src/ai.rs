//! The SQL assistant: one request to Anthropic's Messages API (raw HTTP; there is no
//! official Rust SDK) that turns a question about the database into a query. The API key
//! lives in the Keychain; the schema summary and the question come from the UI.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Server-side fallback when a safety classifier declines (the "default" routing).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// Models the settings may pick; the first is the default.
pub const MODELS: &[&str] = &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"];

/// Keychain account (app-wide) of the API key.
pub const KEY_NAME: &str = "anthropic_api_key";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskRequest {
    pub question: String,
    /// "postgres", "mysql", "sqlite"… and the server version, for the right syntax.
    pub dialect: String,
    /// Tables and their columns, one per line, as the UI summarised them.
    pub schema: String,
    /// The editor's text (or selection), when the question is about it.
    #[serde(default)]
    pub current_sql: String,
    #[serde(default)]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AskAnswer {
    pub sql: String,
    /// What the model said besides the query (assumptions, caveats).
    pub note: String,
    pub model: String,
}

const SYSTEM: &str = "You write SQL for a desktop database client. Answer with one SQL \
code block (```sql … ```) that does what the user asks, written for the given dialect and \
using only the tables and columns listed in the schema. After the block, add at most two \
short sentences on assumptions, if any. If the schema can't answer the question, say so \
in one sentence instead of guessing names. Prefer read-only queries unless the user \
clearly asks to change data.";

/// The request body: the model, its effort, the fallback and the one user turn.
pub fn request_body(r: &AskRequest) -> Value {
    let model = if MODELS.contains(&r.model.as_str()) {
        r.model.as_str()
    } else {
        DEFAULT_MODEL
    };
    let mut user = format!(
        "Dialect: {}\n\nSchema (table: columns):\n{}\n\n",
        r.dialect, r.schema
    );
    if !r.current_sql.trim().is_empty() {
        user.push_str(&format!(
            "Current query:\n```sql\n{}\n```\n\n",
            r.current_sql
        ));
    }
    user.push_str(&format!("Request: {}", r.question));
    let mut body = json!({
        "model": model,
        "max_tokens": 16000,
        "output_config": { "effort": "medium" },
        "system": SYSTEM,
        "messages": [{ "role": "user", "content": user }],
    });
    if uses_fallback(model) {
        body["fallbacks"] = json!("default");
    }
    body
}

/// Haiku has no server-side fallback.
fn uses_fallback(model: &str) -> bool {
    model != "claude-haiku-5-5"
}

pub fn beta_header(model: &str) -> Option<&'static str> {
    uses_fallback(model).then_some(FALLBACK_BETA)
}

/// The SQL block and the rest of the text of a Messages API response.
pub fn parse_answer(resp: &Value) -> AppResult<AskAnswer> {
    if resp["stop_reason"] == "refusal" {
        return Err(AppError::validation(
            "The assistant declined this request. Rephrase it and try again.",
        ));
    }
    let text: String = resp["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let model = resp["model"].as_str().unwrap_or_default().to_string();
    let (sql, note) = split_sql(&text);
    if sql.is_empty() && note.is_empty() {
        return Err(AppError::query("The assistant returned no answer."));
    }
    Ok(AskAnswer { sql, note, model })
}

/// The first ```sql block (or bare ``` block) and the text around it.
fn split_sql(text: &str) -> (String, String) {
    let Some(start) = text.find("```") else {
        return (String::new(), text.trim().to_string());
    };
    let after = &text[start + 3..];
    let body_start = after.find('\n').map_or(0, |i| i + 1);
    let body = &after[body_start..];
    let end = body.find("```").unwrap_or(body.len());
    let sql = body[..end].trim().to_string();
    let rest = body.get(end + 3..).unwrap_or("");
    let note = format!("{} {}", text[..start].trim(), rest.trim());
    (sql, note.trim().to_string())
}

/// An error the API returned, worded for the user.
pub fn api_error(status: u16, body: &Value) -> AppError {
    let message = body["error"]["message"]
        .as_str()
        .unwrap_or("Request failed.");
    let hint = match status {
        401 | 403 => "Check the API key in Settings › SQL assistant.",
        429 => "Rate limited: wait a moment and try again.",
        500..=599 => "The service is having trouble; try again shortly.",
        _ => "",
    };
    let err = AppError::query(format!("Assistant error ({status}): {message}"));
    if hint.is_empty() {
        err
    } else {
        err.hint(hint)
    }
}

pub async fn ask(key: &str, r: &AskRequest) -> AppResult<AskAnswer> {
    let body = request_body(r);
    let model = body["model"].as_str().unwrap_or(DEFAULT_MODEL).to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut req = client
        .post(API_URL)
        .header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json");
    if let Some(beta) = beta_header(&model) {
        req = req.header("anthropic-beta", beta);
    }
    let resp = req
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::query(format!("Couldn't reach the assistant: {e}")))?;
    let status = resp.status().as_u16();
    let value: Value = resp
        .json()
        .await
        .map_err(|e| AppError::query(format!("The assistant's answer couldn't be read: {e}")))?;
    if status >= 400 {
        return Err(api_error(status, &value));
    }
    parse_answer(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask_req(model: &str) -> AskRequest {
        AskRequest {
            question: "top 5 customers".into(),
            dialect: "postgres 16".into(),
            schema: "customers: id, name".into(),
            current_sql: String::new(),
            model: model.into(),
        }
    }

    #[test]
    fn builds_the_request() {
        let body = request_body(&ask_req(""));
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["output_config"]["effort"], "medium");
        let user = body["messages"][0]["content"].as_str().unwrap();
        assert!(user.contains("customers: id, name") && user.ends_with("top 5 customers"));
        let haiku = request_body(&ask_req("claude-haiku-5-5"));
        assert!(haiku.get("fallbacks").is_none());
        assert_eq!(beta_header("claude-haiku-5-5"), None);
        assert_eq!(request_body(&ask_req("gpt"))["model"], DEFAULT_MODEL);
    }

    #[test]
    fn reads_the_sql_and_the_note() {
        let resp = json!({
            "model": "claude-opus-5-5",
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "" },
                { "type": "text", "text": "```sql\nSELECT 1;\n```\nAssumes one row." }
            ]
        });
        let a = parse_answer(&resp).unwrap();
        assert_eq!(a.sql, "SELECT 1;");
        assert_eq!(a.note, "Assumes one row.");
        let refused = json!({ "stop_reason": "refusal", "content": [] });
        assert!(parse_answer(&refused).is_err());
        let plain = json!({ "content": [{ "type": "text", "text": "No such table." }] });
        assert_eq!(parse_answer(&plain).unwrap().sql, "");
    }
}
