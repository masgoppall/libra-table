//! Allowed values of enum columns, for the grid's dropdown editor: Postgres enum types
//! (`pg_enum`) and MySQL `ENUM(…)` / `SET(…)` column types.

use std::collections::BTreeMap;

use crate::db::DbPool;
use crate::error::AppResult;

const PG_ENUMS: &str = "SELECT a.attname::text, e.enumlabel::text \
    FROM pg_attribute a \
    JOIN pg_class c ON c.oid = a.attrelid \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    JOIN pg_enum e ON e.enumtypid = a.atttypid \
    WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped \
    ORDER BY a.attnum, e.enumsortorder";

const MYSQL_ENUMS: &str = "SELECT CAST(COLUMN_NAME AS CHAR), CAST(COLUMN_TYPE AS CHAR) \
    FROM information_schema.COLUMNS \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND DATA_TYPE IN ('enum', 'set') \
    ORDER BY ORDINAL_POSITION";

/// Column name → its allowed values. Columns without a fixed set are absent.
pub async fn column_choices(
    pool: &DbPool,
    schema: &str,
    table: &str,
) -> AppResult<BTreeMap<String, Vec<String>>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    match pool {
        DbPool::Postgres(p) => {
            let rows: Vec<(String, String)> = sqlx::query_as(PG_ENUMS)
                .bind(schema)
                .bind(table)
                .fetch_all(p)
                .await?;
            for (column, label) in rows {
                out.entry(column).or_default().push(label);
            }
        }
        DbPool::Mysql(p) => {
            let rows: Vec<(String, String)> = sqlx::query_as(MYSQL_ENUMS)
                .bind(schema)
                .bind(table)
                .fetch_all(p)
                .await?;
            for (column, column_type) in rows {
                out.insert(column, mysql_enum_values(&column_type));
            }
        }
        DbPool::Sqlite(_) => {}
    }
    Ok(out)
}

/// `enum('a','it''s')` → `["a", "it's"]` (also for `set(…)`).
pub fn mysql_enum_values(column_type: &str) -> Vec<String> {
    let Some(start) = column_type.find('(') else {
        return Vec::new();
    };
    let body = column_type[start + 1..].trim_end_matches(')');
    let mut values = Vec::new();
    let mut current = String::new();
    let mut chars = body.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match (ch, quoted) {
            ('\'', false) => quoted = true,
            ('\'', true) if chars.peek() == Some(&'\'') => {
                chars.next();
                current.push('\'');
            }
            ('\'', true) => {
                quoted = false;
                values.push(std::mem::take(&mut current));
            }
            (_, true) => current.push(ch),
            _ => {}
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mysql_enum_types() {
        assert_eq!(mysql_enum_values("enum('a','b')"), vec!["a", "b"]);
        assert_eq!(
            mysql_enum_values("set('x','it''s, ok')"),
            vec!["x", "it's, ok"]
        );
        assert!(mysql_enum_values("varchar(10)").is_empty());
    }
}
