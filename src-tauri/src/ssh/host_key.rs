//! Host key checks against known_hosts (SSH-4), and the key the user still has to
//! accept when it is unknown or has changed.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use russh::client;
use russh_keys::key::PublicKey;
use serde_json::json;

use crate::error::{AppError, ErrorKind, Layer};

use super::known_hosts;

const CHANGED_HINT: &str = "This can mean the server was reinstalled — or that someone is \
    intercepting the connection. Continue only if you expected this.";

/// A server key the user has not trusted yet, kept until they answer the prompt.
#[derive(Clone)]
pub struct PendingHostKey {
    pub host: String,
    pub port: u16,
    pub key: PublicKey,
    pub changed: bool,
}

impl PendingHostKey {
    pub fn fingerprint(&self) -> String {
        format!("SHA256:{}", self.key.fingerprint())
    }

    pub fn to_error(&self) -> AppError {
        let data = json!({
            "host": self.host,
            "port": self.port,
            "algorithm": self.key.name(),
            "fingerprint": self.fingerprint(),
        });
        let host = known_hosts::host_key(&self.host, self.port);
        if self.changed {
            AppError::new(
                ErrorKind::SshHostKeyChanged,
                Layer::Ssh,
                format!("The SSH host key of {host} has CHANGED."),
            )
            .hint(CHANGED_HINT)
            .data(data)
        } else {
            AppError::new(
                ErrorKind::SshHostKeyUnknown,
                Layer::Ssh,
                format!("Can't verify the SSH host {host}."),
            )
            .hint("Compare the fingerprint with the one your server administrator gave you.")
            .data(data)
        }
    }
}

/// The russh client handler: accepts known keys and records any other key in `seen`.
pub(super) struct Client {
    pub host: String,
    pub port: u16,
    pub known_hosts: PathBuf,
    pub seen: Arc<Mutex<Option<PendingHostKey>>>,
}

#[async_trait]
impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool, Self::Error> {
        let verdict =
            russh_keys::check_known_hosts_path(&self.host, self.port, key, &self.known_hosts);
        let changed = match verdict {
            Ok(true) => return Ok(true),
            // russh-keys reports a key of a *different type* as merely unknown. If the host
            // already has any entry and none matched, the key has changed.
            Ok(false) => known_hosts::has_entry(&self.known_hosts, &self.host, self.port),
            Err(russh_keys::Error::KeyChanged { .. }) => true,
            // Entries we can't read for a host we may know: warn as strongly as possible.
            Err(_) => known_hosts::has_entry(&self.known_hosts, &self.host, self.port),
        };
        *self.seen.lock().unwrap() = Some(PendingHostKey {
            host: self.host.clone(),
            port: self.port,
            key: key.clone(),
            changed,
        });
        Ok(false)
    }
}
