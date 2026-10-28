//! Migrating from TablePlus: its "Export connections" file (`.tableplusconnection`) holds
//! the connections and their groups, passwords included, encrypted with the password
//! chosen at export (RNCryptor). Connections already in LibraTable are left alone, so importing twice
//! adds nothing.

mod convert;
mod payload;

use std::path::PathBuf;

use crate::error::AppResult;
use crate::models::{Connection, Group};
use crate::state::AppState;
use crate::storage::rncryptor;

use super::transfer::{self, ImportReport, NewConnection};
use convert::Entry;
use payload::{plist_to_json, Dict};

pub use rncryptor::is_rncryptor as is_export;

/// The groups of TablePlus on this Mac, for exports that only carry group ids.
fn local_groups() -> Vec<Dict> {
    let path: Option<PathBuf> =
        dirs::data_dir().map(|d| d.join("com.tinyapp.TablePlus/Data/ConnectionGroups.plist"));
    match path
        .and_then(|p| plist::Value::from_file(p).ok())
        .map(plist_to_json)
    {
        Some(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|i| i.as_object().cloned())
            .collect(),
        _ => Vec::new(),
    }
}

/// The same server, user and database under the same name: already migrated.
fn same(a: &Connection, b: &Connection) -> bool {
    a.driver == b.driver
        && a.name == b.name
        && a.summary() == b.summary()
        && a.db.user == b.db.user
        && a.ssh.enabled == b.ssh.enabled
        && a.ssh.host == b.ssh.host
}

pub fn import(state: &AppState, bytes: &[u8], password: &str) -> AppResult<ImportReport> {
    let doc = payload::read(&rncryptor::decrypt(password, bytes)?)?;
    let mut report = ImportReport::default();
    let mut found = Vec::new();
    for raw in &doc.connections {
        let e = Entry(raw);
        match convert::connection(&e) {
            Ok(c) => found.push(c),
            Err(reason) => report.skip(e.text("ConnectionName"), reason),
        }
    }
    let used = |g: &Group| {
        found
            .iter()
            .any(|c| c.connection.group_id.as_ref() == Some(&g.id))
    };
    let raw_groups =
        if doc.groups.is_empty() && found.iter().any(|c| c.connection.group_id.is_some()) {
            local_groups()
        } else {
            doc.groups
        };
    let all: Vec<Group> = raw_groups
        .iter()
        .filter_map(|g| convert::group(&Entry(g)))
        .collect();
    // Only the groups (and their parents) that hold something that comes over.
    let groups: Vec<Group> = all
        .iter()
        .filter(|g| used(g) || holds_used(&all, g, &used, all.len()))
        .cloned()
        .collect();

    let existing = state.config.lock().unwrap().connections.clone();
    let mut fresh = Vec::new();
    for c in found {
        if existing.iter().any(|x| same(x, &c.connection)) {
            report.skip(c.connection.name.clone(), "already in LibraTable".into());
            continue;
        }
        if let Err(e) = super::validate(&c.connection) {
            report.skip(c.connection.name.clone(), e.message.clone());
            continue;
        }
        fresh.push(NewConnection {
            connection: c.connection,
            secrets: c.secrets,
        });
    }
    let ids = transfer::merge_groups(state, &groups)?;
    report.added = transfer::add(state, fresh, &ids)?;
    Ok(report)
}

/// Whether a subgroup of `g` holds a connection that comes over (`depth` stops cycles).
fn holds_used(all: &[Group], g: &Group, used: &dyn Fn(&Group) -> bool, depth: usize) -> bool {
    depth > 0
        && all
            .iter()
            .filter(|c| c.parent_id.as_deref() == Some(&g.id))
            .any(|c| used(c) || holds_used(all, c, used, depth - 1))
}

#[cfg(test)]
mod tests;
