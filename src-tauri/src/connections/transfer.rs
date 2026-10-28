//! Moving connections between machines (TablePlus "Export/Import connections"): a JSON
//! file of connections and groups. Secrets are included only when the user gives a
//! passphrase, and then only encrypted (`storage::crypto`). Files exported by TablePlus
//! itself are read too (`super::tableplus`).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::models::{Connection, Group};
use crate::state::{now_ms, AppState};
use crate::storage::crypto::{self, Sealed};
use crate::storage::secrets::SecretKind;

use super::{tableplus, validate};

const FORMAT: &str = "libratable-connections";
const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportFile {
    format: String,
    version: u32,
    exported_at: i64,
    groups: Vec<Group>,
    connections: Vec<Connection>,
    /// Connection id → secret kind → value, encrypted as one JSON document.
    secrets: Option<Sealed>,
}

type SecretBundle = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub names: Vec<String>,
    pub has_secrets: bool,
    /// "libratable" or "tableplus".
    pub source: &'static str,
    /// Everything is encrypted: the names are known only after the password is given.
    pub locked: bool,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: usize,
    pub skipped: Vec<Skipped>,
}

impl ImportReport {
    pub fn skip(&mut self, name: String, reason: String) {
        self.skipped.push(Skipped { name, reason });
    }
}

/// A connection to add, with the ids of the file it came from and its secrets.
pub struct NewConnection {
    pub connection: Connection,
    pub secrets: Vec<(SecretKind, String)>,
}

fn collect_secrets(state: &AppState, conns: &[Connection]) -> AppResult<SecretBundle> {
    let mut bundle = SecretBundle::new();
    for c in conns {
        let mut kinds = BTreeMap::new();
        for kind in SecretKind::ALL {
            if let Some(v) = state.secrets.get(&c.id, kind)? {
                kinds.insert(kind.suffix().to_string(), v);
            }
        }
        if !kinds.is_empty() {
            bundle.insert(c.id.clone(), kinds);
        }
    }
    Ok(bundle)
}

/// Writes the chosen connections (all when `ids` is empty) and their groups.
pub fn export(
    state: &AppState,
    ids: &[String],
    path: &Path,
    passphrase: Option<&str>,
) -> AppResult<usize> {
    let (conns, groups) = {
        let cfg = state.config.lock().unwrap();
        let conns: Vec<Connection> = cfg
            .connections
            .iter()
            .filter(|c| ids.is_empty() || ids.contains(&c.id))
            .cloned()
            .collect();
        let used = |g: &&Group| conns.iter().any(|c| c.group_id.as_deref() == Some(&g.id));
        (
            conns.clone(),
            cfg.groups.iter().filter(used).cloned().collect(),
        )
    };
    let secrets = match passphrase.filter(|p| !p.is_empty()) {
        Some(p) => Some(crypto::seal(
            p,
            &serde_json::to_vec(&collect_secrets(state, &conns)?)?,
        )?),
        None => None,
    };
    let file = ExportFile {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: now_ms(),
        groups,
        connections: conns,
        secrets,
    };
    crate::storage::atomic_write(path, &serde_json::to_vec_pretty(&file)?)?;
    Ok(file.connections.len())
}

fn parse(bytes: &[u8]) -> AppResult<ExportFile> {
    let file: ExportFile = serde_json::from_slice(bytes)
        .map_err(|_| AppError::validation("This isn't a LibraTable connections file."))?;
    if file.format != FORMAT || file.version > VERSION {
        return Err(AppError::validation(
            "This connections file was made by a newer or different app.",
        ));
    }
    Ok(file)
}

pub fn inspect(path: &Path) -> AppResult<FileSummary> {
    let bytes = std::fs::read(path)?;
    if tableplus::is_export(&bytes) {
        return Ok(FileSummary {
            names: Vec::new(),
            has_secrets: true,
            source: "tableplus",
            locked: true,
        });
    }
    let file = parse(&bytes)?;
    Ok(FileSummary {
        names: file.connections.iter().map(|c| c.name.clone()).collect(),
        has_secrets: file.secrets.is_some(),
        source: "libratable",
        locked: false,
    })
}

/// A group with the same name in the same parent is reused; others are created.
/// Returns file id → LibraTable id.
pub(super) fn merge_groups(
    state: &AppState,
    groups: &[Group],
) -> AppResult<HashMap<String, String>> {
    let mut map = HashMap::new();
    for g in parents_first(groups) {
        let parent_id = g.parent_id.as_ref().and_then(|p| map.get(p).cloned());
        let existing = {
            let cfg = state.config.lock().unwrap();
            cfg.groups
                .iter()
                .find(|x| x.name.eq_ignore_ascii_case(&g.name) && x.parent_id == parent_id)
                .cloned()
        };
        let id = match existing {
            Some(x) => x.id,
            None => {
                super::save_group(
                    state,
                    Group {
                        id: String::new(),
                        parent_id,
                        ..g.clone()
                    },
                )?
                .id
            }
        };
        map.insert(g.id.clone(), id);
    }
    Ok(map)
}

/// Groups ordered so every parent comes before its subgroups (unknown parents count
/// as top level).
fn parents_first(groups: &[Group]) -> Vec<&Group> {
    let depth = |g: &Group| {
        let mut d = 0;
        let mut at = g.parent_id.as_deref();
        while let Some(p) = at.filter(|_| d < groups.len()) {
            d += 1;
            at = groups
                .iter()
                .find(|x| x.id == p)
                .and_then(|x| x.parent_id.as_deref());
        }
        d
    };
    let mut ordered: Vec<&Group> = groups.iter().collect();
    ordered.sort_by_key(|g| depth(g));
    ordered
}

fn open_secrets(file: &ExportFile, passphrase: Option<&str>) -> AppResult<SecretBundle> {
    match (&file.secrets, passphrase.filter(|p| !p.is_empty())) {
        (Some(sealed), Some(p)) => Ok(serde_json::from_slice(&crypto::open(p, sealed)?)?),
        // Without the passphrase the connections come in without their secrets.
        _ => Ok(SecretBundle::new()),
    }
}

/// Adds every connection of the file under a new id. A TablePlus export needs the
/// password it was exported with.
pub fn import(state: &AppState, path: &Path, passphrase: Option<&str>) -> AppResult<ImportReport> {
    let bytes = std::fs::read(path)?;
    if tableplus::is_export(&bytes) {
        let password = passphrase.filter(|p| !p.is_empty()).ok_or_else(|| {
            AppError::validation("Enter the password the connections were exported with.")
        })?;
        return tableplus::import(state, &bytes, password);
    }
    let file = parse(&bytes)?;
    let secrets = open_secrets(&file, passphrase)?;
    file.connections.iter().try_for_each(validate)?;
    let groups = merge_groups(state, &file.groups)?;
    let fresh = file
        .connections
        .iter()
        .map(|c| NewConnection {
            connection: c.clone(),
            secrets: secret_list(secrets.get(&c.id)),
        })
        .collect();
    Ok(ImportReport {
        added: add(state, fresh, &groups)?,
        skipped: Vec::new(),
    })
}

/// Saves the connections under new ids, in the groups `groups` maps them to, with their
/// secrets in the keychain. Returns how many were added.
pub(super) fn add(
    state: &AppState,
    fresh: Vec<NewConnection>,
    groups: &HashMap<String, String>,
) -> AppResult<usize> {
    let now = now_ms();
    let mut added = Vec::new();
    for NewConnection {
        connection: mut n,
        secrets,
    } in fresh
    {
        n.id = uuid::Uuid::new_v4().to_string();
        n.group_id = n.group_id.as_ref().and_then(|g| groups.get(g).cloned());
        (n.created_at, n.updated_at) = (now, now);
        for (kind, value) in &secrets {
            state.secrets.set(&n.id, *kind, value)?;
        }
        added.push(n);
    }
    let mut cfg = state.config.lock().unwrap();
    let mut next = cfg.connections.clone();
    next.extend(added.iter().cloned());
    state.storage.save_connections(&next)?;
    cfg.connections = next;
    Ok(added.len())
}

fn secret_list(kinds: Option<&BTreeMap<String, String>>) -> Vec<(SecretKind, String)> {
    let Some(kinds) = kinds else {
        return Vec::new();
    };
    SecretKind::ALL
        .into_iter()
        .filter_map(|kind| kinds.get(kind.suffix()).map(|v| (kind, v.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_state;

    fn connection(name: &str) -> Connection {
        serde_json::from_value(serde_json::json!({
            "id": "", "name": name, "driver": "postgres", "groupId": null
        }))
        .unwrap()
    }

    async fn source(dir: &Path) -> (AppState, String) {
        let state = test_state(dir).await;
        let g = super::super::save_group(
            &state,
            Group {
                id: String::new(),
                name: "Clients".into(),
                parent_id: None,
                collapsed: false,
                icon: None,
            },
        )
        .unwrap();
        let mut c = connection("Shop");
        c.group_id = Some(g.id);
        let input = serde_json::from_value(serde_json::json!({
            "connection": c,
            "secrets": {
                "dbPassword": { "action": "set", "value": "pw-1" },
                "sshPassword": { "action": "keep" },
                "sshKeyToken": null,
                "clearSshKey": false,
                "sshKeyPassphrase": { "action": "keep" }
            }
        }))
        .unwrap();
        let v = super::super::save(&state, input).unwrap();
        (state, v.connection.id)
    }

    #[tokio::test]
    async fn round_trip_with_encrypted_secrets() {
        let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (a, _) = source(a_dir.path()).await;
        let file = a_dir.path().join("conns.json");
        assert_eq!(export(&a, &[], &file, Some("open sesame")).unwrap(), 1);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            !text.contains("pw-1"),
            "secrets must not be written in clear"
        );
        assert!(inspect(&file).unwrap().has_secrets);

        let b = test_state(b_dir.path()).await;
        assert!(import(&b, &file, Some("nope")).is_err());
        assert_eq!(import(&b, &file, Some("open sesame")).unwrap().added, 1);
        let imported = b.config.lock().unwrap().connections[0].clone();
        let group = b.config.lock().unwrap().groups[0].clone();
        assert_eq!(
            (imported.name.as_str(), group.name.as_str()),
            ("Shop", "Clients")
        );
        assert_eq!(imported.group_id, Some(group.id));
        let pw = b.secrets.get(&imported.id, SecretKind::DbPassword).unwrap();
        assert_eq!(pw.as_deref(), Some("pw-1"));
    }

    #[tokio::test]
    async fn without_passphrase_secrets_stay_behind() {
        let dir = tempfile::tempdir().unwrap();
        let (a, id) = source(dir.path()).await;
        let file = dir.path().join("plain.json");
        export(&a, &[id], &file, None).unwrap();
        assert!(!inspect(&file).unwrap().has_secrets);
        assert_eq!(import(&a, &file, None).unwrap().added, 1);
        let conns = a.config.lock().unwrap().connections.clone();
        assert_eq!(conns.len(), 2);
        assert!(!a.secrets.has(&conns[1].id, SecretKind::DbPassword));
        // The existing group is reused, not duplicated.
        assert_eq!(a.config.lock().unwrap().groups.len(), 1);
    }
}
