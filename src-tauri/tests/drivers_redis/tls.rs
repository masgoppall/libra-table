//! TLS against a server that only speaks TLS:
//!   LIBRATABLE_TEST_REDIS_TLS_URL=redis://:password@127.0.0.1:16390
//!   LIBRATABLE_TEST_REDIS_CA=/path/to/ca.crt (the CA that signed the server certificate)

use libratable_lib::db::open_pool;
use libratable_lib::models::Connection;
use serde_json::json;

use super::Server;

fn connection(s: &Server, mode: &str, ca: &str) -> Connection {
    serde_json::from_value(json!({
        "id": "redis-tls", "name": "Redis TLS", "driver": "redis",
        "db": { "host": s.host, "port": s.port },
        "tls": { "mode": mode, "caPath": ca, "certPath": "", "keyPath": "" },
    }))
    .unwrap()
}

#[tokio::test]
async fn redis_tls() {
    let (Some(server), Ok(ca)) = (
        Server::from_var("LIBRATABLE_TEST_REDIS_TLS_URL"),
        std::env::var("LIBRATABLE_TEST_REDIS_CA"),
    ) else {
        eprintln!("skipped: set LIBRATABLE_TEST_REDIS_TLS_URL and LIBRATABLE_TEST_REDIS_CA");
        return;
    };
    let pw = server.password.as_deref();
    // Require encrypts without checking; Verify checks against the given CA.
    for (mode, ca_path) in [("require", ""), ("verify_full", ca.as_str())] {
        let pool = open_pool(&connection(&server, mode, ca_path), None, pw, None, 1)
            .await
            .unwrap_or_else(|e| panic!("{mode}: {e:?}"));
        assert!(!pool.server_version().await.unwrap().is_empty());
        pool.close().await;
    }
    // Verifying with the system roots fails: the test CA isn't trusted.
    let err = open_pool(&connection(&server, "verify_full", ""), None, pw, None, 1)
        .await
        .err()
        .expect("an unknown CA fails");
    assert!(
        err.message.to_lowercase().contains("certificate"),
        "{err:?}"
    );
}
