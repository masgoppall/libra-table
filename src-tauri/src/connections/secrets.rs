//! Picks the secrets for one connect or test attempt: values typed into the form
//! (test), the password prompt, or the keychain.

use serde_json::json;

use crate::error::{AppError, AppResult, ErrorKind, Layer};
use crate::models::{Connection, PasswordMode, PromptedSecrets, SecretChange, SecretsInput};
use crate::state::AppState;
use crate::storage::secrets::SecretKind;

pub(super) struct Resolved {
    pub db_password: Option<String>,
    pub ssh_password: Option<String>,
    /// (display name, PEM)
    pub ssh_key: Option<(String, String)>,
    pub ssh_passphrase: Option<String>,
}

/// Where one secret comes from when the form doesn't set it.
struct Sources<'a> {
    state: &'a AppState,
    connection: &'a Connection,
    form: &'a SecretsInput,
    prompted: &'a PromptedSecrets,
}

impl Sources<'_> {
    /// The keychain value; a connection that was never saved has none.
    fn stored(&self, kind: SecretKind) -> AppResult<Option<String>> {
        if self.connection.id.is_empty() {
            Ok(None)
        } else {
            self.state.secrets.get(&self.connection.id, kind)
        }
    }

    fn chosen(&self, change: &SecretChange, kind: SecretKind) -> AppResult<Option<String>> {
        match change {
            SecretChange::Set { value } => Ok(Some(value.clone())),
            SecretChange::Clear => Ok(None),
            SecretChange::Keep => self.stored(kind),
        }
    }

    /// A password in one of the three modes; "Ask every time" needs the prompt or the
    /// form to have it, or the UI is asked to prompt (`which` names the prompt).
    fn password(
        &self,
        mode: PasswordMode,
        form: &SecretChange,
        prompted: &Option<String>,
        kind: SecretKind,
        ask: (Layer, &str, &str),
    ) -> AppResult<Option<String>> {
        match mode {
            PasswordMode::None => Ok(None),
            PasswordMode::Keychain => self.chosen(form, kind),
            PasswordMode::Ask => match (prompted, form) {
                (Some(p), _) => Ok(Some(p.clone())),
                (None, SecretChange::Set { value }) => Ok(Some(value.clone())),
                _ => {
                    let (layer, message, which) = ask;
                    let data = json!({ "which": which, "connectionName": self.connection.name });
                    Err(AppError::new(ErrorKind::PasswordRequired, layer, message).data(data))
                }
            },
        }
    }

    /// The imported key waiting in the form, or the key saved for the connection.
    fn ssh_key(&self) -> AppResult<Option<(String, String)>> {
        let token_key = self.form.ssh_key_token.as_ref().and_then(|t| {
            self.state
                .imported_keys
                .lock()
                .unwrap()
                .get(t)
                .map(|k| (k.name.clone(), k.pem.clone()))
        });
        Ok(match token_key {
            Some(k) => Some(k),
            None if self.form.clear_ssh_key => None,
            None => self
                .stored(SecretKind::SshKey)?
                .map(|pem| (self.connection.ssh.key_name.clone(), pem)),
        })
    }

    fn ssh_passphrase(&self) -> AppResult<Option<String>> {
        match &self.prompted.ssh_key_passphrase {
            Some(p) => Ok(Some(p.clone())),
            None => self.chosen(&self.form.ssh_key_passphrase, SecretKind::SshKeyPassphrase),
        }
    }
}

pub(super) fn resolve(
    state: &AppState,
    c: &Connection,
    form: Option<&SecretsInput>,
    prompted: &PromptedSecrets,
) -> AppResult<Resolved> {
    let empty = SecretsInput::default();
    let src = Sources {
        state,
        connection: c,
        form: form.unwrap_or(&empty),
        prompted,
    };
    let db_password = src.password(
        c.db.password_mode,
        &src.form.db_password,
        &prompted.db_password,
        SecretKind::DbPassword,
        (Layer::Database, "Enter the database password.", "database"),
    )?;
    let mut out = Resolved {
        db_password,
        ssh_password: None,
        ssh_key: None,
        ssh_passphrase: None,
    };
    if c.ssh.enabled {
        out.ssh_password = src.password(
            c.ssh.password_mode,
            &src.form.ssh_password,
            &prompted.ssh_password,
            SecretKind::SshPassword,
            (Layer::Ssh, "Enter the SSH password.", "ssh"),
        )?;
        if c.ssh.use_key {
            out.ssh_key = src.ssh_key()?;
        }
        out.ssh_passphrase = src.ssh_passphrase()?;
    }
    Ok(out)
}
