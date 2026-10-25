//! SSH tunnels (SSH-1..SSH-7): a local listener on 127.0.0.1:<random port> whose
//! connections are forwarded through the SSH server to the database host.

mod auth;
pub mod config;
mod host_key;
pub mod known_hosts;
mod tunnel;

pub use host_key::PendingHostKey;
pub use tunnel::Tunnel;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client;

use crate::error::{AppError, AppResult, ErrorKind, Layer};
use host_key::Client;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

pub enum KeySource {
    /// Key imported into the keychain for this connection.
    Stored { name: String, pem: String },
    /// Key file from ~/.ssh/config or the OpenSSH defaults.
    File(PathBuf),
}

impl KeySource {
    fn name(&self) -> String {
        match self {
            KeySource::Stored { name, .. } => name.clone(),
            KeySource::File(p) => p.display().to_string(),
        }
    }
}

/// True for keys that need a passphrase. OpenSSH keys report `KeyIsEncrypted`, but
/// encrypted PKCS#8 / legacy PEM keys fail with other errors, so read the header too.
pub fn is_encrypted_key(pem: &str, decode_error: &russh_keys::Error) -> bool {
    matches!(decode_error, russh_keys::Error::KeyIsEncrypted)
        || pem.contains("BEGIN ENCRYPTED PRIVATE KEY")
        || pem.contains("Proc-Type: 4,ENCRYPTED")
}

pub struct TunnelParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub keys: Vec<KeySource>,
    pub passphrase: Option<String>,
    pub target_host: String,
    pub target_port: u16,
    pub known_hosts: PathBuf,
}

/// Outcome of `open` when the server key needs the user's decision.
pub enum OpenError {
    HostKey(Box<PendingHostKey>),
    Other(AppError),
}

impl From<AppError> for OpenError {
    fn from(e: AppError) -> Self {
        OpenError::Other(e)
    }
}

/// Connects, checks the host key, logs in, checks that the database is reachable and
/// starts forwarding.
pub async fn open(p: TunnelParams) -> Result<Tunnel, OpenError> {
    let seen = Arc::new(Mutex::new(None));
    let handler = Client {
        host: p.host.clone(),
        port: p.port,
        known_hosts: p.known_hosts.clone(),
        seen: seen.clone(),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    let addr = (p.host.clone(), p.port);
    let connected =
        tokio::time::timeout(CONNECT_TIMEOUT, client::connect(config, addr, handler)).await;
    let mut handle = match connected {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => {
            if let Some(pending) = seen.lock().unwrap().take() {
                return Err(OpenError::HostKey(Box::new(pending)));
            }
            return Err(unreachable(&p, &e.to_string()).into());
        }
        Err(_) => return Err(unreachable(&p, "timed out").into()),
    };
    auth::authenticate(&mut handle, &p).await?;
    Ok(Tunnel::start(Arc::new(handle), &p).await?)
}

fn unreachable(p: &TunnelParams, detail: &str) -> AppError {
    AppError::new(
        ErrorKind::SshUnreachable,
        Layer::Ssh,
        format!(
            "Can't connect to the SSH server {}:{} ({detail}).",
            p.host, p.port
        ),
    )
    .hint("Check the SSH server address and port, and that this machine can reach it.")
}

/// Trusts a pending host key: replaces old entries for the host and appends the new key.
pub fn trust(pending: &PendingHostKey, path: &Path) -> AppResult<()> {
    if pending.changed {
        known_hosts::remove_host(path, &pending.host, pending.port)?;
    }
    russh_keys::learn_known_hosts_path(&pending.host, pending.port, &pending.key, path).map_err(
        |e| {
            AppError::new(
                ErrorKind::Io,
                Layer::Ssh,
                format!("Couldn't write known_hosts: {e}"),
            )
        },
    )
}
