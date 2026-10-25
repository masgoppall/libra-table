//! Dialect helpers: identifier quoting, placeholders, qualified names, literal rendering.

use crate::models::Driver;

pub fn quote_ident(driver: Driver, name: &str) -> String {
    match driver {
        Driver::Mysql => format!("`{}`", name.replace('`', "``")),
        Driver::Postgres | Driver::Sqlite => format!("\"{}\"", name.replace('"', "\"\"")),
    }
}

/// `schema.table` quoted for the driver. An empty schema gives just the table.
pub fn qualified(driver: Driver, schema: &str, table: &str) -> String {
    if schema.is_empty() {
        quote_ident(driver, table)
    } else {
        format!(
            "{}.{}",
            quote_ident(driver, schema),
            quote_ident(driver, table)
        )
    }
}

/// Hands out `$1, $2, …` on Postgres and `?` elsewhere.
pub struct Placeholder {
    driver: Driver,
    next: usize,
}

impl Placeholder {
    pub fn new(driver: Driver, first: usize) -> Self {
        Self {
            driver,
            next: first.max(1),
        }
    }

    pub fn next_param(&mut self) -> String {
        let n = self.next;
        self.next += 1;
        match self.driver {
            Driver::Postgres => format!("${n}"),
            _ => "?".to_string(),
        }
    }

    pub fn peek_index(&self) -> usize {
        self.next
    }
}

/// A string literal for previews (never executed with user data).
pub fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// A MySQL string literal: backslash is an escape character there by default.
pub fn mysql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''"))
}

/// A string literal for DDL written by LibraTable (comments), escaped for the driver.
pub fn literal_for(driver: Driver, value: &str) -> String {
    if driver == Driver::Mysql {
        mysql_literal(value)
    } else {
        literal(value)
    }
}

/// Fills placeholders with literals for display, in one pass that skips strings and
/// quoted identifiers, so a `$1` or `?` inside a value or a column name is left alone.
/// For previews only: what runs is the parameterized statement.
pub fn inline_binds(driver: Driver, sql: &str, binds: &[Option<String>]) -> String {
    let lit = |i: usize| match binds.get(i) {
        Some(Some(v)) => literal(v),
        Some(None) => "NULL".to_string(),
        None => "?".to_string(),
    };
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len() + binds.len() * 8);
    let mut i = 0;
    let mut next_q = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' || c == '`' {
            i = copy_quoted(&chars, i, &mut out);
            continue;
        }
        if driver == Driver::Postgres && c == '$' {
            if let Some((n, end)) = dollar_param(&chars, i) {
                out.push_str(&lit(n.saturating_sub(1)));
                i = end;
                continue;
            }
        }
        if driver != Driver::Postgres && c == '?' {
            out.push_str(&lit(next_q));
            next_q += 1;
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Copies the quoted run starting at `start` verbatim (a doubled quote is an escaped
/// quote). Returns the index after it.
fn copy_quoted(chars: &[char], start: usize, out: &mut String) -> usize {
    let q = chars[start];
    out.push(q);
    let mut i = start + 1;
    while i < chars.len() {
        out.push(chars[i]);
        if chars[i] == q {
            if chars.get(i + 1) == Some(&q) {
                out.push(q);
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

/// A Postgres `$n` placeholder at `start`: its number and the index after it.
fn dollar_param(chars: &[char], start: usize) -> Option<(usize, usize)> {
    let mut j = start + 1;
    while j < chars.len() && chars[j].is_ascii_digit() {
        j += 1;
    }
    if j == start + 1 {
        return None;
    }
    let n = chars[start + 1..j]
        .iter()
        .collect::<String>()
        .parse()
        .unwrap_or(0);
    Some((n, j))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_doubles_the_quote_char() {
        assert_eq!(quote_ident(Driver::Postgres, r#"we"ird"#), r#""we""ird""#);
        assert_eq!(quote_ident(Driver::Mysql, "we`ird"), "`we``ird`");
        assert_eq!(qualified(Driver::Sqlite, "main", "t"), r#""main"."t""#);
        assert_eq!(qualified(Driver::Mysql, "", "t"), "`t`");
    }

    #[test]
    fn inline_binds_skips_quoted_text() {
        let b = vec![Some("$2".to_string()), Some("x".to_string())];
        // The value "$2" must not be replaced again by the second bind.
        assert_eq!(
            inline_binds(
                Driver::Postgres,
                r#"SELECT "a$1" FROM t WHERE a = $1 AND b = $2"#,
                &b
            ),
            r#"SELECT "a$1" FROM t WHERE a = '$2' AND b = 'x'"#
        );
        // A `?` inside a MySQL identifier doesn't shift the values.
        assert_eq!(
            inline_binds(
                Driver::Mysql,
                "UPDATE t SET `odd?` = ? WHERE id = ?",
                &[Some("v".into()), None]
            ),
            "UPDATE t SET `odd?` = 'v' WHERE id = NULL"
        );
        assert_eq!(
            inline_binds(Driver::Sqlite, "x LIKE ? ESCAPE '!'", &[Some("a?".into())]),
            "x LIKE 'a?' ESCAPE '!'"
        );
    }

    #[test]
    fn placeholders() {
        let mut p = Placeholder::new(Driver::Postgres, 2);
        assert_eq!(
            (p.next_param(), p.next_param()),
            ("$2".to_string(), "$3".to_string())
        );
        let mut q = Placeholder::new(Driver::Mysql, 1);
        assert_eq!(q.next_param(), "?");
    }
}
