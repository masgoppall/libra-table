//! RNCryptor v3 with a password, the format of TablePlus' `.tableplusconnection` exports:
//! `03 01 | encryption salt (8) | HMAC salt (8) | IV (16) | AES-256-CBC ciphertext |
//! HMAC-SHA256 (32)`, both keys from PBKDF2-SHA1 with 10 000 rounds.

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::Sha256;

use crate::error::{AppError, AppResult};

const VERSION: u8 = 3;
const OPTIONS_PASSWORD: u8 = 1;
const ROUNDS: u32 = 10_000;
const SALT_LEN: usize = 8;
const IV_LEN: usize = 16;
const HMAC_LEN: usize = 32;
const HEADER_LEN: usize = 2 + 2 * SALT_LEN + IV_LEN;

/// Whether the bytes look like password-encrypted RNCryptor v3 data.
pub fn is_rncryptor(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER_LEN + IV_LEN + HMAC_LEN
        && bytes[0] == VERSION
        && bytes[1] == OPTIONS_PASSWORD
}

fn key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha1>(password.as_bytes(), salt, ROUNDS, &mut key);
    key
}

fn wrong_password() -> AppError {
    AppError::validation("The password is wrong, or the file is damaged.")
}

pub fn decrypt(password: &str, bytes: &[u8]) -> AppResult<Vec<u8>> {
    if !is_rncryptor(bytes) {
        return Err(AppError::validation(
            "This isn't an encrypted TablePlus export.",
        ));
    }
    let (signed, tag) = bytes.split_at(bytes.len() - HMAC_LEN);
    let enc_salt = &bytes[2..2 + SALT_LEN];
    let hmac_salt = &bytes[2 + SALT_LEN..2 + 2 * SALT_LEN];
    let iv = &bytes[2 + 2 * SALT_LEN..HEADER_LEN];

    let mut mac = Hmac::<Sha256>::new_from_slice(&key(password, hmac_salt))
        .map_err(|_| AppError::internal("HMAC key setup failed."))?;
    mac.update(signed);
    // Constant-time check before anything is decrypted.
    mac.verify_slice(tag).map_err(|_| wrong_password())?;

    let mut data = signed[HEADER_LEN..].to_vec();
    let plain = cbc::Decryptor::<aes::Aes256>::new(&key(password, enc_salt).into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut data)
        .map_err(|_| wrong_password())?
        .len();
    data.truncate(plain);
    Ok(data)
}

/// The writer side, for tests: TablePlus files are only ever read.
#[cfg(test)]
pub fn encrypt(password: &str, plaintext: &[u8], salts: [u8; 16], iv: [u8; 16]) -> Vec<u8> {
    use aes::cipher::BlockEncryptMut;
    let mut out = vec![VERSION, OPTIONS_PASSWORD];
    out.extend_from_slice(&salts);
    out.extend_from_slice(&iv);
    let mut buf = plaintext.to_vec();
    buf.resize(plaintext.len() + IV_LEN, 0);
    let n = cbc::Encryptor::<aes::Aes256>::new(&key(password, &salts[..8]).into(), &iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .unwrap()
        .len();
    out.extend_from_slice(&buf[..n]);
    let mut mac = Hmac::<Sha256>::new_from_slice(&key(password, &salts[8..])).unwrap();
    mac.update(&out);
    out.extend_from_slice(&mac.finalize().into_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_wrong_password() {
        let sealed = encrypt("s3cret", b"[{\"ID\":\"1\"}]", [7; 16], [9; 16]);
        assert!(is_rncryptor(&sealed));
        assert_eq!(decrypt("s3cret", &sealed).unwrap(), b"[{\"ID\":\"1\"}]");
        assert!(decrypt("nope", &sealed).is_err());
        let mut tampered = sealed.clone();
        tampered[40] ^= 1;
        assert!(decrypt("s3cret", &tampered).is_err());
        assert!(!is_rncryptor(b"{\"format\":\"libratable-connections\"}"));
    }
}
