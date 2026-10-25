//! Minimal `~/.ssh/config` reader for SSH-3: HostName, User, Port and IdentityFile,
//! with OpenSSH's first-match-wins rule and `*` / `?` / `!` host patterns.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostConfig {
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_files: Vec<PathBuf>,
}

pub fn default_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}

pub fn load(alias: &str) -> HostConfig {
    match default_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(text) => resolve(&text, alias, dirs::home_dir().as_deref()),
        None => HostConfig::default(),
    }
}

/// Resolves the settings for `alias` from config `text`.
pub fn resolve(text: &str, alias: &str, home: Option<&Path>) -> HostConfig {
    let mut cfg = HostConfig::default();
    let mut active = true; // lines before the first Host apply to every host
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = match split_kv(line) {
            Some(kv) => kv,
            None => continue,
        };
        let key = key.to_ascii_lowercase();
        if key == "host" {
            active = host_matches(&value, alias);
            continue;
        }
        if key == "match" {
            // Match blocks need runtime evaluation we do not support; skip them.
            active = false;
            continue;
        }
        if !active {
            continue;
        }
        let value = unquote(&value);
        match key.as_str() {
            "hostname" if cfg.host_name.is_none() => {
                cfg.host_name = Some(value.replace("%h", alias))
            }
            "user" if cfg.user.is_none() => cfg.user = Some(value),
            "port" if cfg.port.is_none() => cfg.port = value.parse().ok(),
            "identityfile" => cfg.identity_files.push(expand(&value, home)),
            _ => {}
        }
    }
    cfg
}

fn split_kv(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let idx = line.find(|c: char| c.is_whitespace() || c == '=')?;
    let key = line[..idx].to_string();
    let rest = line[idx..]
        .trim_start_matches(|c: char| c.is_whitespace() || c == '=')
        .trim();
    Some((key, rest.to_string()))
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

fn expand(path: &str, home: Option<&Path>) -> PathBuf {
    let path = unquote(path);
    match (path.strip_prefix("~/"), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => PathBuf::from(path),
    }
}

fn host_matches(patterns: &str, alias: &str) -> bool {
    let mut matched = false;
    for pat in patterns.split_whitespace() {
        let (neg, pat) = match pat.strip_prefix('!') {
            Some(p) => (true, p),
            None => (false, pat),
        };
        if glob(pat, alias) {
            if neg {
                return false;
            }
            matched = true;
        }
    }
    matched
}

fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    fn rec(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            (Some('?'), Some(_)) => rec(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    rec(&p, &t)
}

/// Keys OpenSSH tries by default when nothing else is configured.
pub fn default_identities() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    ["id_ed25519", "id_ecdsa", "id_rsa"]
        .iter()
        .map(|n| home.join(".ssh").join(n))
        .filter(|p| p.exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = r#"
# global
IdentityFile ~/.ssh/global_key

Host bastion prod-*
    HostName bastion.example.com
    User deploy
    Port 2222
    IdentityFile "~/.ssh/deploy key"

Host *.internal !secret.internal
    User ops

Host *
    User fallback
    Port 22
"#;

    #[test]
    fn first_match_wins_and_identity_files_accumulate() {
        let home = Path::new("/home/me");
        let c = resolve(CFG, "bastion", Some(home));
        assert_eq!(c.host_name.as_deref(), Some("bastion.example.com"));
        assert_eq!(c.user.as_deref(), Some("deploy"));
        assert_eq!(c.port, Some(2222));
        assert_eq!(
            c.identity_files,
            vec![home.join(".ssh/global_key"), home.join(".ssh/deploy key")]
        );
    }

    #[test]
    fn wildcards_and_negation() {
        assert_eq!(
            resolve(CFG, "prod-db1", None).user.as_deref(),
            Some("deploy")
        );
        assert_eq!(
            resolve(CFG, "db.internal", None).user.as_deref(),
            Some("ops")
        );
        assert_eq!(
            resolve(CFG, "secret.internal", None).user.as_deref(),
            Some("fallback")
        );
        let other = resolve(CFG, "example.org", None);
        assert_eq!(other.host_name, None);
        assert_eq!(other.port, Some(22));
    }
}
