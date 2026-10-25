//! known_hosts maintenance for SSH-4 "Continue & update": drop the old entries for a
//! host (plain or hashed `|1|salt|hash`), then let russh-keys append the new key.

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::error::AppResult;

pub fn default_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("known_hosts"))
}

/// `host` for port 22, `[host]:port` otherwise — the form OpenSSH writes.
pub fn host_key(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn entry_matches(entry: &str, host_port: &str) -> bool {
    if let Some(rest) = entry.strip_prefix("|1|") {
        let mut parts = rest.split('|');
        let (Some(salt), Some(hash)) = (parts.next(), parts.next()) else {
            return false;
        };
        let (Ok(salt), Ok(hash)) = (B64.decode(salt), B64.decode(hash)) else {
            return false;
        };
        let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(&salt) else {
            return false;
        };
        mac.update(host_port.as_bytes());
        mac.verify_slice(&hash).is_ok()
    } else {
        entry == host_port
    }
}

fn line_hosts(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    let mut fields = t.split_whitespace();
    let mut hosts = fields.next()?;
    if hosts.starts_with('@') {
        hosts = fields.next()?;
    }
    Some(hosts)
}

/// True when known_hosts has any entry (of any key type) for `host:port`.
pub fn has_entry(path: &Path, host: &str, port: u16) -> bool {
    let target = host_key(host, port);
    std::fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter_map(line_hosts)
                .any(|hosts| hosts.split(',').any(|e| entry_matches(e, &target)))
        })
        .unwrap_or(false)
}

/// Removes every line for `host:port`. Returns how many lines were removed.
pub fn remove_host(path: &Path, host: &str, port: u16) -> AppResult<usize> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    let target = host_key(host, port);
    let mut removed = 0;
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            // Comments and blank lines stay; an optional @marker is skipped.
            let Some(hosts) = line_hosts(line) else {
                return true;
            };
            let hit = hosts.split(',').any(|e| entry_matches(e, &target));
            if hit {
                removed += 1;
            }
            !hit
        })
        .collect();
    if removed > 0 {
        let mut out = kept.join("\n");
        out.push('\n');
        crate::storage::atomic_write(path, out.as_bytes())?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_plain_and_hashed_entries_only_for_that_host() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        // Hashed entry for "[db.example.com]:2222".
        let salt = b"0123456789abcdefghij";
        let mut mac = Hmac::<Sha1>::new_from_slice(salt).unwrap();
        mac.update(b"[db.example.com]:2222");
        let hashed = format!(
            "|1|{}|{}",
            B64.encode(salt),
            B64.encode(mac.finalize().into_bytes())
        );
        let content = [
            "# comment",
            "[db.example.com]:2222 ssh-ed25519 AAAA1",
            "other.host ssh-ed25519 AAAA2",
            &format!("{hashed} ssh-rsa AAAA3"),
            "a,[db.example.com]:2222 ssh-ed25519 AAAA4",
            "",
        ]
        .join("\n");
        std::fs::write(&path, content).unwrap();
        assert!(has_entry(&path, "db.example.com", 2222));
        assert!(!has_entry(&path, "db.example.com", 22));
        assert_eq!(remove_host(&path, "db.example.com", 2222).unwrap(), 3);
        assert!(!has_entry(&path, "db.example.com", 2222));
        let left = std::fs::read_to_string(&path).unwrap();
        assert_eq!(left, "# comment\nother.host ssh-ed25519 AAAA2\n");
        assert_eq!(
            remove_host(&dir.path().join("missing"), "x", 22).unwrap(),
            0
        );
    }

    #[test]
    fn host_key_format() {
        assert_eq!(host_key("h", 22), "h");
        assert_eq!(host_key("h", 2200), "[h]:2200");
    }
}
