//! The forwarding side of a tunnel: a local listener whose connections become
//! direct-tcpip channels to the database host.

use std::sync::Arc;

use russh::client::Handle;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::error::{AppError, AppResult, ErrorKind, Layer};

use super::host_key::Client;
use super::TunnelParams;

const PROBE_HINT: &str = "Use the database address as seen from the SSH server. If the \
    database runs on the SSH server itself, use 127.0.0.1.";

pub struct Tunnel {
    pub local_port: u16,
    handle: Arc<Handle<Client>>,
    task: JoinHandle<()>,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Tunnel {
    /// Checks the database is reachable through `handle`, then listens on a random
    /// local port and forwards every connection to it.
    pub(super) async fn start(handle: Arc<Handle<Client>>, p: &TunnelParams) -> AppResult<Self> {
        probe(&handle, p).await?;
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| tunnel_error(format!("Can't open a local port for the tunnel: {e}")))?;
        let local_port = listener
            .local_addr()
            .map_err(|e| tunnel_error(e.to_string()))?
            .port();
        let target = (p.target_host.clone(), p.target_port);
        let task = tokio::spawn(accept_loop(listener, handle.clone(), target));
        Ok(Tunnel {
            local_port,
            handle,
            task,
        })
    }

    pub async fn close(&self) {
        self.task.abort();
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
    }
}

fn tunnel_error(message: String) -> AppError {
    AppError::new(ErrorKind::TunnelFailed, Layer::Ssh, message)
}

async fn accept_loop(listener: TcpListener, handle: Arc<Handle<Client>>, target: (String, u16)) {
    while let Ok((sock, peer)) = listener.accept().await {
        let h = handle.clone();
        let target = target.clone();
        tokio::spawn(async move { forward(h, sock, peer.port(), target).await });
    }
}

async fn forward(
    handle: Arc<Handle<Client>>,
    mut sock: TcpStream,
    peer_port: u16,
    (host, port): (String, u16),
) {
    let channel = handle
        .channel_open_direct_tcpip(host, port as u32, "127.0.0.1", peer_port as u32)
        .await;
    match channel {
        Ok(ch) => {
            let mut stream = ch.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
        }
        Err(e) => log::warn!("tunnel channel failed: {e}"),
    }
}

/// Opens (and drops) one forwarded channel so a bad database host/port is reported
/// as a tunnel problem instead of a vague database error.
async fn probe(handle: &Handle<Client>, p: &TunnelParams) -> AppResult<()> {
    let target = p.target_host.clone();
    match handle
        .channel_open_direct_tcpip(target, p.target_port as u32, "127.0.0.1", 0)
        .await
    {
        Ok(ch) => {
            let _ = ch.close().await;
            Ok(())
        }
        Err(e) => Err(tunnel_error(format!(
            "The SSH server can't reach the database at {}:{} ({e}).",
            p.target_host, p.target_port
        ))
        .hint(PROBE_HINT)),
    }
}
