//! Safe mode (SAFE-2): before statements run, the connection's safe mode may require a
//! confirmation (by default: writes on a `production` connection), and a read-only
//! connection refuses writes. Classification uses a SQL parser, not regexes in the UI.
//! Anything the parser can't read counts as a write (fail safe).

use serde::Serialize;
use sqlparser::ast::{Query, SetExpr, Statement};
use sqlparser::dialect::{Dialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

use crate::models::{Connection, Driver, SafeMode};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// SELECT, SHOW, EXPLAIN (without ANALYZE of a write), DESCRIBE, read-only PRAGMA.
    Read,
    /// SET, USE, BEGIN/COMMIT/ROLLBACK: changes only the session, no data.
    Session,
    /// Everything else, including statements the parser can't read.
    Write,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classified {
    pub sql: String,
    pub kind: Kind,
    /// False when the parser couldn't read the statement (it is treated as a write).
    pub understood: bool,
}

/// SQLite pragmas that take an argument but only read.
const READ_ONLY_PRAGMAS: &[&str] = &[
    "table_info",
    "table_xinfo",
    "table_list",
    "index_list",
    "index_info",
    "index_xinfo",
    "foreign_key_list",
    "foreign_key_check",
    "integrity_check",
    "quick_check",
    "database_list",
    "collation_list",
    "function_list",
    "module_list",
    "pragma_list",
    "compile_options",
];

fn dialect(driver: Driver) -> Box<dyn Dialect> {
    match driver {
        Driver::Postgres => Box::new(PostgreSqlDialect {}),
        Driver::Mysql => Box::new(MySqlDialect {}),
        Driver::Sqlite => Box::new(SQLiteDialect {}),
    }
}

/// MySQL executes things sqlparser reads as comments: `/*! … */` (versioned comments)
/// and `--x` (a MySQL comment needs whitespace after `--`). Text containing either
/// can't be classified safely.
fn mysql_hidden_code(sql: &str) -> bool {
    if sql.contains("/*!") {
        return true;
    }
    let b = sql.as_bytes();
    (0..b.len().saturating_sub(1)).any(|i| {
        b[i] == b'-' && b[i + 1] == b'-' && b.get(i + 2).is_some_and(|c| !c.is_ascii_whitespace())
    })
}

/// Classifies one statement (already split by `db::split`).
pub fn classify(driver: Driver, sql: &str) -> Classified {
    if driver == Driver::Mysql && mysql_hidden_code(sql) {
        return Classified {
            sql: sql.to_string(),
            kind: Kind::Write,
            understood: false,
        };
    }
    let d = dialect(driver);
    match Parser::parse_sql(d.as_ref(), sql) {
        Ok(stmts) if !stmts.is_empty() => {
            let kind = stmts
                .iter()
                .map(statement_kind)
                .max_by_key(|k| rank(*k))
                .unwrap_or(Kind::Write);
            Classified {
                sql: sql.to_string(),
                kind,
                understood: true,
            }
        }
        _ => Classified {
            sql: sql.to_string(),
            kind: Kind::Write,
            understood: false,
        },
    }
}

impl Kind {
    /// For a statement wrapped by EXPLAIN: Write stays Write, everything else is a read.
    fn max_write(self) -> Kind {
        if self == Kind::Write {
            Kind::Write
        } else {
            Kind::Read
        }
    }
}

fn rank(k: Kind) -> u8 {
    match k {
        Kind::Read => 0,
        Kind::Session => 1,
        Kind::Write => 2,
    }
}

fn statement_kind(s: &Statement) -> Kind {
    match s {
        Statement::Query(q) => {
            if query_writes(q) {
                Kind::Write
            } else {
                Kind::Read
            }
        }
        // EXPLAIN ANALYZE (also spelled `EXPLAIN (ANALYZE, …)`) executes the statement,
        // so explaining any write counts as a write.
        Statement::Explain { statement, .. } => statement_kind(statement).max_write(),
        Statement::ExplainTable { .. }
        | Statement::ShowFunctions { .. }
        | Statement::ShowVariable { .. }
        | Statement::ShowStatus { .. }
        | Statement::ShowVariables { .. }
        | Statement::ShowCreate { .. }
        | Statement::ShowColumns { .. }
        | Statement::ShowDatabases { .. }
        | Statement::ShowSchemas { .. }
        | Statement::ShowTables { .. }
        | Statement::ShowViews { .. }
        | Statement::ShowCollation { .. } => Kind::Read,
        Statement::Pragma { name, value, is_eq } => {
            // `PRAGMA x` reads a setting; `PRAGMA x = v` / `PRAGMA x(v)` usually sets it,
            // except the table-valued introspection pragmas below.
            let pragma = name
                .0
                .last()
                .map(|i| i.value.to_ascii_lowercase())
                .unwrap_or_default();
            if value.is_none() || (!*is_eq && READ_ONLY_PRAGMAS.contains(&pragma.as_str())) {
                Kind::Read
            } else {
                Kind::Write
            }
        }
        // SET GLOBAL / PERSIST / PASSWORD change the server, not just this session.
        Statement::SetVariable { .. } => {
            let text = s.to_string().to_ascii_uppercase();
            if text.contains("GLOBAL") || text.contains("PERSIST") || text.contains("PASSWORD") {
                Kind::Write
            } else {
                Kind::Session
            }
        }
        Statement::SetTimeZone { .. }
        | Statement::SetNames { .. }
        | Statement::SetNamesDefault { .. }
        | Statement::SetTransaction { .. }
        | Statement::SetRole { .. }
        | Statement::Use(_)
        | Statement::StartTransaction { .. }
        | Statement::Commit { .. }
        | Statement::Rollback { .. } => Kind::Session,
        _ => Kind::Write,
    }
}

fn query_writes(q: &Query) -> bool {
    if let Some(with) = &q.with {
        if with.cte_tables.iter().any(|cte| query_writes(&cte.query)) {
            return true;
        }
    }
    set_expr_writes(&q.body)
}

fn set_expr_writes(e: &SetExpr) -> bool {
    match e {
        // SELECT … INTO new_table creates a table.
        SetExpr::Select(s) => s.into.is_some(),
        SetExpr::Query(q) => query_writes(q),
        SetExpr::SetOperation { left, right, .. } => {
            set_expr_writes(left) || set_expr_writes(right)
        }
        SetExpr::Values(_) | SetExpr::Table(_) => false,
        SetExpr::Insert(_) | SetExpr::Update(_) => true,
    }
}

/// How a confirmation is asked: a plain dialog, or typing the connection name.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Alert,
    Safe,
}

/// Which statements a connection confirms before running them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Nothing,
    Writes,
    Everything,
}

/// The connection's safe mode; `Auto` confirms writes on production connections only.
pub fn policy(conn: &Connection) -> (Scope, Level) {
    match conn.options.safe_mode {
        SafeMode::Auto if conn.is_production() => (Scope::Writes, Level::Alert),
        SafeMode::Auto | SafeMode::Silent => (Scope::Nothing, Level::Alert),
        SafeMode::AlertWrites => (Scope::Writes, Level::Alert),
        SafeMode::AlertAll => (Scope::Everything, Level::Alert),
        SafeMode::SafeWrites => (Scope::Writes, Level::Safe),
        SafeMode::SafeAll => (Scope::Everything, Level::Safe),
    }
}

/// The statements that need confirmation on this connection (empty = run freely).
pub fn needing_confirmation(conn: &Connection, statements: &[String]) -> Vec<Classified> {
    let (scope, _) = policy(conn);
    if scope == Scope::Nothing {
        return Vec::new();
    }
    statements
        .iter()
        .map(|s| classify(conn.driver, s))
        .filter(|c| scope == Scope::Everything || c.kind == Kind::Write)
        .collect()
}

/// The writes a read-only connection refuses (session statements such as SET are fine).
pub fn read_only_violations(conn: &Connection, statements: &[String]) -> Vec<Classified> {
    if !conn.options.read_only {
        return Vec::new();
    }
    statements
        .iter()
        .map(|s| classify(conn.driver, s))
        .filter(|c| c.kind == Kind::Write)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(driver: Driver, sql: &str) -> Kind {
        classify(driver, sql).kind
    }

    #[test]
    fn reads() {
        assert_eq!(
            k(Driver::Postgres, "SELECT * FROM t WHERE id = 1"),
            Kind::Read
        );
        assert_eq!(
            k(Driver::Postgres, "WITH x AS (SELECT 1) SELECT * FROM x"),
            Kind::Read
        );
        assert_eq!(k(Driver::Postgres, "EXPLAIN SELECT 1"), Kind::Read);
        assert_eq!(k(Driver::Mysql, "SHOW TABLES"), Kind::Read);
        assert_eq!(k(Driver::Mysql, "DESCRIBE users"), Kind::Read);
        assert_eq!(k(Driver::Sqlite, "PRAGMA table_info('t')"), Kind::Read);
        assert_eq!(k(Driver::Sqlite, "PRAGMA user_version"), Kind::Read);
        assert_eq!(k(Driver::Postgres, "SELECT 1 UNION SELECT 2"), Kind::Read);
    }

    #[test]
    fn writes() {
        assert_eq!(k(Driver::Postgres, "UPDATE t SET a = 1"), Kind::Write);
        assert_eq!(k(Driver::Postgres, "DELETE FROM t"), Kind::Write);
        assert_eq!(k(Driver::Mysql, "INSERT INTO t VALUES (1)"), Kind::Write);
        assert_eq!(k(Driver::Postgres, "DROP TABLE t"), Kind::Write);
        assert_eq!(k(Driver::Postgres, "TRUNCATE t"), Kind::Write);
        assert_eq!(
            k(
                Driver::Postgres,
                "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d"
            ),
            Kind::Write
        );
        assert_eq!(
            k(Driver::Postgres, "SELECT * INTO backup FROM t"),
            Kind::Write
        );
        assert_eq!(
            k(Driver::Postgres, "EXPLAIN ANALYZE DELETE FROM t"),
            Kind::Write
        );
        assert_eq!(
            k(
                Driver::Postgres,
                "EXPLAIN (ANALYZE, BUFFERS) DELETE FROM orders"
            ),
            Kind::Write
        );
        assert_eq!(
            k(Driver::Postgres, "EXPLAIN DELETE FROM orders"),
            Kind::Write
        );
        assert_eq!(k(Driver::Mysql, "SET @@GLOBAL.read_only = 0"), Kind::Write);
        assert_eq!(
            k(Driver::Mysql, "SET GLOBAL max_connections = 1"),
            Kind::Write
        );
        assert_eq!(
            k(Driver::Mysql, "SET PERSIST max_connections = 1"),
            Kind::Write
        );
        assert_eq!(k(Driver::Sqlite, "PRAGMA foreign_keys = OFF"), Kind::Write);
    }

    #[test]
    fn session_statements_and_unparseable() {
        assert_eq!(k(Driver::Postgres, "SET search_path TO app"), Kind::Session);
        assert_eq!(k(Driver::Mysql, "USE shop"), Kind::Session);
        assert_eq!(k(Driver::Postgres, "BEGIN"), Kind::Session);
        assert_eq!(k(Driver::Mysql, "SET @x = 1"), Kind::Session);
        // MySQL runs what sqlparser treats as comments.
        assert_eq!(
            classify(Driver::Mysql, "SELECT 1 --1; DELETE FROM t").kind,
            Kind::Write
        );
        assert_eq!(
            classify(Driver::Mysql, "/*!DELETE FROM t WHERE*/ (SELECT 1)").kind,
            Kind::Write
        );
        assert_eq!(k(Driver::Mysql, "SELECT 1 -- a real comment"), Kind::Read);
        let c = classify(Driver::Postgres, "VACUUM FULL ANALYZE !!!");
        assert_eq!((c.kind, c.understood), (Kind::Write, false));
        // The parser can't read bare identifiers in PRAGMA: fail safe, ask for confirmation.
        let p = classify(Driver::Sqlite, "PRAGMA table_info(t)");
        assert_eq!((p.kind, p.understood), (Kind::Write, false));
    }

    #[test]
    fn only_production_needs_confirmation() {
        let mut conn: Connection = serde_json::from_value(serde_json::json!({
            "id": "x", "name": "n", "driver": "postgres", "tag": "staging"
        }))
        .unwrap();
        let stmts = vec!["select 1".to_string(), "delete from t".to_string()];
        assert!(needing_confirmation(&conn, &stmts).is_empty());
        conn.tag = crate::models::Tag::Production;
        let need = needing_confirmation(&conn, &stmts);
        assert_eq!(need.len(), 1);
        assert_eq!(need[0].sql, "delete from t");
    }

    fn staging() -> Connection {
        serde_json::from_value(serde_json::json!({
            "id": "x", "name": "n", "driver": "postgres", "tag": "staging"
        }))
        .unwrap()
    }

    #[test]
    fn safe_mode_levels() {
        let stmts = vec!["select 1".to_string(), "delete from t".to_string()];
        let mut conn = staging();
        let count = |c: &Connection| needing_confirmation(c, &stmts).len();
        conn.options.safe_mode = SafeMode::AlertWrites;
        assert_eq!((count(&conn), policy(&conn).1), (1, Level::Alert));
        conn.options.safe_mode = SafeMode::SafeAll;
        assert_eq!((count(&conn), policy(&conn).1), (2, Level::Safe));
        conn.tag = crate::models::Tag::Production;
        conn.options.safe_mode = SafeMode::Silent;
        assert_eq!(count(&conn), 0);
    }

    #[test]
    fn read_only_refuses_writes_only() {
        let mut conn = staging();
        let stmts = vec![
            "select 1".to_string(),
            "set search_path to app".to_string(),
            "update t set a = 1".to_string(),
        ];
        assert!(read_only_violations(&conn, &stmts).is_empty());
        conn.options.read_only = true;
        let refused = read_only_violations(&conn, &stmts);
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].sql, "update t set a = 1");
    }
}
