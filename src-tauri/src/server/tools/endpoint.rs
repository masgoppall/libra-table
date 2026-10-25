//! Where a native tool connects, and the arguments and environment that say so.
//!
//! Tools reach the server the way the session's pool does: through the SSH tunnel's
//! local port when there is one. The password goes in the environment (PGPASSWORD /
//! MYSQL_PWD), never on the command line, where `ps` would show it.

use std::path::Path;

use crate::models::{Dialect, TlsMode, TlsSettings};
use crate::state::Session;

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub driver: Dialect,
    /// None when a Unix socket is used.
    pub host: Option<String>,
    pub port: u16,
    pub socket: Option<String>,
    pub user: String,
    pub database: String,
    pub password: Option<String>,
    pub tls: TlsSettings,
    pub through_tunnel: bool,
    /// SQLite: the database file.
    pub file: String,
}

impl Endpoint {
    pub fn of(s: &Session) -> Self {
        let c = &s.connection;
        let socket = (c.db.use_socket && !c.db.socket.is_empty() && s.tunnel.is_none())
            .then(|| c.db.socket.clone());
        let (host, port) = match (&s.tunnel, &socket) {
            (Some(t), _) => (Some("127.0.0.1".to_string()), t.local_port),
            (None, Some(_)) => (None, c.db_port()),
            (None, None) => (Some(c.db_host()), c.db_port()),
        };
        Endpoint {
            driver: c.driver.dialect(),
            host,
            port,
            socket,
            user: c.db.user.clone(),
            database: s.database.clone(),
            password: s.db_password.clone(),
            tls: c.tls.clone(),
            through_tunnel: s.tunnel.is_some(),
            file: c.db.file_path.clone(),
        }
    }

    /// `-h -p -U` (Postgres) or `-h -P -u` (MySQL), without the database. `mariadb`
    /// picks MariaDB's TLS flags, which differ from MySQL's.
    pub fn conn_args(&self, mariadb: bool) -> Vec<String> {
        match self.driver {
            Dialect::Postgres => self.pg_args(),
            Dialect::Mysql => self.mysql_args(mariadb),
            Dialect::Sqlite => Vec::new(),
        }
    }

    fn pg_args(&self) -> Vec<String> {
        let (host, port) = match (&self.host, &self.socket) {
            (Some(h), _) => (h.clone(), self.port),
            (None, Some(sock)) => pg_socket(sock, self.port),
            (None, None) => ("127.0.0.1".into(), self.port),
        };
        let mut a = vec!["-h".into(), host, "-p".into(), port.to_string()];
        if !self.user.is_empty() {
            a.extend(["-U".into(), self.user.clone()]);
        }
        a
    }

    fn mysql_args(&self, mariadb: bool) -> Vec<String> {
        let mut a = match (&self.host, &self.socket) {
            (Some(h), _) => vec![
                "-h".into(),
                h.clone(),
                "-P".into(),
                self.port.to_string(),
                // "localhost" would otherwise mean the default socket.
                "--protocol=TCP".into(),
            ],
            (None, Some(sock)) => vec![format!("--socket={sock}")],
            (None, None) => vec![],
        };
        if !self.user.is_empty() {
            a.extend(["-u".into(), self.user.clone()]);
        }
        a.extend(mysql_tls(&self.tls, mariadb));
        a
    }

    /// The password and TLS settings for the tool's environment.
    pub fn env(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();
        let pw_var = match self.driver {
            Dialect::Postgres => "PGPASSWORD",
            Dialect::Mysql => "MYSQL_PWD",
            Dialect::Sqlite => return env,
        };
        if let Some(pw) = &self.password {
            env.push((pw_var.to_string(), pw.clone()));
        }
        if self.driver == Dialect::Postgres {
            env.push(("PGSSLMODE".into(), pg_ssl_mode(self.tls.mode).into()));
            for (var, path) in [
                ("PGSSLROOTCERT", &self.tls.ca_path),
                ("PGSSLCERT", &self.tls.cert_path),
                ("PGSSLKEY", &self.tls.key_path),
            ] {
                if !path.is_empty() {
                    env.push((var.into(), path.clone()));
                }
            }
            env.push(("PGAPPNAME".into(), "LibraTable".into()));
        }
        env
    }
}

/// A Postgres socket setting is a directory, or the socket file `.s.PGSQL.<port>`.
fn pg_socket(sock: &str, port: u16) -> (String, u16) {
    let p = Path::new(sock);
    let name = p.file_name().map(|n| n.to_string_lossy().to_string());
    match name.as_deref().and_then(|n| n.strip_prefix(".s.PGSQL.")) {
        Some(n) => (
            p.parent()
                .map(|d| d.display().to_string())
                .unwrap_or_default(),
            n.parse().unwrap_or(port),
        ),
        None => (sock.to_string(), port),
    }
}

pub fn pg_ssl_mode(mode: TlsMode) -> &'static str {
    match mode {
        TlsMode::Disable => "disable",
        TlsMode::Allow => "allow",
        TlsMode::Prefer => "prefer",
        TlsMode::Require => "require",
        TlsMode::VerifyCa => "verify-ca",
        TlsMode::VerifyFull => "verify-full",
    }
}

/// MySQL clients take `--ssl-mode`; MariaDB's take `--ssl`/`--skip-ssl` and
/// `--ssl-verify-server-cert`. "Prefer" is both clients' default.
pub fn mysql_tls(tls: &TlsSettings, mariadb: bool) -> Vec<String> {
    let mut a: Vec<String> = match (mariadb, tls.mode) {
        (_, TlsMode::Allow | TlsMode::Prefer) => vec![],
        (false, TlsMode::Disable) => vec!["--ssl-mode=DISABLED".into()],
        (false, TlsMode::Require) => vec!["--ssl-mode=REQUIRED".into()],
        (false, TlsMode::VerifyCa) => vec!["--ssl-mode=VERIFY_CA".into()],
        (false, TlsMode::VerifyFull) => vec!["--ssl-mode=VERIFY_IDENTITY".into()],
        (true, TlsMode::Disable) => vec!["--skip-ssl".into()],
        (true, TlsMode::Require) => vec!["--ssl".into()],
        (true, TlsMode::VerifyCa | TlsMode::VerifyFull) => {
            vec!["--ssl".into(), "--ssl-verify-server-cert".into()]
        }
    };
    for (flag, path) in [
        ("--ssl-ca", &tls.ca_path),
        ("--ssl-cert", &tls.cert_path),
        ("--ssl-key", &tls.key_path),
    ] {
        if !path.is_empty() {
            a.push(format!("{flag}={path}"));
        }
    }
    a
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn endpoint(driver: Dialect) -> Endpoint {
        Endpoint {
            driver,
            host: Some("127.0.0.1".into()),
            port: 5432,
            socket: None,
            user: "admin".into(),
            database: "shop".into(),
            password: Some("s3cret".into()),
            tls: TlsSettings::default(),
            through_tunnel: false,
            file: String::new(),
        }
    }

    #[test]
    fn password_only_in_the_environment() {
        let pg = endpoint(Dialect::Postgres);
        let args = pg.conn_args(false).join(" ");
        assert_eq!(args, "-h 127.0.0.1 -p 5432 -U admin");
        assert!(pg.env().contains(&("PGPASSWORD".into(), "s3cret".into())));
        let my = Endpoint {
            port: 3306,
            ..endpoint(Dialect::Mysql)
        };
        let args = my.conn_args(false).join(" ");
        assert_eq!(args, "-h 127.0.0.1 -P 3306 --protocol=TCP -u admin");
        assert!(!args.contains("s3cret"));
        assert_eq!(my.env(), vec![("MYSQL_PWD".into(), "s3cret".into())]);
    }

    #[test]
    fn sockets_and_tls() {
        assert_eq!(pg_socket("/tmp/.s.PGSQL.5433", 5432), ("/tmp".into(), 5433));
        assert_eq!(
            pg_socket("/var/run/postgresql", 5432).0,
            "/var/run/postgresql"
        );
        let tls = TlsSettings {
            mode: TlsMode::Require,
            ca_path: "/ca.pem".into(),
            ..Default::default()
        };
        assert_eq!(
            mysql_tls(&tls, false),
            vec!["--ssl-mode=REQUIRED", "--ssl-ca=/ca.pem"]
        );
        assert_eq!(mysql_tls(&tls, true), vec!["--ssl", "--ssl-ca=/ca.pem"]);
    }
}
