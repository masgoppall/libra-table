//! Service-account login: an RS256-signed JWT (ring, no OpenSSL) exchanged at the key's
//! `token_uri` for an access token, cached until shortly before it expires.

use std::time::{Duration, Instant};

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use ring::rand::SystemRandom;
use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::engines::cloud_common::transport;
use crate::error::{AppError, AppResult};

pub const SCOPE: &str = "https://www.googleapis.com/auth/bigquery";
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const GRANT: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
/// Tokens are renewed this long before they expire.
const EARLY: Duration = Duration::from_secs(120);
const LIFETIME_SECS: i64 = 3600;

#[derive(Deserialize)]
struct KeyFile {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    project_id: String,
    #[serde(default)]
    private_key_id: String,
    private_key: String,
    client_email: String,
    #[serde(default)]
    token_uri: String,
}

/// A parsed service-account key. Its fields never appear in errors or logs.
pub struct ServiceKey {
    pub email: String,
    pub project_id: String,
    key_id: String,
    token_uri: String,
    pair: RsaKeyPair,
}

fn bad_key(what: &str) -> AppError {
    AppError::validation(format!("The service account key {what}."))
        .hint("Paste the whole JSON key file you downloaded from Google Cloud (IAM → Keys).")
}

impl ServiceKey {
    pub fn parse(json_text: &str) -> AppResult<ServiceKey> {
        let k: KeyFile =
            serde_json::from_str(json_text.trim()).map_err(|_| bad_key("is not valid JSON"))?;
        if !k.kind.is_empty() && k.kind != "service_account" {
            return Err(bad_key("is not a service account key"));
        }
        let pem: String = k
            .private_key
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .map(str::trim)
            .collect();
        let der = STANDARD
            .decode(pem)
            .map_err(|_| bad_key("has an unreadable private key"))?;
        let pair = RsaKeyPair::from_pkcs8(&der)
            .map_err(|_| bad_key("has a private key that isn't a PKCS#8 RSA key"))?;
        Ok(ServiceKey {
            email: k.client_email,
            project_id: k.project_id,
            key_id: k.private_key_id,
            token_uri: if k.token_uri.is_empty() {
                GOOGLE_TOKEN_URI.into()
            } else {
                k.token_uri
            },
            pair,
        })
    }

    /// The signed assertion for the token request, issued at `now` (Unix seconds).
    pub fn assertion(&self, now: i64) -> AppResult<String> {
        let mut header = json!({ "alg": "RS256", "typ": "JWT" });
        if !self.key_id.is_empty() {
            header["kid"] = json!(self.key_id);
        }
        let claims = json!({
            "iss": self.email,
            "scope": SCOPE,
            "aud": self.token_uri,
            "iat": now,
            "exp": now + LIFETIME_SECS,
        });
        let enc = |v: &Value| URL_SAFE_NO_PAD.encode(v.to_string());
        let signing_input = format!("{}.{}", enc(&header), enc(&claims));
        let mut sig = vec![0; self.pair.public().modulus_len()];
        self.pair
            .sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                signing_input.as_bytes(),
                &mut sig,
            )
            .map_err(|_| AppError::internal("Signing the service account token failed."))?;
        Ok(format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(sig)))
    }
}

#[derive(Deserialize)]
struct TokenAnswer {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// Access tokens for one key (None: an emulator without authentication).
pub struct Auth {
    key: Option<ServiceKey>,
    cached: tokio::sync::Mutex<Option<(String, Instant)>>,
}

impl Auth {
    pub fn new(key: Option<ServiceKey>) -> Self {
        Self {
            key,
            cached: tokio::sync::Mutex::new(None),
        }
    }

    /// The `Authorization` header value, fetching a new token when needed.
    pub async fn header(&self, http: &reqwest::Client) -> AppResult<Option<String>> {
        let Some(key) = &self.key else {
            return Ok(None);
        };
        let mut cached = self.cached.lock().await;
        if let Some((token, until)) = cached.as_ref() {
            if Instant::now() < *until {
                return Ok(Some(format!("Bearer {token}")));
            }
        }
        let (token, ttl) = fetch(http, key).await?;
        let until = Instant::now() + Duration::from_secs(ttl).saturating_sub(EARLY);
        *cached = Some((token.clone(), until));
        Ok(Some(format!("Bearer {token}")))
    }

    /// Drops the cached token (after the API refused it).
    pub async fn forget(&self) {
        *self.cached.lock().await = None;
    }
}

async fn fetch(http: &reqwest::Client, key: &ServiceKey) -> AppResult<(String, u64)> {
    let assertion = key.assertion(chrono::Utc::now().timestamp())?;
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", GRANT)
        .append_pair("assertion", &assertion)
        .finish();
    let resp = http
        .post(&key.token_uri)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(transport)?;
    let status = resp.status();
    let a: TokenAnswer = resp
        .json()
        .await
        .map_err(|_| AppError::auth_failed(format!("the token endpoint answered HTTP {status}")))?;
    if a.access_token.is_empty() {
        let why = a
            .error_description
            .or(a.error)
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(AppError::auth_failed(why)
            .hint("Check that the service account key is current and not disabled."));
    }
    Ok((a.access_token, a.expires_in.unwrap_or(LIFETIME_SECS as u64)))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use ring::signature::{UnparsedPublicKey, RSA_PKCS1_2048_8192_SHA256};
    use rsa::pkcs1::EncodeRsaPublicKey;
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};

    /// A throwaway key file and the DER of its public key.
    pub fn throwaway_key(token_uri: &str) -> (String, Vec<u8>) {
        let key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).unwrap();
        let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let public = key
            .to_public_key()
            .to_pkcs1_der()
            .unwrap()
            .as_bytes()
            .to_vec();
        let file = json!({
            "type": "service_account",
            "project_id": "libratable-test",
            "private_key_id": "test-key-1",
            "private_key": pem,
            "client_email": "tester@libratable-test.iam.gserviceaccount.com",
            "token_uri": token_uri,
        });
        (file.to_string(), public)
    }

    #[test]
    fn signs_a_verifiable_jwt() {
        let (file, public) = throwaway_key("http://127.0.0.1:1/token");
        let key = ServiceKey::parse(&file).unwrap();
        let jwt = key.assertion(1_700_000_000).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        assert_eq!(
            header,
            json!({"alg": "RS256", "typ": "JWT", "kid": "test-key-1"})
        );
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["aud"], "http://127.0.0.1:1/token");
        assert_eq!(claims["scope"], SCOPE);
        assert_eq!(claims["exp"], 1_700_003_600);
        let sig = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        let signed = format!("{}.{}", parts[0], parts[1]);
        UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, &public)
            .verify(signed.as_bytes(), &sig)
            .expect("signature verifies");
    }

    #[test]
    fn rejects_bad_keys_without_echoing_them() {
        let e = ServiceKey::parse("{\"private_key\": \"secret-ish\"}")
            .err()
            .unwrap();
        assert!(!e.message.contains("secret-ish"));
        let e = ServiceKey::parse("not json").err().unwrap();
        assert!(e.message.contains("not valid JSON"));
        let pem = "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----";
        let file = json!({"private_key": pem, "client_email": "x"});
        assert!(ServiceKey::parse(&file.to_string()).is_err());
    }
}
