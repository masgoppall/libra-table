//! Statements built for user changes: masking, escaping, diffs and validation.

mod postgres_tests {
    use super::super::postgres::plan;
    use super::super::{GrantTarget, Grantee, PgRole, RoleSpec, UserChange};

    fn spec(name: &str) -> RoleSpec {
        RoleSpec {
            name: name.into(),
            login: true,
            ..Default::default()
        }
    }

    #[test]
    fn create_masks_the_password_and_escapes_it() {
        let role = RoleSpec {
            conn_limit: Some(3),
            member_of: vec!["readers".into()],
            ..spec("app")
        };
        let change = UserChange::CreateRole {
            role,
            password: Some(r"it's\secret".into()),
        };
        let s = plan(&change, None).unwrap();
        let head = "CREATE ROLE \"app\" WITH LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE \
            NOREPLICATION CONNECTION LIMIT 3 PASSWORD";
        assert_eq!(s[0].shown, format!("{head} '********'"));
        assert_eq!(s[0].run, format!(r"{head} E'it''s\\secret'"));
        assert!(!s[0].shown.contains("secret"));
        assert_eq!(s[1].run, r#"GRANT "readers" TO "app""#);
    }

    #[test]
    fn alter_emits_only_changes() {
        let cur = PgRole {
            name: "app".into(),
            login: true,
            superuser: false,
            create_db: false,
            create_role: false,
            replication: false,
            conn_limit: -1,
            valid_until: None,
            member_of: vec!["old".into()],
        };
        let role = RoleSpec {
            create_db: true,
            member_of: vec!["new".into()],
            ..spec("app2")
        };
        let change = UserChange::AlterRole {
            name: "app".into(),
            role,
            password: None,
        };
        let shown: Vec<String> = plan(&change, Some(&cur))
            .unwrap()
            .into_iter()
            .map(|s| s.shown)
            .collect();
        assert_eq!(
            shown,
            vec![
                r#"ALTER ROLE "app" RENAME TO "app2""#,
                r#"ALTER ROLE "app2" WITH CREATEDB"#,
                r#"GRANT "new" TO "app2""#,
                r#"REVOKE "old" FROM "app2""#,
            ]
        );
    }

    #[test]
    fn grants_validate_privileges() {
        let grantee = Grantee {
            name: "app".into(),
            host: None,
        };
        let grant = |privileges: Vec<&str>, target| UserChange::Grant {
            grantee: grantee.clone(),
            privileges: privileges.into_iter().map(String::from).collect(),
            target,
            with_grant_option: false,
        };
        let all = GrantTarget::AllTables {
            schema: "public".into(),
        };
        let s = plan(&grant(vec!["SELECT", "UPDATE"], all.clone()), None).unwrap();
        assert_eq!(
            s[0].run,
            r#"GRANT SELECT, UPDATE ON ALL TABLES IN SCHEMA "public" TO "app""#
        );
        assert!(plan(&grant(vec!["SELECT; DROP TABLE x"], all), None).is_err());
        let db = GrantTarget::Database { name: "d".into() };
        assert!(plan(&grant(vec!["SELECT"], db), None).is_err());
    }
}

mod mysql_tests {
    use super::super::mysql::{list_sql, plan, Quoting};
    use super::super::{GrantTarget, Grantee, UserChange};

    fn grantee() -> Grantee {
        Grantee {
            name: "app".into(),
            host: Some("localhost".into()),
        }
    }

    #[test]
    fn passwords_are_masked_and_escaped_for_the_mode() {
        let change = UserChange::CreateUser {
            user: "app".into(),
            host: "".into(),
            password: Some(r"a'b\c".into()),
        };
        let s = &plan(&change, Quoting::default()).unwrap()[0];
        assert_eq!(s.shown, "CREATE USER `app`@`%` IDENTIFIED BY '********'");
        assert_eq!(s.run, r"CREATE USER `app`@`%` IDENTIFIED BY 'a''b\\c'");
        let plain = Quoting {
            backslash_escapes: false,
        };
        let s = &plan(&change, plain).unwrap()[0];
        assert_eq!(s.run, r"CREATE USER `app`@`%` IDENTIFIED BY 'a''b\c'");
        let set = UserChange::SetPassword {
            grantee: grantee(),
            password: "x".into(),
        };
        let s = &plan(&set, plain).unwrap()[0];
        assert_eq!(
            s.shown,
            "ALTER USER `app`@`localhost` IDENTIFIED BY '********'"
        );
    }

    #[test]
    fn grant_targets() {
        let grant = |target| UserChange::Grant {
            grantee: grantee(),
            privileges: vec!["SELECT".into(), "INSERT".into()],
            target,
            with_grant_option: true,
        };
        let run = |c: UserChange| plan(&c, Quoting::default()).map(|s| s[0].run.clone());
        assert_eq!(
            run(grant(GrantTarget::Global)).unwrap(),
            "GRANT SELECT, INSERT ON *.* TO `app`@`localhost` WITH GRANT OPTION"
        );
        let table = GrantTarget::Table {
            schema: "shop".into(),
            name: "or`ders".into(),
        };
        assert_eq!(
            run(grant(table)).unwrap(),
            "GRANT SELECT, INSERT ON `shop`.`or``ders` TO `app`@`localhost` WITH GRANT OPTION"
        );
        let schema = GrantTarget::Schema { name: "x".into() };
        assert!(run(grant(schema)).is_err());
        let revoke = UserChange::Revoke {
            grantee: grantee(),
            privileges: vec!["DROP DATABASE".into()],
            target: GrantTarget::Database {
                name: "shop".into(),
            },
        };
        assert!(run(revoke).is_err());
        assert!(list_sql().contains("IF(Process_priv = 'Y', 'PROCESS', NULL)"));
    }

    #[test]
    fn all_privileges_stands_alone() {
        let grant = UserChange::Grant {
            grantee: grantee(),
            privileges: vec!["SELECT".into(), "ALL PRIVILEGES".into()],
            target: GrantTarget::Global,
            with_grant_option: false,
        };
        let e = plan(&grant, Quoting::default()).unwrap_err();
        assert!(e.message.contains("choose it alone"), "{}", e.message);
    }
}
