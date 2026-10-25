//! The privileges each kind of object accepts. They are keywords, not values, so they
//! can't be bound or quoted: anything outside these lists is refused.

use serde::Serialize;

use crate::models::Dialect;

use super::TargetKind;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrivilegeOption {
    pub target: TargetKind,
    pub privileges: Vec<&'static str>,
}

const PG_DATABASE: &[&str] = &["CONNECT", "CREATE", "TEMPORARY", "ALL PRIVILEGES"];
const PG_SCHEMA: &[&str] = &["USAGE", "CREATE", "ALL PRIVILEGES"];
const PG_TABLE: &[&str] = &[
    "SELECT",
    "INSERT",
    "UPDATE",
    "DELETE",
    "TRUNCATE",
    "REFERENCES",
    "TRIGGER",
    "ALL PRIVILEGES",
];

const MY_TABLE: &[&str] = &[
    "SELECT",
    "INSERT",
    "UPDATE",
    "DELETE",
    "CREATE",
    "DROP",
    "INDEX",
    "ALTER",
    "REFERENCES",
    "TRIGGER",
    "CREATE VIEW",
    "SHOW VIEW",
    "ALL PRIVILEGES",
];

const MY_DATABASE: &[&str] = &[
    "SELECT",
    "INSERT",
    "UPDATE",
    "DELETE",
    "CREATE",
    "DROP",
    "INDEX",
    "ALTER",
    "REFERENCES",
    "TRIGGER",
    "CREATE VIEW",
    "SHOW VIEW",
    "CREATE TEMPORARY TABLES",
    "LOCK TABLES",
    "EXECUTE",
    "CREATE ROUTINE",
    "ALTER ROUTINE",
    "EVENT",
    "ALL PRIVILEGES",
];

/// `mysql.user` column → privilege, for the global privileges of each account.
pub const MY_GLOBAL_COLUMNS: &[(&str, &str)] = &[
    ("Select_priv", "SELECT"),
    ("Insert_priv", "INSERT"),
    ("Update_priv", "UPDATE"),
    ("Delete_priv", "DELETE"),
    ("Create_priv", "CREATE"),
    ("Drop_priv", "DROP"),
    ("Reload_priv", "RELOAD"),
    ("Shutdown_priv", "SHUTDOWN"),
    ("Process_priv", "PROCESS"),
    ("File_priv", "FILE"),
    ("References_priv", "REFERENCES"),
    ("Index_priv", "INDEX"),
    ("Alter_priv", "ALTER"),
    ("Show_db_priv", "SHOW DATABASES"),
    ("Super_priv", "SUPER"),
    ("Create_tmp_table_priv", "CREATE TEMPORARY TABLES"),
    ("Lock_tables_priv", "LOCK TABLES"),
    ("Execute_priv", "EXECUTE"),
    ("Repl_slave_priv", "REPLICATION SLAVE"),
    ("Repl_client_priv", "REPLICATION CLIENT"),
    ("Create_view_priv", "CREATE VIEW"),
    ("Show_view_priv", "SHOW VIEW"),
    ("Create_routine_priv", "CREATE ROUTINE"),
    ("Alter_routine_priv", "ALTER ROUTINE"),
    ("Create_user_priv", "CREATE USER"),
    ("Event_priv", "EVENT"),
    ("Trigger_priv", "TRIGGER"),
    ("Grant_priv", "GRANT OPTION"),
];

/// Global privileges that can be granted (GRANT OPTION goes through WITH GRANT OPTION).
fn my_global() -> Vec<&'static str> {
    MY_GLOBAL_COLUMNS
        .iter()
        .map(|(_, p)| *p)
        .filter(|p| *p != "GRANT OPTION")
        .chain(["ALL PRIVILEGES"])
        .collect()
}

/// The privileges `kind` accepts on `driver`; None when the driver has no such target.
pub fn allowed(driver: Dialect, kind: TargetKind) -> Option<Vec<&'static str>> {
    use TargetKind::*;
    Some(match (driver, kind) {
        (Dialect::Postgres, Database) => PG_DATABASE.to_vec(),
        (Dialect::Postgres, Schema) => PG_SCHEMA.to_vec(),
        (Dialect::Postgres, AllTables | Table) => PG_TABLE.to_vec(),
        (Dialect::Mysql, Global) => my_global(),
        (Dialect::Mysql, Database) => MY_DATABASE.to_vec(),
        (Dialect::Mysql, Table) => MY_TABLE.to_vec(),
        _ => return None,
    })
}

pub fn privilege_options(driver: Dialect) -> Vec<PrivilegeOption> {
    use TargetKind::*;
    [Global, Database, Schema, AllTables, Table]
        .into_iter()
        .filter_map(|target| {
            allowed(driver, target).map(|privileges| PrivilegeOption { target, privileges })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_per_driver() {
        let kinds =
            |d| -> Vec<TargetKind> { privilege_options(d).into_iter().map(|o| o.target).collect() };
        use TargetKind::*;
        assert_eq!(
            kinds(Dialect::Postgres),
            vec![Database, Schema, AllTables, Table]
        );
        assert_eq!(kinds(Dialect::Mysql), vec![Global, Database, Table]);
        assert!(kinds(Dialect::Sqlite).is_empty());
        let global = allowed(Dialect::Mysql, Global).unwrap();
        assert!(global.contains(&"PROCESS") && !global.contains(&"GRANT OPTION"));
    }
}
