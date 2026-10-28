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
    let mut cfg = state.config.lock().unwrap();
    if g.id.is_empty() {
        g.id = uuid::Uuid::new_v4().to_string();
    }
    g.parent_id = g.parent_id.filter(|p| !p.is_empty());
    g.icon = g.icon.filter(|i| !i.is_empty());
    if let Some(icon) = &g.icon {
        check_icon(icon)?;
    }
    if let Some(parent) = &g.parent_id {
        check_parent(&cfg.groups, &g.id, parent)?;
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

/// Largest group icon kept in groups.json (the dialog stores a 96 px PNG, ~10 KB).
const MAX_ICON_LEN: usize = 256 * 1024;

fn check_icon(icon: &str) -> AppResult<()> {
    if !icon.starts_with("data:image/") {
        return Err(AppError::validation("The group icon must be an image."));
    }
    if icon.len() > MAX_ICON_LEN {
        return Err(AppError::validation("The group icon is too large."));
    }
    Ok(())
}

/// Deepest nesting of groups the list shows.
const MAX_GROUP_DEPTH: usize = 8;

/// A group can't sit in a missing group, in itself or in one of its own subgroups.
fn check_parent(groups: &[Group], id: &str, parent: &str) -> AppResult<()> {
    let mut depth = 1;
    let mut at = Some(parent.to_string());
    while let Some(current) = at {
        if current == id {
            return Err(AppError::validation("A group can't be moved into itself."));
        }
        let g = groups
            .iter()
            .find(|g| g.id == current)
            .ok_or_else(|| AppError::validation("The parent group no longer exists."))?;
        depth += 1;
        if depth > MAX_GROUP_DEPTH {
            return Err(AppError::validation("Groups can be nested 8 levels deep."));
        }
        at = g.parent_id.clone();
    }
    Ok(())
}

/// Deletes the group; its subgroups and connections move up to its parent.
pub fn delete_group(state: &AppState, id: &str) -> AppResult<()> {
    let mut cfg = state.config.lock().unwrap();
    let parent = cfg
        .groups
        .iter()
        .find(|g| g.id == id)
        .and_then(|g| g.parent_id.clone());
    let groups: Vec<Group> = cfg
        .groups
        .iter()
        .filter(|g| g.id != id)
        .cloned()
        .map(|mut g| {
            if g.parent_id.as_deref() == Some(id) {
                g.parent_id = parent.clone();
            }
            g
        })
        .collect();
    let conns: Vec<Connection> = cfg
        .connections
        .iter()
        .cloned()
        .map(|mut c| {
            if c.group_id.as_deref() == Some(id) {
                c.group_id = parent.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, parent: Option<&str>) -> Group {
        Group {
            id: id.into(),
            name: id.into(),
            parent_id: parent.map(str::to_string),
            collapsed: false,
            icon: None,
        }
    }

    #[test]
    fn groups_nest_but_never_into_themselves() {
        let groups = [
            group("a", None),
            group("b", Some("a")),
            group("c", Some("b")),
        ];
        assert!(check_parent(&groups, "new", "c").is_ok());
        assert!(check_parent(&groups, "a", "c").is_err(), "a cycle");
        assert!(check_parent(&groups, "b", "b").is_err(), "itself");
        assert!(check_parent(&groups, "new", "missing").is_err());
    }

    #[test]
    fn group_icons_are_small_images() {
        assert!(check_icon("data:image/png;base64,iVBORw0KGgo=").is_ok());
        assert!(check_icon("https://example.com/icon.png").is_err());
        let big = format!("data:image/png;base64,{}", "A".repeat(MAX_ICON_LEN));
        assert!(check_icon(&big).is_err());
    }
}
