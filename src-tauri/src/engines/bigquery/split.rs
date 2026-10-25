//! Editor scripts: `;`-separated GoogleSQL (backticks, backslash escapes), except that a
//! scripting block (BEGIN … END, IF … END IF, LOOP, WHILE, REPEAT, FOR) stays one
//! statement so it runs as one script.

use crate::db::split::{self, Statement};
use crate::models::Dialect;

/// Change in block depth a split piece makes.
fn depth_change(text: &str) -> i32 {
    let words: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == '(')
        .filter(|w| !w.is_empty())
        .take(2)
        .map(str::to_ascii_uppercase)
        .collect();
    let first = words.first().map(String::as_str).unwrap_or("");
    let second = words.get(1).map(String::as_str).unwrap_or("");
    match first {
        "BEGIN" if second == "TRANSACTION" || second.is_empty() => 0,
        "BEGIN" | "IF" | "LOOP" | "WHILE" | "REPEAT" | "FOR" => 1,
        "END" => -1,
        _ => 0,
    }
}

/// UTF-16 offsets back to text.
fn slice(units: &[u16], from: usize, to: usize) -> String {
    String::from_utf16_lossy(&units[from.min(units.len())..to.min(units.len())])
}

pub fn split(script: &str) -> Vec<Statement> {
    let pieces = split::split(Dialect::Mysql, script);
    let units: Vec<u16> = script.encode_utf16().collect();
    let mut out: Vec<Statement> = Vec::new();
    let mut open: Option<(usize, i32)> = None;
    for p in pieces {
        let change = depth_change(&p.text);
        match open.as_mut() {
            Some((from, depth)) => {
                *depth += change;
                if *depth <= 0 {
                    let from = *from;
                    out.push(Statement {
                        text: slice(&units, from, p.to),
                        from,
                        to: p.to,
                    });
                    open = None;
                }
            }
            None if change > 0 => open = Some((p.from, change)),
            None => out.push(p),
        }
    }
    if let Some((from, _)) = open {
        let to = units.len();
        let text = slice(&units, from, to).trim_end().to_string();
        out.push(Statement { text, from, to });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(s: &str) -> Vec<String> {
        split(s).into_iter().map(|s| s.text).collect()
    }

    #[test]
    fn blocks_stay_whole() {
        let script = "SELECT 1;\nBEGIN\n  SELECT 2;\n  IF x THEN SELECT 3; END IF;\nEND;\nSELECT 4";
        assert_eq!(
            texts(script),
            vec![
                "SELECT 1",
                "BEGIN\n  SELECT 2;\n  IF x THEN SELECT 3; END IF;\nEND",
                "SELECT 4"
            ]
        );
        assert_eq!(
            texts("BEGIN TRANSACTION; SELECT 'a;b'; COMMIT TRANSACTION"),
            vec!["BEGIN TRANSACTION", "SELECT 'a;b'", "COMMIT TRANSACTION"]
        );
    }
}
