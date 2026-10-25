//! Saving, duplicating and deleting connections and groups, and the keychain changes
//! a saved form asks for.

use crate::error::{AppError, AppResult};
use crate::models::{Connection, ConnectionView, Group, SecretChange, SecretsInput};
use crate::state::{now_ms, AppState};
use crate::storage::saved::SavedQueries;
use crate::storage::secrets::SecretKind;

use super::{disconnect, validate, view, ConnectionInput};

pub fn save(state: &AppState, input: ConnectionInput) -> AppResult<ConnectionView> {
    let mut c = input.connection;
    c.name = c.name.trim().to_string();
    c.group_id = c.group_id.filter(|g| !g.is_empty());
    validate(&c)?;
    {
        let mut cfg = state.config.lock().unwrap();
        if let Some(g) = &c.group_id {
            if !cfg.groups.iter().any(|x| &x.id == g) {
                return Err(AppError::validation("That group no longer exists."));
            }
        }
        stamp(&mut c, &cfg.connections)?;
        let mut next = cfg.connections.clone();
        match next.iter_mut().find(|x| x.id == c.id) {
            Some(slot) => *slot = c.clone(),
            None => next.push(c.clone()),
        }
        state.storage.save_connections(&next)?;
        cfg.connections = next;
    }
    apply_secrets(state, &c.id, &input.secrets)?;
    Ok(view(state, &c))
}

/// Gives a new connection its id and creation time, keeps the creation time of an
/// existing one, and marks it updated now.
fn stamp(c: &mut Connection, existing: &[Connection]) -> AppResult<()> {
    let now = now_ms();
    if c.id.is_empty() {
        c.id = uuid::Uuid::new_v4().to_string();
        c.created_at = now;
    } else if let Some(old) = existing.iter().find(|x| x.id == c.id) {
        c.created_at = old.created_at;
    } else {
        return Err(AppError::not_found("That connection no longer exists."));
    }
    c.updated_at = now;
    Ok(())
}

fn apply_secrets(state: &AppState, id: &str, s: &SecretsInput) -> AppResult<()> {
    let apply = |kind: SecretKind, change: &SecretChange| -> AppResult<()> {
        match change {
            SecretChange::Keep => Ok(()),
            SecretChange::Set { value } => state.secrets.set(id, kind, value),
            SecretChange::Clear => state.secrets.delete(id, kind),
        }
    };
    apply(SecretKind::DbPassword, &s.db_password)?;
    apply(SecretKind::SshPassword, &s.ssh_password)?;
    if s.clear_ssh_key {
        state.secrets.delete(id, SecretKind::SshKey)?;
        state.secrets.delete(id, SecretKind::SshKeyPassphrase)?;
    }
    if let Some(token) = &s.ssh_key_token {
        let key = state
            .imported_keys
            .lock()
            .unwrap()
            .remove(token)
            .ok_or_else(|| AppError::validation("The imported key expired; import it again."))?;
        state.secrets.set(id, SecretKind::SshKey, &key.pem)?;
        // A new key invalidates the old passphrase unless one was just entered.
        if matches!(s.ssh_key_passphrase, SecretChange::Keep) {
            state.secrets.delete(id, SecretKind::SshKeyPassphrase)?;
        }
    }
    apply(SecretKind::SshKeyPassphrase, &s.ssh_key_passphrase)?;
    Ok(())
}

/// Deletes the connection and everything kept for it: secrets, saved queries,
/// history and grid state (CON-9).
pub async fn delete(state: &AppState, id: &str) -> AppResult<()> {
    disconnect(state, id).await;
    {
        let mut cfg = state.config.lock().unwrap();
        let next: Vec<Connection> = cfg
            .connections
            .iter()
            .filter(|c| c.id != id)
            .cloned()
            .collect();
        if next.len() == cfg.connections.len() {
            return Err(AppError::not_found("That connection no longer exists."));
        }
        state.storage.save_connections(&next)?;
        cfg.connections = next;
    }
    state.secrets.delete_all(id)?;
    SavedQueries::new(state.storage.dir()).delete_connection(id)?;
    state.history.delete_connection(id).await?;
    {
        let mut ts = state.table_states.lock().unwrap();
        let prefix = format!("{id}/");
        ts.retain(|k, _| !k.starts_with(&prefix));
        state.storage.save_table_states(&ts)?;
    }
    Ok(())
}

pub fn duplicate(state: &AppState, id: &str, copy_secrets: bool) -> AppResult<ConnectionView> {
    let mut c = state.connection(id)?;
    let new_id = uuid::Uuid::new_v4().to_string();
    c.id = new_id.clone();
    c.name = format!("{} copy", c.name);
    let now = now_ms();
    c.created_at = now;
    c.updated_at = now;
    {
        let mut cfg = state.config.lock().unwrap();
        let mut next = cfg.connections.clone();
        next.push(c.clone());
        state.storage.save_connections(&next)?;
        cfg.connections = next;
    }
    if copy_secrets {
        state.secrets.copy_all(id, &new_id)?;
    }
    Ok(view(state, &c))
}

pub fn save_group(state: &AppState, mut g: Group) -> AppResult<Group> {
    g.name = g.name.trim().to_string();
    if g.name.is_empty() {
        return Err(AppError::validation("Give the group a name."));
    }
    g.parent_id = None; // one level in the MVP
    let mut cfg = state.config.lock().unwrap();
    if g.id.is_empty() {
        g.id = uuid::Uuid::new_v4().to_string();
    }
    let mut next = cfg.groups.clone();
    match next.iter_mut().find(|x| x.id == g.id) {
        Some(slot) => *slot = g.clone(),
        None => next.push(g.clone()),
    }
    state.storage.save_groups(&next)?;
    cfg.groups = next;
    Ok(g)
}

/// Deletes the group; its connections move to the top level.
pub fn delete_group(state: &AppState, id: &str) -> AppResult<()> {
    let mut cfg = state.config.lock().unwrap();
    let groups: Vec<Group> = cfg.groups.iter().filter(|g| g.id != id).cloned().collect();
    let conns: Vec<Connection> = cfg
        .connections
        .iter()
        .cloned()
        .map(|mut c| {
            if c.group_id.as_deref() == Some(id) {
                c.group_id = None;
            }
            c
        })
        .collect();
    state.storage.save_connections(&conns)?;
    state.storage.save_groups(&groups)?;
    cfg.groups = groups;
    cfg.connections = conns;
    Ok(())
}
