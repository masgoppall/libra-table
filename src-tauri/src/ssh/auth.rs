//! SSH login: each private key in turn, then the password (SSH-2, SSH-3, SSH-5).

use std::sync::Arc;

use russh::client::Handle;
use russh_keys::key::KeyPair;
use serde_json::json;

use crate::error::{AppError, AppResult, ErrorKind, Layer};

use super::host_key::Client;
use super::{is_encrypted_key, KeySource, TunnelParams};

/// What reading one key gave.
enum Loaded {
    Key(Box<KeyPair>),
    /// Encrypted and no passphrase was given.
    NeedsPassphrase,
    /// Missing or unreadable key file: try the next one.
    Skip,
}

pub(super) async fn authenticate(handle: &mut Handle<Client>, p: &TunnelParams) -> AppResult<()> {
    let user = p.user.clone();
    let mut encrypted_without_passphrase: Option<String> = None;

    for source in &p.keys {
        let keypair = match load_key(source, p.passphrase.as_deref())? {
            Loaded::Key(k) => k,
            Loaded::NeedsPassphrase => {
                encrypted_without_passphrase.get_or_insert_with(|| source.name());
                continue;
            }
            Loaded::Skip => continue,
        };
        let ok = handle
            .authenticate_publickey(user.clone(), Arc::new(*keypair))
            .await
            .map_err(|e| auth_error(&format!("SSH key authentication failed: {e}")))?;
        if ok {
            return Ok(());
        }
    }

    if let Some(pw) = &p.password {
        let ok = handle
            .authenticate_password(user.clone(), pw.clone())
            .await
            .map_err(|e| auth_error(&format!("SSH password authentication failed: {e}")))?;
        if ok {
            return Ok(());
        }
    }

    if let Some(name) = encrypted_without_passphrase {
        return Err(AppError::new(
            ErrorKind::SshPassphraseRequired,
            Layer::Ssh,
            "Please enter the SSH private key passphrase.",
        )
        .data(json!({ "keyName": name })));
    }
    Err(auth_error(&format!(
        "SSH authentication failed for user \"{user}\"."
    )))
}

fn load_key(source: &KeySource, passphrase: Option<&str>) -> AppResult<Loaded> {
    let pem = match source {
        KeySource::Stored { pem, .. } => pem.clone(),
        KeySource::File(path) => match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return Ok(Loaded::Skip),
        },
    };
    match russh_keys::decode_secret_key(&pem, None) {
        Ok(k) => Ok(Loaded::Key(Box::new(k))),
        Err(e) if is_encrypted_key(&pem, &e) => match passphrase {
            Some(pass) => russh_keys::decode_secret_key(&pem, Some(pass))
                .map(|k| Loaded::Key(Box::new(k)))
                .map_err(|_| wrong_passphrase(source)),
            None => Ok(Loaded::NeedsPassphrase),
        },
        // A stored key must work; a default key file that doesn't parse is skipped.
        Err(e) => match source {
            KeySource::Stored { .. } => Err(AppError::new(
                ErrorKind::SshKeyInvalid,
                Layer::Ssh,
                format!("Can't read the private key: {e}"),
            )
            .hint("Import the private key file (not the .pub file) again.")),
            KeySource::File(_) => Ok(Loaded::Skip),
        },
    }
}

fn wrong_passphrase(source: &KeySource) -> AppError {
    AppError::new(
        ErrorKind::SshPassphraseWrong,
        Layer::Ssh,
        "The passphrase was wrong or the private key is not supported.",
    )
    .hint("Re-enter the passphrase.")
    .data(json!({ "keyName": source.name() }))
}

fn auth_error(msg: &str) -> AppError {
    AppError::new(ErrorKind::SshAuthFailed, Layer::Ssh, msg.to_string())
        .hint("Check the SSH user, password or private key. The server rejected the login.")
}
