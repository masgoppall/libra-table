//! Opening and closing sessions: the SSH tunnel (if any), the pool, connect,
//! disconnect, test and trusting a new host key.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::db::{self, DbPool, Target};
use crate::error::{AppError, AppResult};
use crate::models::{Connection, Driver, PromptedSecrets};
use crate::ssh::{self, KeySource, OpenError, Tunnel, TunnelParams};
use crate::state::{AppState, Session};
use crate::storage::secrets::SecretKind;

use super::secrets::{resolve, Resolved};
use super::{validate, ConnectInfo, ConnectionInput, TestResult, Warning};

/// Pool size of a connection's own session.
const SESSION_POOL_SIZE: u32 = 6;

/// Shortest keep-alive interval, whatever the setting says.
const MIN_KEEP_ALIVE_SECS: u32 = 5;

const SSH_HOST_SAME_AS_DB: &str = "You're using an SSH tunnel, but the database host is the \
    same as the SSH server. If the database runs on that server, use 127.0.0.1 as the \
    database host.";

pub async fn connect(
    state: &AppState,
    id: &str,
    prompted: PromptedSecrets,
) -> AppResult<ConnectInfo> {
    if let Ok(s) = state.session(id).await {
        return Ok(info(&s));
    }
    let c = state.connection(id)?;
    let secrets = resolve(state, &c, None, &prompted)?;
    let (pool, tunnel) = open(state, &c, &secrets, SESSION_POOL_SIZE).await?;
    if prompted.save_passphrase {
        if let Some(p) = &prompted.ssh_key_passphrase {
            state.secrets.set(id, SecretKind::SshKeyPassphrase, p)?;
        }
    }
    let version = pool.server_version().await.unwrap_or_default();
    let database = match c.driver {
        Driver::Sqlite => "main".to_string(),
        _ => current_database(&pool)
            .await
            .unwrap_or_else(|_| c.db.database.clone()),
    };
    let session = Arc::new(Session::new(
        c,
        pool,
        tunnel,
        database,
        version,
        secrets.db_password,
    ));
    let out = info(&session);
    spawn_keep_alive(&session);
    state.sessions.write().await.insert(id.to_string(), session);
    Ok(out)
}

/// Pings the session every `keep_alive_secs` so idle connections and the SSH tunnel stay
/// open. Holds only a weak reference: the task ends when the session is gone or closed.
fn spawn_keep_alive(session: &Arc<Session>) {
    let secs = session.connection.options.keep_alive_secs;
    if secs == 0 {
        return;
    }
    let period = Duration::from_secs(u64::from(secs.max(MIN_KEEP_ALIVE_SECS)));
    let weak = Arc::downgrade(session);
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(period);
        tick.tick().await;
        loop {
            tick.tick().await;
            let Some(s) = weak.upgrade() else { break };
            if s.pool.is_closed() {
                break;
            }
            let _ = s.pool.server_version().await;
        }
    });
}

pub async fn disconnect(state: &AppState, id: &str) {
    let session = state.sessions.write().await.remove(id);
    if let Some(s) = session {
        s.close().await;
    }
}

/// Test with the form as it is (saved or not). Nothing is stored.
pub async fn test(
    state: &AppState,
    input: ConnectionInput,
    prompted: PromptedSecrets,
) -> AppResult<TestResult> {
    let c = input.connection;
    validate(&c)?;
    let started = Instant::now();
    let secrets = resolve(state, &c, Some(&input.secrets), &prompted)?;
    let warnings = test_warnings(&c);
    let (pool, tunnel) = open(state, &c, &secrets, 1).await?;
    let version = pool.server_version().await;
    pool.close().await;
    if let Some(t) = tunnel {
        t.close().await;
    }
    Ok(TestResult {
        server_version: version?,
        duration_ms: started.elapsed().as_millis() as u64,
        warnings,
    })
}

/// SSH-6: warn, don't block.
fn test_warnings(c: &Connection) -> Vec<Warning> {
    let mut warnings = Vec::new();
    if c.ssh.enabled {
        let db_host = c.db_host();
        let ssh_host = c.ssh.host.trim();
        if db_host.eq_ignore_ascii_case(ssh_host)
            && db_host != "127.0.0.1"
            && !db_host.eq_ignore_ascii_case("localhost")
        {
            warnings.push(Warning {
                code: "ssh_host_same_as_db",
                message: SSH_HOST_SAME_AS_DB.into(),
            });
        }
    }
    warnings
}

/// SSH-4: the user accepted the fingerprint; record it and let them retry.
pub fn trust_host_key(state: &AppState, host: &str, port: u16) -> AppResult<()> {
    let pending = state
        .pending_host_keys
        .lock()
        .unwrap()
        .remove(&format!("{host}:{port}"))
        .ok_or_else(|| {
            AppError::validation("No host key is waiting for confirmation; connect again.")
        })?;
    let path = ssh::known_hosts::default_path()
        .ok_or_else(|| AppError::internal("Can't find ~/.ssh/known_hosts."))?;
    ssh::trust(&pending, &path)
}

fn info(s: &Session) -> ConnectInfo {
    ConnectInfo {
        connection_id: s.connection.id.clone(),
        driver: s.connection.driver,
        database: s.database.clone(),
        server_version: s.server_version.clone(),
        through_tunnel: s.tunnel.is_some(),
    }
}

async fn current_database(pool: &DbPool) -> AppResult<String> {
    Ok(match pool {
        DbPool::Postgres(p) => {
            sqlx::query_scalar::<_, String>("SELECT current_database()::text")
                .fetch_one(p)
                .await?
        }
        DbPool::Mysql(p) => sqlx::query_scalar::<_, Option<String>>("SELECT DATABASE()")
            .fetch_one(p)
            .await?
            .unwrap_or_default(),
        DbPool::Sqlite(_) => "main".into(),
    })
}

/// Opens the tunnel (if any) and the pool. Shared by connect and test.
async fn open(
    state: &AppState,
    c: &Connection,
    secrets: &Resolved,
    max_connections: u32,
) -> AppResult<(DbPool, Option<Arc<Tunnel>>)> {
    let tunnel = if c.ssh.enabled {
        Some(Arc::new(open_tunnel(state, c, secrets).await?))
    } else {
        None
    };
    let target = tunnel.as_ref().map(|t| Target {
        host: "127.0.0.1".into(),
        port: t.local_port,
        through_tunnel: true,
    });
    let password = secrets.db_password.as_deref();
    match db::open_pool(c, target.as_ref(), password, None, max_connections).await {
        Ok(pool) => Ok((pool, tunnel)),
        Err(e) => {
            if let Some(t) = &tunnel {
                t.close().await;
            }
            Err(e)
        }
    }
}

/// Opens the SSH tunnel. An unknown or changed host key is kept for `trust_host_key`.
async fn open_tunnel(state: &AppState, c: &Connection, secrets: &Resolved) -> AppResult<Tunnel> {
    match ssh::open(tunnel_params(c, secrets)?).await {
        Ok(t) => Ok(t),
        Err(OpenError::HostKey(pending)) => {
            let err = pending.to_error();
            state
                .pending_host_keys
                .lock()
                .unwrap()
                .insert(format!("{}:{}", pending.host, pending.port), *pending);
            Err(err)
        }
        Err(OpenError::Other(e)) => Err(e),
    }
}

/// Tunnel settings from the connection, filled in from `~/.ssh/config`.
fn tunnel_params(c: &Connection, secrets: &Resolved) -> AppResult<TunnelParams> {
    let alias = c.ssh.host.trim().to_string();
    let cfg = ssh::config::load(&alias);
    let host = cfg.host_name.clone().unwrap_or_else(|| alias.clone());
    let port = c.ssh.port.or(cfg.port).unwrap_or(22);
    let user = if !c.ssh.user.trim().is_empty() {
        c.ssh.user.trim().to_string()
    } else {
        cfg.user
            .clone()
            .or_else(|| std::env::var("USER").ok())
            .unwrap_or_default()
    };
    let known_hosts = ssh::known_hosts::default_path().ok_or_else(|| {
        AppError::internal("Can't find the home directory for ~/.ssh/known_hosts.")
    })?;
    Ok(TunnelParams {
        host,
        port,
        user,
        password: secrets.ssh_password.clone(),
        keys: key_sources(c, secrets, &cfg),
        passphrase: secrets.ssh_passphrase.clone(),
        target_host: c.db_host(),
        target_port: c.db_port(),
        known_hosts,
    })
}

/// The imported key first; with none, SSH-3: `~/.ssh/config` and the OpenSSH defaults.
fn key_sources(
    c: &Connection,
    secrets: &Resolved,
    cfg: &ssh::config::HostConfig,
) -> Vec<KeySource> {
    let mut keys = Vec::new();
    if let Some((name, pem)) = &secrets.ssh_key {
        keys.push(KeySource::Stored {
            name: if name.is_empty() {
                "imported key".into()
            } else {
                name.clone()
            },
            pem: pem.clone(),
        });
    }
    let want_files = (c.ssh.use_key && secrets.ssh_key.is_none())
        || (!c.ssh.use_key && secrets.ssh_password.is_none());
    if want_files {
        let mut files = cfg.identity_files.clone();
        if files.is_empty() {
            files = ssh::config::default_identities();
        }
        keys.extend(
            files
                .into_iter()
                .filter(|p| p.exists())
                .map(KeySource::File),
        );
    }
    keys
}
