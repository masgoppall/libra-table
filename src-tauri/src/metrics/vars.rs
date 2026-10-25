//! `{{name}}` variables in widget SQL, filled from the board's input widgets.
//!
//! Values become quoted SQL literals, not bind parameters: they are typed as text, and
//! a text bind compared with an integer or date column is an error on Postgres
//! (`integer = text`), while a quoted literal is untyped there and coerces like any
//! literal the user would type. Quoting is the driver's own (`''` doubling; backslashes
//! escaped on MySQL; an `E''` string on Postgres when the value has a backslash, which is
//! safe whatever `standard_conforming_strings` says). Number inputs must parse as a
//! number and go in bare. Placeholders inside strings, quoted names and comments are
//! left alone.

use serde::Deserialize;

use crate::db::sql::{literal, mysql_literal};
use crate::error::{AppError, AppResult};
use crate::models::Driver;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VarKind {
    #[default]
    Text,
    Number,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub kind: VarKind,
}

pub fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_number(v: &str) -> bool {
    let body = v.strip_prefix('-').unwrap_or(v);
    let (int, frac) = body.split_once('.').unwrap_or((body, "0"));
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    digits(int) && digits(frac)
}

fn quote(driver: Driver, v: &str) -> String {
    match driver {
        Driver::Mysql => mysql_literal(v),
        Driver::Postgres if v.contains('\\') => {
            format!("E'{}'", v.replace('\\', "\\\\").replace('\'', "''"))
        }
        _ => literal(v),
    }
}

fn render(driver: Driver, var: &Variable) -> AppResult<String> {
    if var.value.contains('\0') {
        return Err(AppError::validation("Variable values can't contain NUL."));
    }
    match var.kind {
        VarKind::Number if var.value.trim().is_empty() => Ok("NULL".into()),
        VarKind::Number if is_number(var.value.trim()) => Ok(var.value.trim().to_string()),
        VarKind::Number => Err(AppError::validation(format!(
            "{{{{{}}}}} must be a number.",
            var.name
        ))),
        VarKind::Text => Ok(quote(driver, &var.value)),
    }
}

/// End of a quoted run or comment starting at `i`, or None when `i` starts neither.
fn skip(driver: Driver, s: &[char], i: usize) -> Option<usize> {
    let next = s.get(i + 1).copied();
    let close = |from: usize, pat: &[char]| {
        (from..s.len())
            .find(|&j| s[j..].starts_with(pat))
            .map_or(s.len(), |j| j + pat.len())
    };
    match s[i] {
        q @ ('\'' | '"') => Some(close(i + 1, &[q])),
        '`' if driver == Driver::Mysql => Some(close(i + 1, &['`'])),
        '-' if next == Some('-') => Some(close(i, &['\n'])),
        '#' if driver == Driver::Mysql => Some(close(i, &['\n'])),
        '/' if next == Some('*') => Some(close(i + 2, &['*', '/'])),
        _ => None,
    }
}

/// `{{ name }}` at `i`: the name and the index after it.
fn placeholder(s: &[char], i: usize) -> Option<(String, usize)> {
    if !s[i..].starts_with(&['{', '{']) {
        return None;
    }
    let end = (i + 2..s.len().saturating_sub(1)).find(|&j| s[j] == '}' && s[j + 1] == '}')?;
    let name: String = s[i + 2..end].iter().collect::<String>().trim().to_string();
    is_name(&name).then_some((name, end + 2))
}

/// Replaces every `{{name}}` outside strings and comments with its value.
pub fn substitute(driver: Driver, sql: &str, vars: &[Variable]) -> AppResult<String> {
    let s: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < s.len() {
        if let Some(end) = skip(driver, &s, i) {
            out.extend(&s[i..end]);
            i = end;
        } else if let Some((name, end)) = placeholder(&s, i) {
            let var = vars.iter().find(|v| v.name == name).ok_or_else(|| {
                AppError::validation(format!("No input widget defines {{{{{name}}}}}."))
            })?;
            out.push_str(&render(driver, var)?);
            i = end;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str, value: &str, kind: VarKind) -> Variable {
        Variable {
            name: name.into(),
            value: value.into(),
            kind,
        }
    }

    #[test]
    fn fills_values_as_literals() {
        let v = [
            var("region", "O'Hara", VarKind::Text),
            var("n", "10", VarKind::Number),
        ];
        let sql = "select * from t where r = {{region}} and x > {{ n }} limit {{n}}";
        assert_eq!(
            substitute(Driver::Postgres, sql, &v).unwrap(),
            "select * from t where r = 'O''Hara' and x > 10 limit 10"
        );
        let slash = [var("p", "a\\'", VarKind::Text)];
        assert_eq!(
            substitute(Driver::Postgres, "{{p}}", &slash).unwrap(),
            "E'a\\\\'''"
        );
        assert_eq!(
            substitute(Driver::Mysql, "{{p}}", &slash).unwrap(),
            "'a\\\\'''"
        );
        assert_eq!(
            substitute(Driver::Sqlite, "{{p}}", &slash).unwrap(),
            "'a\\'''"
        );
    }

    #[test]
    fn leaves_strings_and_comments_alone() {
        let v = [var("x", "1", VarKind::Number)];
        let sql = "select '{{x}}', \"{{x}}\" -- {{x}}\n, {{x}} /* {{x}} */";
        assert_eq!(
            substitute(Driver::Postgres, sql, &v).unwrap(),
            "select '{{x}}', \"{{x}}\" -- {{x}}\n, 1 /* {{x}} */"
        );
        assert_eq!(
            substitute(Driver::Sqlite, "{{ not a name }}", &v).unwrap(),
            "{{ not a name }}"
        );
    }

    #[test]
    fn rejects_unknown_names_and_bad_numbers() {
        let v = [var("n", "1; drop table t", VarKind::Number)];
        assert!(substitute(Driver::Mysql, "{{n}}", &v).is_err());
        assert!(substitute(Driver::Mysql, "{{missing}}", &v).is_err());
        let empty = [var("n", "", VarKind::Number)];
        assert_eq!(substitute(Driver::Mysql, "{{n}}", &empty).unwrap(), "NULL");
    }
}
