//! SSH tunnel end-to-end against an in-process russh server (SSH-1, SSH-2, SSH-4,
//! SSH-5, SSH-7): host key prompt, trust, wrong password, forwarding, unreachable
//! target, changed host key, and encrypted keys.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use libratable_lib::error::ErrorKind;
use libratable_lib::ssh::{self, KeySource, OpenError, TunnelParams};
use russh::server::{self, Auth, Msg, Session};
use russh::Channel;
use russh_keys::key::{KeyPair, PublicKey};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
struct TestServer {
    client_key: Option<PublicKey>,
}

#[async_trait]
impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        if user == "tester" && password == "secret" {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::Reject {
                proceed_with_methods: None,
            })
        }
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        if user == "tester" && self.client_key.as_ref() == Some(key) {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::Reject {
                proceed_with_methods: None,
            })
        }
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _orig: &str,
        _orig_port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let Ok(mut target) = TcpStream::connect((host.to_string(), port as u16)).await else {
            return Ok(false);
        };
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut target).await;
        });
        Ok(true)
    }
}

/// Starts an SSH server with `host_key` and returns its port.
async fn start_server(host_key: KeyPair, client_key: Option<PublicKey>) -> u16 {
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (sock, _) = listener.accept().await.unwrap();
            let cfg = config.clone();
            let handler = TestServer {
                client_key: client_key.clone(),
            };
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(cfg, sock, handler).await {
                    let _ = session.await;
                }
            });
        }
    });
    port
}

/// An echo server standing in for the database.
async fn start_echo() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                while let Ok(n) = sock.read(&mut buf).await {
                    if n == 0 || sock.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

fn params(port: u16, target_port: u16, known_hosts: &std::path::Path) -> TunnelParams {
    TunnelParams {
        host: "127.0.0.1".into(),
        port,
        user: "tester".into(),
        password: Some("secret".into()),
        keys: vec![],
        passphrase: None,
        target_host: "127.0.0.1".into(),
        target_port,
        known_hosts: known_hosts.to_path_buf(),
    }
}

fn kind(r: Result<ssh::Tunnel, OpenError>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(OpenError::HostKey(p)) => {
            if p.changed {
                "host_key_changed".into()
            } else {
                "host_key_unknown".into()
            }
        }
        Err(OpenError::Other(e)) => format!("{:?}", e.kind),
    }
}

#[tokio::test]
async fn host_key_trust_password_and_forwarding() {
    let dir = tempfile::tempdir().unwrap();
    let known_hosts = dir.path().join("known_hosts");
    let ssh_port = start_server(KeyPair::generate_ed25519().unwrap(), None).await;
    let echo_port = start_echo().await;

    // 1. Unknown host key → the caller gets the key to show its fingerprint.
    let pending = match ssh::open(params(ssh_port, echo_port, &known_hosts)).await {
        Err(OpenError::HostKey(p)) => p,
        other => panic!("expected host key prompt, got {}", kind(other)),
    };
    assert!(!pending.changed);
    assert!(pending.fingerprint().starts_with("SHA256:"));
    let err = pending.to_error();
    assert_eq!(err.kind, ErrorKind::SshHostKeyUnknown);
    assert_eq!(err.data.as_ref().unwrap()["port"], ssh_port);

    // 2. Trust it → written to known_hosts.
    ssh::trust(&pending, &known_hosts).unwrap();
    assert!(std::fs::read_to_string(&known_hosts)
        .unwrap()
        .contains(&format!("[127.0.0.1]:{ssh_port}")));

    // 3. Wrong password → SSH layer auth error.
    let mut bad = params(ssh_port, echo_port, &known_hosts);
    bad.password = Some("nope".into());
    assert_eq!(kind(ssh::open(bad).await), "SshAuthFailed");

    // 4. Correct password → bytes flow through the tunnel.
    let tunnel = match ssh::open(params(ssh_port, echo_port, &known_hosts)).await {
        Ok(t) => t,
        Err(e) => panic!("tunnel failed: {}", kind(Err(e))),
    };
    let mut sock = TcpStream::connect(("127.0.0.1", tunnel.local_port))
        .await
        .unwrap();
    sock.write_all(b"ping through ssh").await.unwrap();
    let mut buf = [0u8; 16];
    tokio::time::timeout(Duration::from_secs(5), sock.read_exact(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&buf, b"ping through ssh");
    tunnel.close().await;

    // 5. Target not reachable from the SSH server → tunnel error, not a DB error.
    let closed = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert_eq!(
        kind(ssh::open(params(ssh_port, closed, &known_hosts)).await),
        "TunnelFailed"
    );

    // 6. Nothing listening on the SSH port → unreachable.
    assert_eq!(
        kind(ssh::open(params(closed, echo_port, &known_hosts)).await),
        "SshUnreachable"
    );
}

#[tokio::test]
async fn changed_host_key_is_flagged_and_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let known_hosts = dir.path().join("known_hosts");
    let ssh_port = start_server(KeyPair::generate_ed25519().unwrap(), None).await;
    let echo_port = start_echo().await;

    // known_hosts holds a different key *of another type* for this host:port, which
    // russh-keys alone would report as merely "unknown".
    let other_key = concat!(
        "AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEmKSENjQEezOmxkZMy7opKgwFB9",
        "nkt5YRrYMjNuG5N87uRgg6CLrbo5wAdT/y6v0mKV0U2w0WZ2YB/++Tpockg="
    );
    std::fs::write(
        &known_hosts,
        format!("[127.0.0.1]:{ssh_port} ecdsa-sha2-nistp256 {other_key}\n"),
    )
    .unwrap();

    let pending = match ssh::open(params(ssh_port, echo_port, &known_hosts)).await {
        Err(OpenError::HostKey(p)) => p,
        other => panic!("expected changed key, got {}", kind(other)),
    };
    assert!(pending.changed);
    assert_eq!(pending.to_error().kind, ErrorKind::SshHostKeyChanged);

    ssh::trust(&pending, &known_hosts).unwrap();
    let text = std::fs::read_to_string(&known_hosts).unwrap();
    assert_eq!(
        text.lines()
            .filter(|l| l.contains(&format!("[127.0.0.1]:{ssh_port}")))
            .count(),
        1,
        "old entry replaced"
    );
    assert_eq!(
        kind(ssh::open(params(ssh_port, echo_port, &known_hosts)).await),
        "ok"
    );
}

#[tokio::test]
async fn encrypted_key_needs_passphrase() {
    let dir = tempfile::tempdir().unwrap();
    let known_hosts = dir.path().join("known_hosts");

    let client = KeyPair::generate_ed25519().unwrap();
    let mut pem = Vec::new();
    russh_keys::encode_pkcs8_pem_encrypted(&client, b"hunter2", 10, &mut pem).unwrap();
    let pem = String::from_utf8(pem).unwrap();

    let host_key = KeyPair::generate_ed25519().unwrap();
    let host_pub = host_key.clone_public_key().unwrap();
    let ssh_port = start_server(host_key, Some(client.clone_public_key().unwrap())).await;
    russh_keys::learn_known_hosts_path("127.0.0.1", ssh_port, &host_pub, &known_hosts).unwrap();
    let echo_port = start_echo().await;

    let with_key = |passphrase: Option<&str>| {
        let mut p = params(ssh_port, echo_port, &known_hosts);
        p.password = None;
        p.keys = vec![KeySource::Stored {
            name: "id_test".into(),
            pem: pem.clone(),
        }];
        p.passphrase = passphrase.map(str::to_string);
        p
    };
    assert_eq!(
        kind(ssh::open(with_key(None)).await),
        "SshPassphraseRequired"
    );
    assert_eq!(
        kind(ssh::open(with_key(Some("wrong"))).await),
        "SshPassphraseWrong"
    );
    assert_eq!(kind(ssh::open(with_key(Some("hunter2"))).await), "ok");
}
