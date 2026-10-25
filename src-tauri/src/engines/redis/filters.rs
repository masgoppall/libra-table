//! Grid filters as a `SCAN` request: the key operators become a `MATCH` glob (with the
//! value's glob characters escaped), `type = …` becomes `TYPE`. Raw rows are a glob
//! pattern typed as is.

use crate::db::filter::{FilterRow, OperatorInfo, ANY_COLUMN, RAW_SQL};
use crate::error::{AppError, AppResult};

/// What SCAN is asked for. `pattern` None means every key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct KeyQuery {
    pub pattern: Option<String>,
    pub key_type: Option<String>,
}

impl KeyQuery {
    pub fn is_empty(&self) -> bool {
        self.pattern.is_none() && self.key_type.is_none()
    }

    /// The SCAN call as text (`MATCH … COUNT … TYPE …`), shown under the grid.
    pub fn describe(&self, count: u32) -> String {
        let mut s = String::from("SCAN 0");
        if let Some(p) = &self.pattern {
            s.push_str(&format!(" MATCH {}", super::args::quote(p.as_bytes())));
        }
        s.push_str(&format!(" COUNT {count}"));
        if let Some(t) = &self.key_type {
            s.push_str(&format!(" TYPE {t}"));
        }
        s
    }

    /// Only the filter part (`MATCH user:* TYPE hash`), for filter previews.
    pub fn clause(&self) -> String {
        let mut parts = Vec::new();
        if let Some(p) = &self.pattern {
            parts.push(format!("MATCH {}", super::args::quote(p.as_bytes())));
        }
        if let Some(t) = &self.key_type {
            parts.push(format!("TYPE {t}"));
        }
        parts.join(" ")
    }
}

/// The filter bar's operators: TablePlus offers these four for Redis.
pub fn operators() -> Vec<OperatorInfo> {
    [
        ("eq", "=", "key or type"),
        ("contains", "Contains", "text"),
        ("has_prefix", "Has prefix", "text"),
        ("has_suffix", "Has suffix", "text"),
    ]
    .into_iter()
    .map(|(id, label, placeholder)| OperatorInfo {
        id,
        label,
        value_kind: "single",
        placeholder,
    })
    .collect()
}

/// Escapes `* ? [ ] \` so the value matches literally in a glob.
pub fn escape_glob(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '*' | '?' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn need(value: &str) -> AppResult<&str> {
    if value.is_empty() {
        Err(AppError::validation("Enter a value for this filter."))
    } else {
        Ok(value)
    }
}

fn key_pattern(row: &FilterRow) -> AppResult<String> {
    if row.column == RAW_SQL {
        return Ok(need(row.value.trim())?.to_string());
    }
    let v = escape_glob(need(&row.value)?);
    Ok(match row.operator.as_str() {
        "eq" => v,
        "contains" => format!("*{v}*"),
        "has_prefix" => format!("{v}*"),
        "has_suffix" => format!("*{v}"),
        other => {
            return Err(AppError::validation(format!(
                "Redis can't filter keys with \"{other}\"; use =, Contains, Has prefix or \
                 Has suffix."
            )))
        }
    })
}

fn add_row(q: &mut KeyQuery, row: &FilterRow) -> AppResult<()> {
    match row.column.as_str() {
        "type" => {
            if row.operator != "eq" {
                return Err(AppError::validation("Filter the type with =."));
            }
            let t = need(row.value.trim())?.to_ascii_lowercase();
            if q.key_type.replace(t).is_some() {
                return Err(AppError::validation("Use one type filter at a time."));
            }
        }
        "key" | ANY_COLUMN | RAW_SQL => {
            let p = key_pattern(row)?;
            if q.pattern.replace(p).is_some() {
                return Err(AppError::validation(
                    "Redis matches keys with one pattern; use a single key filter.",
                ));
            }
        }
        other => {
            return Err(AppError::validation(format!(
                "Redis can filter by key and type only, not \"{other}\"."
            )))
        }
    }
    Ok(())
}

/// The SCAN request for the grid's filter rows (`only`: the per-row Apply).
pub fn from_filters(rows: &[FilterRow], only: Option<usize>) -> AppResult<KeyQuery> {
    let selected: Vec<&FilterRow> = match only {
        Some(i) => vec![rows
            .get(i)
            .ok_or_else(|| AppError::validation("That filter row no longer exists."))?],
        None => rows.iter().filter(|r| r.enabled).collect(),
    };
    let mut q = KeyQuery::default();
    for row in selected {
        add_row(&mut q, row)?;
    }
    Ok(q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(column: &str, operator: &str, value: &str) -> FilterRow {
        FilterRow {
            enabled: true,
            column: column.into(),
            operator: operator.into(),
            value: value.into(),
            value2: String::new(),
        }
    }

    fn pattern(r: FilterRow) -> Option<String> {
        from_filters(&[r], None).unwrap().pattern
    }

    #[test]
    fn operators_become_globs_with_escaping() {
        assert_eq!(pattern(row("key", "eq", "a*b")), Some(r"a\*b".into()));
        assert_eq!(pattern(row("key", "contains", "x?")), Some(r"*x\?*".into()));
        assert_eq!(
            pattern(row("key", "has_prefix", "user:")),
            Some("user:*".into())
        );
        assert_eq!(
            pattern(row("key", "has_suffix", "[1]")),
            Some(r"*\[1\]".into())
        );
        assert_eq!(pattern(row(ANY_COLUMN, "eq", r"a\b")), Some(r"a\\b".into()));
        assert_eq!(
            pattern(row(RAW_SQL, "", "user:[0-9]*")),
            Some("user:[0-9]*".into())
        );
    }

    #[test]
    fn type_filter_and_errors() {
        let q = from_filters(
            &[row("type", "eq", "Hash"), row("key", "has_prefix", "u")],
            None,
        )
        .unwrap();
        assert_eq!(q.key_type.as_deref(), Some("hash"));
        assert_eq!(q.describe(300), "SCAN 0 MATCH u* COUNT 300 TYPE hash");
        assert_eq!(q.clause(), "MATCH u* TYPE hash");
        assert!(from_filters(&[row("type", "contains", "h")], None).is_err());
        assert!(from_filters(&[row("ttl", "eq", "1")], None).is_err());
        assert!(from_filters(&[row("key", "lt", "1")], None).is_err());
        assert!(from_filters(&[row("key", "eq", "")], None).is_err());
        let two = [row("key", "eq", "a"), row("key", "eq", "b")];
        assert!(from_filters(&two, None).is_err());
        assert_eq!(
            from_filters(&two, Some(1)).unwrap().pattern.as_deref(),
            Some("b")
        );
        let mut off = row("key", "eq", "a");
        off.enabled = false;
        assert!(from_filters(&[off], None).unwrap().is_empty());
    }
}
