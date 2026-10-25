//! Secrets live only in the OS keychain (macOS Keychain, Windows Credential Manager,
//! Linux Secret Service). Service = bundle id, account = `<connectionId>_<kind>`.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::AppResult;

pub const SERVICE: &str = "com.libratable.app";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    DbPassword,
    SshPassword,
    SshKey,
    SshKeyPassphrase,
}

impl SecretKind {
    pub const ALL: [SecretKind; 4] = [
        SecretKind::DbPassword,
        SecretKind::SshPassword,
        SecretKind::SshKey,
        SecretKind::SshKeyPassphrase,
    ];

    pub fn suffix(self) -> &'static str {
        match self {
            SecretKind::DbPassword => "db_password",
            SecretKind::SshPassword => "ssh_password",
            SecretKind::SshKey => "ssh_key",
            SecretKind::SshKeyPassphrase => "ssh_key_passphrase",
        }
    }
}

pub fn account(connection_id: &str, kind: SecretKind) -> String {
    format!("{connection_id}_{}", kind.suffix())
}

pub enum SecretStore {
    Keychain,
    /// In-memory store for tests.
    Memory(Mutex<HashMap<String, String>>),
}

impl SecretStore {
    pub fn keychain() -> Self {
        SecretStore::Keychain
    }

    pub fn memory() -> Self {
        SecretStore::Memory(Mutex::new(HashMap::new()))
    }

    pub fn get(&self, connection_id: &str, kind: SecretKind) -> AppResult<Option<String>> {
        self.get_account(&account(connection_id, kind))
    }

    fn get_account(&self, acct: &str) -> AppResult<Option<String>> {
        match self {
            SecretStore::Keychain => match keyring::Entry::new(SERVICE, acct)?.get_password() {
                Ok(v) => Ok(Some(v)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(e.into()),
            },
            SecretStore::Memory(m) => Ok(m.lock().unwrap().get(acct).cloned()),
        }
    }

    pub fn has(&self, connection_id: &str, kind: SecretKind) -> bool {
        matches!(self.get(connection_id, kind), Ok(Some(_)))
    }

    pub fn set(&self, connection_id: &str, kind: SecretKind, value: &str) -> AppResult<()> {
        self.set_account(&account(connection_id, kind), value)
    }

    fn set_account(&self, acct: &str, value: &str) -> AppResult<()> {
        match self {
            SecretStore::Keychain => Ok(keyring::Entry::new(SERVICE, acct)?.set_password(value)?),
            SecretStore::Memory(m) => {
                m.lock()
                    .unwrap()
                    .insert(acct.to_string(), value.to_string());
                Ok(())
            }
        }
    }

    pub fn delete(&self, connection_id: &str, kind: SecretKind) -> AppResult<()> {
        self.delete_account(&account(connection_id, kind))
    }

    fn delete_account(&self, acct: &str) -> AppResult<()> {
        match self {
            SecretStore::Keychain => {
                match keyring::Entry::new(SERVICE, acct)?.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(e) => Err(e.into()),
                }
            }
            SecretStore::Memory(m) => {
                m.lock().unwrap().remove(acct);
                Ok(())
            }
        }
    }

    /// Removes every secret of a connection (CON-9: deleting a connection leaves nothing behind).
    pub fn delete_all(&self, connection_id: &str) -> AppResult<()> {
        for kind in SecretKind::ALL {
            self.delete(connection_id, kind)?;
        }
        Ok(())
    }

    /// App-wide secrets (the passcode hash), stored as account `app_<name>`.
    pub fn get_app(&self, name: &str) -> AppResult<Option<String>> {
        self.get_account(&format!("app_{name}"))
    }

    pub fn set_app(&self, name: &str, value: &str) -> AppResult<()> {
        self.set_account(&format!("app_{name}"), value)
    }

    pub fn delete_app(&self, name: &str) -> AppResult<()> {
        self.delete_account(&format!("app_{name}"))
    }

    pub fn copy_all(&self, from: &str, to: &str) -> AppResult<()> {
        for kind in SecretKind::ALL {
            if let Some(v) = self.get(from, kind)? {
                self.set(to, kind, &v)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip_and_delete_all() {
        let s = SecretStore::memory();
        s.set("c1", SecretKind::DbPassword, "").unwrap();
        s.set("c1", SecretKind::SshKey, "KEY").unwrap();
        assert_eq!(
            s.get("c1", SecretKind::DbPassword).unwrap(),
            Some(String::new())
        );
        assert!(s.has("c1", SecretKind::SshKey));
        s.copy_all("c1", "c2").unwrap();
        s.delete_all("c1").unwrap();
        assert!(!s.has("c1", SecretKind::SshKey));
        assert_eq!(
            s.get("c2", SecretKind::SshKey).unwrap().as_deref(),
            Some("KEY")
        );
        assert_eq!(
            account("abc", SecretKind::SshKeyPassphrase),
            "abc_ssh_key_passphrase"
        );
    }
}
