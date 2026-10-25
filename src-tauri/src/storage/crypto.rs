//! Passphrase-based encryption for exported connection secrets (AES-256-GCM with a
//! PBKDF2-SHA256 key) and the app passcode hash. Nothing here touches the disk.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::error::{AppError, AppResult};

/// OWASP's guidance for PBKDF2-HMAC-SHA256 is in this range; it costs ~0.2 s.
const ITERATIONS: u32 = 310_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Encrypted bytes with what is needed to decrypt them, given the passphrase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sealed {
    pub iterations: u32,
    pub salt: String,
    pub nonce: String,
    pub data: String,
}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn derive(passphrase: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, iterations, &mut key);
    key
}

fn decode(field: &str) -> AppResult<Vec<u8>> {
    B64.decode(field)
        .map_err(|_| AppError::validation("The file is damaged (bad encoding)."))
}

pub fn seal(passphrase: &str, plaintext: &[u8]) -> AppResult<Sealed> {
    let salt = random::<SALT_LEN>();
    let nonce = random::<NONCE_LEN>();
    let key = derive(passphrase, &salt, ITERATIONS);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let data = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| AppError::internal("Encryption failed."))?;
    Ok(Sealed {
        iterations: ITERATIONS,
        salt: B64.encode(salt),
        nonce: B64.encode(nonce),
        data: B64.encode(data),
    })
}

/// Fails with a validation error when the passphrase is wrong (or the data altered).
pub fn open(passphrase: &str, sealed: &Sealed) -> AppResult<Vec<u8>> {
    let salt = decode(&sealed.salt)?;
    let nonce = decode(&sealed.nonce)?;
    if nonce.len() != NONCE_LEN {
        return Err(AppError::validation("The file is damaged (bad nonce)."));
    }
    let key = derive(passphrase, &salt, sealed.iterations);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(&nonce), decode(&sealed.data)?.as_ref())
        .map_err(|_| AppError::validation("Wrong passphrase."))
}

/// `pbkdf2-sha256$<iterations>$<salt>$<hash>`, for the app passcode.
pub fn hash_secret(secret: &str) -> String {
    let salt = random::<SALT_LEN>();
    let hash = derive(secret, &salt, ITERATIONS);
    format!(
        "pbkdf2-sha256${ITERATIONS}${}${}",
        B64.encode(salt),
        B64.encode(hash)
    )
}

pub fn verify_secret(secret: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    let [_, iterations, salt, hash] = parts.as_slice() else {
        return false;
    };
    let (Ok(iterations), Ok(salt), Ok(hash)) = (
        iterations.parse::<u32>(),
        B64.decode(salt),
        B64.decode(hash),
    ) else {
        return false;
    };
    derive(secret, &salt, iterations)
        .ct_eq(hash.as_slice())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_and_open_round_trip() {
        let sealed = seal("correct horse", b"secret bytes").unwrap();
        assert_eq!(open("correct horse", &sealed).unwrap(), b"secret bytes");
        let err = open("wrong", &sealed).unwrap_err();
        assert_eq!(err.message, "Wrong passphrase.");
    }

    #[test]
    fn hashes_verify_only_the_same_secret() {
        let stored = hash_secret("1234");
        assert!(verify_secret("1234", &stored));
        assert!(!verify_secret("1235", &stored));
        assert!(!verify_secret("1234", "garbage"));
    }
}
