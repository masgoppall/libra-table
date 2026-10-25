//! The DynamoDB JSON API: `POST /` with `X-Amz-Target: DynamoDB_20120810.<Action>`,
//! signed with SigV4.

use serde_json::{json, Value};

use super::sigv4::{authorization, Credentials, Request};
use crate::engines::cloud_common::{bad_answer, transport};
use crate::error::{AppError, AppResult};

const SERVICE: &str = "DynamoDB";
const CONTENT_TYPE: &str = "application/x-amz-json-1.0";

pub struct Client {
    http: reqwest::Client,
    url: String,
    /// `host[:port]` as sent in the Host header.
    host: String,
    pub region: String,
    creds: Credentials,
}

/// The short error code of an answer (`com.amazonaws…#ResourceNotFoundException` →
/// `ResourceNotFoundException`).
pub fn error_code(e: &AppError) -> &str {
    e.data
        .as_ref()
        .and_then(|d| d.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn api_error(status: reqwest::StatusCode, body: &Value) -> AppError {
    let code = body
        .get("__type")
        .and_then(Value::as_str)
        .map(|t| t.rsplit('#').next().unwrap_or(t))
        .unwrap_or("")
        .to_string();
    let message = body
        .get("message")
        .or_else(|| body.get("Message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("HTTP {status}"));
    let data = json!({
        "code": code,
        "reasons": body.get("CancellationReasons").cloned(),
    });
    match code.as_str() {
        "UnrecognizedClientException"
        | "InvalidSignatureException"
        | "MissingAuthenticationTokenException"
        | "IncompleteSignatureException" => AppError::auth_failed(message)
            .hint("Check the access key ID, the secret access key and the region.")
            .data(data),
        "AccessDeniedException" => AppError::query(message)
            .hint("The IAM user or role lacks the DynamoDB permission for this action.")
            .data(data),
        _ => AppError::query(message).data(data),
    }
}

impl Client {
    pub fn new(
        http: reqwest::Client,
        url: &str,
        region: &str,
        creds: Credentials,
    ) -> AppResult<Self> {
        let parsed = url::Url::parse(url)
            .map_err(|_| AppError::validation(format!("\"{url}\" is not a valid endpoint.")))?;
        let host = match (parsed.host_str(), parsed.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_string(),
            _ => return Err(AppError::validation("The endpoint has no host.")),
        };
        Ok(Self {
            http,
            url: url.trim_end_matches('/').to_string() + "/",
            host,
            region: region.to_string(),
            creds,
        })
    }

    /// Calls one action and returns its JSON answer.
    pub async fn call(&self, action: &str, body: &Value) -> AppResult<Value> {
        let payload = body.to_string();
        let target = format!("DynamoDB_20120810.{action}");
        let amz_date = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let headers = [
            ("content-type", CONTENT_TYPE),
            ("host", self.host.as_str()),
            ("x-amz-date", amz_date.as_str()),
            ("x-amz-target", target.as_str()),
        ];
        let path = url::Url::parse(&self.url)
            .map(|u| u.path().to_string())
            .unwrap_or_else(|_| "/".into());
        let req = Request {
            method: "POST",
            path: &path,
            query: "",
            headers: &headers,
            body: payload.as_bytes(),
        };
        let auth = authorization(&self.creds, &self.region, "dynamodb", &amz_date, &req);
        let resp = self
            .http
            .post(&self.url)
            .header("Content-Type", CONTENT_TYPE)
            .header("X-Amz-Date", &amz_date)
            .header("X-Amz-Target", &target)
            .header("Authorization", auth)
            .body(payload)
            .send()
            .await
            .map_err(transport)?;
        let status = resp.status();
        let text = resp.text().await.map_err(transport)?;
        let answer: Value = serde_json::from_str(&text)
            .map_err(|_| bad_answer(SERVICE, format!("HTTP {status}")))?;
        if status.is_success() {
            Ok(answer)
        } else {
            Err(api_error(status, &answer))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_keep_their_short_code() {
        let body = json!({
            "__type": "com.amazonaws.dynamodb.v20120810#ConditionalCheckFailedException",
            "message": "The conditional request failed"
        });
        let e = api_error(reqwest::StatusCode::BAD_REQUEST, &body);
        assert_eq!(error_code(&e), "ConditionalCheckFailedException");
        let body = json!({ "__type": "x#UnrecognizedClientException", "Message": "bad key" });
        let e = api_error(reqwest::StatusCode::BAD_REQUEST, &body);
        assert_eq!(e.kind, crate::error::ErrorKind::DbAuth);
    }
}
