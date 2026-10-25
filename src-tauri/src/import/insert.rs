//! Multi-row INSERT statements for imports. Values are always bind parameters; on
//! Postgres each one is cast to its column type (`$1::integer`) because they are bound
//! as text. Rows per statement stay under each driver's bind parameter limit.

use crate::db::meta::ColumnInfo;
use crate::db::sql::{qualified, quote_ident, Placeholder};
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

/// Postgres and MySQL allow 65,535 parameters per statement, SQLite 32,766 (3.32+).
fn max_params(driver: Dialect) -> usize {
    match driver {
        Dialect::Sqlite => 32_000,
        _ => 65_000,
    }
}

#[derive(Debug, Clone)]
struct TargetColumn {
    quoted: String,
    /// Postgres cast (`::integer`), empty elsewhere.
    cast: String,
    /// MySQL/SQLite integer or boolean column: `true`/`false` become 1/0.
    boolish: bool,
}

/// The table an import writes to and the columns it fills, in mapping order.
#[derive(Debug, Clone)]
pub struct Target {
    driver: Dialect,
    table: String,
    columns: Vec<TargetColumn>,
}

fn is_boolish(data_type: &str) -> bool {
    let t = data_type.to_ascii_lowercase();
    t.contains("int") || t.contains("bool") || t == "bit(1)"
}

impl Target {
    /// `names` are the target columns; `catalog` describes the table's columns (only
    /// name and type are used).
    pub fn new(
        driver: Dialect,
        schema: &str,
        table: &str,
        names: &[String],
        catalog: &[ColumnInfo],
    ) -> AppResult<Self> {
        if names.is_empty() {
            return Err(AppError::validation("Map at least one column to import."));
        }
        let mut columns = Vec::with_capacity(names.len());
        for (i, name) in names.iter().enumerate() {
            if names[..i].contains(name) {
                return Err(AppError::validation(format!(
                    "Column \"{name}\" is mapped twice."
                )));
            }
            let col = catalog.iter().find(|c| &c.name == name).ok_or_else(|| {
                AppError::validation(format!("Table \"{table}\" has no column \"{name}\"."))
            })?;
            columns.push(TargetColumn {
                quoted: quote_ident(driver, name),
                cast: match driver {
                    Dialect::Postgres => format!("::{}", col.data_type),
                    _ => String::new(),
                },
                boolish: driver != Dialect::Postgres && is_boolish(&col.data_type),
            });
        }
        Ok(Self {
            driver,
            table: qualified(driver, schema, table),
            columns,
        })
    }

    /// Rows in one INSERT: the batch size, capped by the parameter limit.
    pub fn rows_per_statement(&self, batch_size: usize) -> usize {
        let by_params = (max_params(self.driver) / self.columns.len()).max(1);
        batch_size.clamp(1, by_params)
    }

    fn head(&self) -> String {
        let names: Vec<&str> = self.columns.iter().map(|c| c.quoted.as_str()).collect();
        format!("INSERT INTO {} ({}) VALUES ", self.table, names.join(", "))
    }

    fn tuple(&self, ph: &mut Placeholder) -> String {
        let vals: Vec<String> = self
            .columns
            .iter()
            .map(|c| format!("{}{}", ph.next_param(), c.cast))
            .collect();
        format!("({})", vals.join(", "))
    }

    /// `INSERT INTO t (a, b) VALUES ($1::int, $2::text), ($3::int, $4::text)` for `rows`.
    pub fn sql(&self, rows: usize) -> String {
        let mut ph = Placeholder::new(self.driver, 1);
        let tuples: Vec<String> = (0..rows).map(|_| self.tuple(&mut ph)).collect();
        format!("{}{}", self.head(), tuples.join(", "))
    }

    /// The statement shape for previews: two rows, then an ellipsis.
    pub fn template(&self, rows: usize) -> String {
        let shown = rows.min(2);
        let mut sql = self.sql(shown);
        if rows > shown {
            sql.push_str(", …");
        }
        sql
    }

    /// The bind values of `rows`, flattened in placeholder order.
    pub fn binds(&self, rows: &[Vec<Option<String>>]) -> Vec<Option<String>> {
        let mut out = Vec::with_capacity(rows.len() * self.columns.len());
        for row in rows {
            for (i, c) in self.columns.iter().enumerate() {
                let v = row.get(i).cloned().flatten();
                out.push(if c.boolish { v.map(bool_to_int) } else { v });
            }
        }
        out
    }
}

fn bool_to_int(v: String) -> String {
    match v.to_ascii_lowercase().as_str() {
        "true" | "t" | "yes" => "1".into(),
        "false" | "f" | "no" => "0".into(),
        _ => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, data_type: &str) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            data_type: data_type.into(),
            nullable: true,
            default: None,
            pk_position: None,
            auto_increment: false,
            comment: None,
        }
    }

    fn names(n: &[&str]) -> Vec<String> {
        n.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn builds_multi_row_inserts_with_casts() {
        let cat = vec![col("id", "integer"), col("name", "text")];
        let t = Target::new(
            Dialect::Postgres,
            "public",
            "p",
            &names(&["id", "name"]),
            &cat,
        )
        .unwrap();
        assert_eq!(
            t.sql(2),
            "INSERT INTO \"public\".\"p\" (\"id\", \"name\") VALUES \
             ($1::integer, $2::text), ($3::integer, $4::text)"
        );
        assert!(t.template(500).ends_with("($3::integer, $4::text), …"));
        let m = Target::new(Dialect::Mysql, "shop", "p", &names(&["name"]), &cat).unwrap();
        assert_eq!(m.sql(2), "INSERT INTO `shop`.`p` (`name`) VALUES (?), (?)");
    }

    #[test]
    fn binds_follow_the_mapping_and_convert_booleans() {
        let cat = vec![col("flag", "tinyint(1)"), col("name", "varchar(10)")];
        let t = Target::new(Dialect::Mysql, "", "p", &names(&["flag", "name"]), &cat).unwrap();
        let rows = vec![
            vec![Some("true".into()), Some("true".into())],
            vec![None, Some("x".into())],
        ];
        assert_eq!(
            t.binds(&rows),
            vec![
                Some("1".into()),
                Some("true".into()),
                None,
                Some("x".into())
            ]
        );
    }

    #[test]
    fn caps_rows_by_parameter_limit() {
        let cat: Vec<ColumnInfo> = (0..100).map(|i| col(&format!("c{i}"), "TEXT")).collect();
        let all: Vec<String> = cat.iter().map(|c| c.name.clone()).collect();
        let t = Target::new(Dialect::Sqlite, "main", "t", &all, &cat).unwrap();
        assert_eq!(t.rows_per_statement(1000), 320);
        assert_eq!(t.rows_per_statement(50), 50);
        assert_eq!(t.rows_per_statement(0), 1);
    }

    #[test]
    fn rejects_unknown_and_duplicate_columns() {
        let cat = vec![col("a", "text")];
        assert!(Target::new(Dialect::Sqlite, "main", "t", &names(&["b"]), &cat).is_err());
        assert!(Target::new(Dialect::Sqlite, "main", "t", &names(&["a", "a"]), &cat).is_err());
        assert!(Target::new(Dialect::Sqlite, "main", "t", &[], &cat).is_err());
    }
}
