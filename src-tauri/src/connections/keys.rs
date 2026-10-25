//! Importing an SSH private key file chosen in the form (SSH-2).

use std::path::Path;

use crate::error::{AppError, AppResult, ErrorKind, Layer};
use crate::ssh;
use crate::state::{AppState, ImportedKey};

use super::ImportedKeyInfo;

const PUBLIC_KEY_CHOSEN: &str = "You selected the public key (.pub). Choose the private key \
    file, which usually has no extension.";

fn key_error(message: impl Into<String>) -> AppError {
    AppError::new(ErrorKind::SshKeyInvalid, Layer::Ssh, message)
}

/// Reads the key. It stays in Rust behind a token until the form is saved.
pub fn import_key(state: &AppState, path: &str) -> AppResult<ImportedKeyInfo> {
    let p = Path::new(path);
    if p.extension().and_then(|e| e.to_str()) == Some("pub") {
        return Err(key_error(PUBLIC_KEY_CHOSEN));
    }
    let pem = std::fs::read_to_string(p)
        .map_err(|e| key_error(format!("Can't read the private key: {e}")))?;
    let encrypted = is_encrypted(&pem)?;
    let token = uuid::Uuid::new_v4().to_string();
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "key".into());
    state.imported_keys.lock().unwrap().insert(
        token.clone(),
        ImportedKey {
            name: name.clone(),
            pem,
        },
    );
    Ok(ImportedKeyInfo {
        token,
        name,
        encrypted,
    })
}

/// Whether the key needs a passphrase; an error when it isn't a key we can use.
fn is_encrypted(pem: &str) -> AppResult<bool> {
    match russh_keys::decode_secret_key(pem, None) {
        Ok(_) => Ok(false),
        Err(e) if ssh::is_encrypted_key(pem, &e) => Ok(true),
        Err(e) => Err(
            key_error(format!("This is not a supported private key: {e}"))
                .hint("Use an OpenSSH or PEM private key (ed25519, ECDSA or RSA)."),
        ),
    }
}
