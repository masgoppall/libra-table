use serde_json::{json, Value};

use super::*;
use crate::models::{Driver, PasswordMode, Tag, TlsMode};
use crate::state::test_state;
use crate::storage::secrets::SecretKind;

const KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----\n";

/// Entries shaped like TablePlus' own (every key present, numbers as strings).
fn entry(id: &str, name: &str, driver: &str, group: &str) -> Value {
    json!({
        "ID": id, "ConnectionName": name, "Driver": driver, "GroupID": group,
        "DatabaseHost": "", "DatabasePort": "", "DatabaseUser": "root", "DatabaseName": "shop",
        "DatabasePasswordMode": 0, "DatabasePassword": "db-pw", "DatabaseSocket": "",
        "DatabasePath": "", "isUseSocket": false, "Enviroment": "staging",
        "statusColor": "#686B6F", "isOverSSH": false, "ServerAddress": "", "ServerPort": "",
        "ServerUser": "", "ServerPasswordMode": 0, "isUsePrivateKey": false,
        "ServerPrivateKeyName": "", "tLSMode": 0, "TlsKeyPaths": ["", "", ""],
        "LimitRowsReturned": 0, "LimitQueryRowsReturned": 0, "ShowSystemSchemas": false,
        "StartupCommands": "",
    })
}

fn sample() -> Value {
    let mut tunnel = entry("c1", "Shop prod", "MySQL", "g2");
    let t = tunnel.as_object_mut().unwrap();
    for (k, v) in [
        ("DatabasePort", json!("3307")),
        ("Enviroment", json!("production")),
        ("statusColor", json!("#007F3D")),
        ("isOverSSH", json!(true)),
        ("ServerAddress", json!("bastion.example.com")),
        ("ServerPort", json!("2222")),
        ("ServerUser", json!("deploy")),
        ("isUsePrivateKey", json!(true)),
        ("ServerPrivateKeyName", json!("id_deploy")),
        ("ServerPrivateKeyData", json!(KEY)),
        ("PrivateKeyPassword", json!("key-pass")),
        ("tLSMode", json!(2)),
        ("LimitRowsReturned", json!(100000)),
    ] {
        t.insert(k.into(), v);
    }
    let mut asked = entry("c2", "Analytics", "PostgreSQL", "g1");
    let a = asked.as_object_mut().unwrap();
    a.insert("DatabaseHost".into(), json!("db.example.com"));
    a.insert("DatabasePasswordMode".into(), json!(1));
    a.insert("tLSMode".into(), json!(5));
    a.insert("TlsKeyPaths".into(), json!(["/k.pem", "/c.pem", "/ca.pem"]));
    json!({
        "connections": [tunnel, asked, entry("c3", "Odd", "Couchbase", "")],
        "groups": [
            { "ID": "g1", "Name": "Clients", "GroupID": "", "IsExpaned": true },
            { "ID": "g2", "Name": "Production", "GroupID": "g1", "IsExpaned": false },
            { "ID": "g3", "Name": "Empty", "GroupID": "", "IsExpaned": true },
        ],
    })
}

fn sealed(doc: &Value) -> Vec<u8> {
    rncryptor::encrypt(
        "export-pw",
        &serde_json::to_vec(doc).unwrap(),
        [1; 16],
        [2; 16],
    )
}

#[tokio::test]
async fn imports_connections_groups_and_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let state = test_state(dir.path()).await;
    let file = sealed(&sample());
    assert!(import(&state, &file, "wrong").is_err());

    let report = import(&state, &file, "export-pw").unwrap();
    assert_eq!(report.added, 2);
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].name, "Odd");

    let cfg = state.config.lock().unwrap();
    // Only groups holding a connection come over, nested as in TablePlus.
    let names: Vec<&str> = cfg.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Clients", "Production"]);
    let (clients, prod) = (&cfg.groups[0], &cfg.groups[1]);
    assert_eq!(prod.parent_id.as_ref(), Some(&clients.id));
    assert!(prod.collapsed && !clients.collapsed);

    let shop = cfg
        .connections
        .iter()
        .find(|c| c.name == "Shop prod")
        .unwrap();
    assert_eq!(shop.driver, Driver::Mysql);
    assert_eq!(shop.group_id.as_ref(), Some(&prod.id));
    assert_eq!(
        (shop.tag, shop.color.as_str()),
        (Tag::Production, "#007F3D")
    );
    assert_eq!(
        (shop.db.port, shop.db.database.as_str()),
        (Some(3307), "shop")
    );
    assert!(shop.ssh.enabled && shop.ssh.use_key);
    assert_eq!(
        (shop.ssh.host.as_str(), shop.ssh.port),
        ("bastion.example.com", Some(2222))
    );
    assert_eq!(shop.ssh.key_name, "id_deploy");
    assert_eq!(shop.tls.mode, TlsMode::Require);
    assert_eq!(shop.options.page_size, crate::db::data::MAX_PAGE_SIZE);
    let secret = |id: &str, kind| state.secrets.get(id, kind).unwrap();
    assert_eq!(
        secret(&shop.id, SecretKind::DbPassword).as_deref(),
        Some("db-pw")
    );
    assert_eq!(secret(&shop.id, SecretKind::SshKey).as_deref(), Some(KEY));
    assert_eq!(
        secret(&shop.id, SecretKind::SshKeyPassphrase).as_deref(),
        Some("key-pass")
    );

    let pg = cfg
        .connections
        .iter()
        .find(|c| c.name == "Analytics")
        .unwrap();
    assert_eq!(
        (pg.driver, pg.tag, pg.color.as_str()),
        (Driver::Postgres, Tag::Staging, "")
    );
    assert_eq!(pg.db.password_mode, PasswordMode::Ask);
    // "Ask every time" keeps nothing, even when the export had a password.
    assert_eq!(secret(&pg.id, SecretKind::DbPassword), None);
    assert_eq!(pg.tls.mode, TlsMode::VerifyFull);
    assert_eq!(
        (
            pg.tls.key_path.as_str(),
            pg.tls.cert_path.as_str(),
            pg.tls.ca_path.as_str()
        ),
        ("/k.pem", "/c.pem", "/ca.pem")
    );
    drop(cfg);

    // A second run finds everything already there.
    let again = import(&state, &file, "export-pw").unwrap();
    assert_eq!(again.added, 0);
    assert_eq!(again.skipped.len(), 3);
    assert_eq!(state.config.lock().unwrap().groups.len(), 2);
}

#[test]
fn reads_property_lists_and_bare_lists() {
    let list = json!([entry("c1", "A", "MariaDB", "")]);
    assert_eq!(
        payload::read(&serde_json::to_vec(&list).unwrap())
            .unwrap()
            .connections
            .len(),
        1
    );

    let mut dict = plist::Dictionary::new();
    dict.insert("ConnectionName".into(), plist::Value::String("B".into()));
    dict.insert("DatabasePort".into(), plist::Value::Integer(5432.into()));
    dict.insert("isOverSSH".into(), plist::Value::Boolean(true));
    let mut xml = Vec::new();
    plist::Value::Array(vec![plist::Value::Dictionary(dict)])
        .to_writer_xml(&mut xml)
        .unwrap();
    let doc = payload::read(&xml).unwrap();
    let e = Entry(&doc.connections[0]);
    assert_eq!(
        (e.text("ConnectionName"), e.int("DatabasePort")),
        ("B".into(), 5432)
    );
    assert!(e.flag("isOverSSH"));
    assert!(payload::read(b"not a document").is_err());
}

/// Groups holding their connections and subgroups, the shape of a group export.
#[tokio::test]
async fn imports_a_group_tree() {
    let mut nested = entry("c2", "Billing", "PostgreSQL", "");
    nested
        .as_object_mut()
        .unwrap()
        .insert("DatabaseHost".into(), json!("billing.local"));
    let tree = json!([
        {
            "ID": "g1", "Name": "Clients", "IsExpaned": true,
            "Items": [
                entry("c1", "Shop", "MySQL", ""),
                { "ID": "g2", "Name": "Billing team", "IsExpaned": false, "Items": [nested] },
            ],
        },
        { "Name": "No id", "Children": [entry("c3", "Cache", "Redis", "")] },
    ]);
    let doc = payload::read(&serde_json::to_vec(&tree).unwrap()).unwrap();
    assert_eq!((doc.connections.len(), doc.groups.len()), (3, 3));

    let dir = tempfile::tempdir().unwrap();
    let state = test_state(dir.path()).await;
    let report = import(&state, &sealed(&tree), "export-pw").unwrap();
    assert_eq!((report.added, report.skipped.len()), (3, 0));
    let cfg = state.config.lock().unwrap();
    let group = |name: &str| cfg.groups.iter().find(|g| g.name == name).unwrap();
    let conn = |name: &str| cfg.connections.iter().find(|c| c.name == name).unwrap();
    assert_eq!(
        group("Billing team").parent_id.as_ref(),
        Some(&group("Clients").id)
    );
    assert_eq!(conn("Shop").group_id.as_ref(), Some(&group("Clients").id));
    assert_eq!(
        conn("Billing").group_id.as_ref(),
        Some(&group("Billing team").id)
    );
    assert_eq!(conn("Cache").group_id.as_ref(), Some(&group("No id").id));
}

#[test]
fn reads_documents_nested_as_text_and_explains_unknown_ones() {
    use base64::Engine;
    let inner = serde_json::to_vec(&json!([entry("c1", "A", "MySQL", "")])).unwrap();
    let wrapped = json!({ "version": 2, "data": base64::prelude::BASE64_STANDARD.encode(inner) });
    let doc = payload::read(&serde_json::to_vec(&wrapped).unwrap()).unwrap();
    assert_eq!(doc.connections.len(), 1);

    let odd = json!([{ "uuid": "x", "title": "y" }]);
    let err = payload::read(&serde_json::to_vec(&odd).unwrap())
        .err()
        .unwrap();
    // Key names help with an unknown layout; values never appear.
    assert!(
        err.message.contains("1 entries with the keys title, uuid"),
        "{}",
        err.message
    );
}

#[test]
fn driver_names() {
    assert_eq!(convert::driver("MicrosoftSQLServer"), Some(Driver::Mssql));
    assert_eq!(convert::driver("PostgreSQL"), Some(Driver::Postgres));
    assert_eq!(convert::driver("Cloudflare D1"), Some(Driver::CloudflareD1));
    assert_eq!(convert::driver("Couchbase"), None);
}
