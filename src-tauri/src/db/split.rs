//! Splits a script into statements on `;`, ignoring semicolons inside strings, quoted
//! identifiers, comments and Postgres dollar-quoted bodies. Offsets are UTF-16 code
//! units so they line up with editor (CodeMirror) positions for "Run current" (QRY-3).

use serde::Serialize;

use crate::models::Dialect;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Statement {
    pub text: String,
    /// UTF-16 offset of the first non-blank character.
    pub from: usize,
    /// UTF-16 offset just past the statement (before the `;`).
    pub to: usize,
}

pub fn split(driver: Dialect, sql: &str) -> Vec<Statement> {
    let chars: Vec<char> = sql.chars().collect();
    let mut s = Splitter::new(&chars, driver);
    s.run();
    s.out
}

/// The words that open a compound body (CREATE TRIGGER/PROCEDURE/FUNCTION/EVENT).
const COMPOUND_KINDS: &[&str] = &["TRIGGER", "PROCEDURE", "FUNCTION", "EVENT"];
/// END IF / END LOOP / END WHILE / END REPEAT close blocks we don't count.
const NESTED_ENDS: &[&str] = &["IF", "LOOP", "WHILE", "REPEAT"];

struct Splitter<'a> {
    chars: &'a [char],
    /// utf16[i] = UTF-16 offset of chars[i]
    utf16: Vec<usize>,
    mysql: bool,
    pg: bool,
    out: Vec<Statement>,
    /// Where the current statement starts.
    start: usize,
    // Compound bodies (CREATE TRIGGER/PROCEDURE/FUNCTION/EVENT … BEGIN … END) keep their
    // inner semicolons: track the first words of the statement and BEGIN/CASE … END depth.
    words: Vec<String>,
    compound: bool,
    depth: i32,
}

impl<'a> Splitter<'a> {
    fn new(chars: &'a [char], driver: Dialect) -> Self {
        let mut utf16 = Vec::with_capacity(chars.len() + 1);
        let mut off = 0usize;
        for c in chars {
            utf16.push(off);
            off += c.len_utf16();
        }
        utf16.push(off);
        Self {
            chars,
            utf16,
            mysql: driver == Dialect::Mysql,
            pg: driver == Dialect::Postgres,
            out: Vec::new(),
            start: 0,
            words: Vec::new(),
            compound: false,
            depth: 0,
        }
    }

    fn run(&mut self) {
        let n = self.chars.len();
        let mut i = 0usize;
        while i < n {
            let c = self.chars[i];
            i = if c.is_alphabetic() || c == '_' {
                self.word(i)
            } else {
                self.symbol(i)
            };
        }
        self.end_statement(n);
    }

    /// Reads the word at `start` and tracks compound bodies. Returns the index after it.
    fn word(&mut self, start: usize) -> usize {
        let chars = self.chars;
        let mut i = start;
        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
            i += 1;
        }
        let word: String = chars[start..i]
            .iter()
            .collect::<String>()
            .to_ascii_uppercase();
        if self.words.len() < 10 {
            self.words.push(word.clone());
            self.compound = self.words[0] == "CREATE"
                && self
                    .words
                    .iter()
                    .skip(1)
                    .take(8)
                    .any(|w| COMPOUND_KINDS.contains(&w.as_str()));
        }
        if self.compound {
            match word.as_str() {
                "BEGIN" | "CASE" => self.depth += 1,
                "END" => {
                    let next = next_word(chars, i);
                    if !next.is_some_and(|w| NESTED_ENDS.contains(&w.as_str())) {
                        self.depth = (self.depth - 1).max(0);
                    }
                }
                _ => {}
            }
        }
        i
    }

    /// Skips quotes and comments and ends statements at `;`. Returns the next index.
    fn symbol(&mut self, i: usize) -> usize {
        let chars = self.chars;
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let mysql = self.mysql;
        match c {
            '\'' | '"' | '`' if c != '`' || mysql => skip_quoted(chars, i, c, mysql && c != '`'),
            // In MySQL `--` starts a comment only when followed by whitespace (or the end).
            '-' if next == Some('-')
                && (!mysql || chars.get(i + 2).is_none_or(|c| c.is_whitespace())) =>
            {
                skip_line(chars, i)
            }
            '#' if mysql => skip_line(chars, i),
            '/' if next == Some('*') => skip_block(chars, i, self.pg),
            '$' if self.pg => skip_dollar(chars, i).unwrap_or(i + 1),
            ';' if self.compound && self.depth > 0 => i + 1,
            ';' => {
                self.end_statement(i);
                self.start = i + 1;
                self.words.clear();
                self.compound = false;
                self.depth = 0;
                i + 1
            }
            _ => i + 1,
        }
    }

    /// Adds the statement from `self.start` to `end`, trimmed, unless it is empty or
    /// only comments.
    fn end_statement(&mut self, end: usize) {
        let (chars, start) = (self.chars, self.start);
        let slice = &chars[start..end];
        let lead = slice.iter().take_while(|c| c.is_whitespace()).count();
        let trail = slice.iter().rev().take_while(|c| c.is_whitespace()).count();
        if lead + trail >= slice.len() {
            return;
        }
        let s = start + lead;
        let e = end - trail;
        let text: String = chars[s..e].iter().collect();
        if is_only_comments(&text, self.mysql) {
            return;
        }
        self.out.push(Statement {
            text,
            from: self.utf16[s],
            to: self.utf16[e],
        });
    }
}

/// The statement that contains `cursor` (UTF-16), or the last one that starts before it.
pub fn statement_at(stmts: &[Statement], cursor: usize) -> Option<&Statement> {
    stmts
        .iter()
        .find(|s| cursor >= s.from && cursor <= s.to + 1)
        .or_else(|| stmts.iter().rev().find(|s| s.from <= cursor))
        .or_else(|| stmts.first())
}

/// The next word after position `i`, upper-cased (skipping whitespace).
fn next_word(chars: &[char], mut i: usize) -> Option<String> {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let start = i;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    if i > start {
        Some(
            chars[start..i]
                .iter()
                .collect::<String>()
                .to_ascii_uppercase(),
        )
    } else {
        None
    }
}

/// MySQL runs `/*! … */` (versioned comments, as mysqldump writes `SET` lines): code.
fn is_only_comments(text: &str, mysql: bool) -> bool {
    let mut rest = text.trim();
    loop {
        if rest.is_empty() {
            return true;
        }
        if let Some(r) = rest.strip_prefix("--").or_else(|| rest.strip_prefix('#')) {
            rest = r
                .split_once('\n')
                .map(|(_, b)| b)
                .unwrap_or("")
                .trim_start();
        } else if let Some(r) = rest.strip_prefix("/*") {
            if mysql && r.starts_with('!') {
                return false;
            }
            match r.find("*/") {
                Some(p) => rest = r[p + 2..].trim_start(),
                None => return true,
            }
        } else {
            return false;
        }
    }
}

fn skip_quoted(chars: &[char], start: usize, q: char, backslash_escapes: bool) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        let c = chars[i];
        if backslash_escapes && c == '\\' {
            i += 2;
            continue;
        }
        if c == q {
            if chars.get(i + 1) == Some(&q) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    chars.len()
}

fn skip_line(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

fn skip_block(chars: &[char], start: usize, nested: bool) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            depth += 1;
            i += 2;
            if !nested && depth > 1 {
                depth = 1;
            }
            continue;
        }
        if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
            continue;
        }
        i += 1;
    }
    chars.len()
}

/// `$tag$ … $tag$` (tag may be empty). Returns the index after the closing tag, or
/// None when this `$` does not start a dollar quote (e.g. a `$1` placeholder).
fn skip_dollar(chars: &[char], start: usize) -> Option<usize> {
    let mut j = start + 1;
    while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
        if j == start + 1 && chars[j].is_ascii_digit() {
            return None;
        }
        j += 1;
    }
    if chars.get(j) != Some(&'$') {
        return None;
    }
    let tag: Vec<char> = chars[start..=j].to_vec();
    let mut i = j + 1;
    while i + tag.len() <= chars.len() {
        if chars[i..i + tag.len()] == tag[..] {
            return Some(i + tag.len());
        }
        i += 1;
    }
    Some(chars.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(driver: Dialect, sql: &str) -> Vec<String> {
        split(driver, sql).into_iter().map(|s| s.text).collect()
    }

    #[test]
    fn mysql_versioned_comments_are_statements() {
        let sql = "/*!40101 SET NAMES utf8mb4 */;\n/* plain */;\nselect 1;";
        assert_eq!(
            texts(Dialect::Mysql, sql),
            vec!["/*!40101 SET NAMES utf8mb4 */", "select 1"]
        );
        assert_eq!(texts(Dialect::Postgres, sql), vec!["select 1"]);
    }

    #[test]
    fn splits_on_semicolons_outside_strings_and_comments() {
        let sql = "select 'a;b'; -- c;d\nselect \"x;y\" /* e;f */ from t;\n\n";
        assert_eq!(
            texts(Dialect::Postgres, sql),
            vec!["select 'a;b'", "-- c;d\nselect \"x;y\" /* e;f */ from t"]
        );
    }

    #[test]
    fn postgres_dollar_quotes_and_placeholders() {
        let sql = "create function f() returns int as $$ begin return 1; end $$ \
                   language plpgsql; select $1;";
        let t = texts(Dialect::Postgres, sql);
        assert_eq!(t.len(), 2);
        assert!(t[0].ends_with("language plpgsql"));
        assert_eq!(t[1], "select $1");
        let tagged = "do $body$ begin perform 1; end $body$; select 2";
        assert_eq!(texts(Dialect::Postgres, tagged).len(), 2);
    }

    #[test]
    fn mysql_backticks_hash_comments_and_backslashes() {
        let sql = "select `a;b`, 'it\\'s; ok' from t # trailing; comment\n; select 2";
        assert_eq!(
            texts(Dialect::Mysql, sql),
            vec![
                "select `a;b`, 'it\\'s; ok' from t # trailing; comment",
                "select 2"
            ]
        );
    }

    #[test]
    fn mysql_double_dash_needs_whitespace() {
        // MySQL does not treat `--1` as a comment, so the DELETE is its own statement.
        assert_eq!(
            texts(Dialect::Mysql, "SELECT 1 --1; DELETE FROM t"),
            vec!["SELECT 1 --1", "DELETE FROM t"]
        );
        assert_eq!(texts(Dialect::Mysql, "SELECT 1 -- c; d\nSELECT 2").len(), 1);
        assert_eq!(
            texts(Dialect::Postgres, "SELECT 1 --1; x\nSELECT 2").len(),
            1
        );
    }

    #[test]
    fn compound_bodies_stay_whole() {
        let lite = "CREATE TRIGGER t AFTER INSERT ON a BEGIN UPDATE b SET n = n + 1; \
                    INSERT INTO log VALUES (1); END; SELECT 1;";
        let t = texts(Dialect::Sqlite, lite);
        assert_eq!(t.len(), 2, "{t:?}");
        assert!(t[0].ends_with("END"));
        let proc = "CREATE PROCEDURE p() BEGIN DECLARE x INT; IF x > 1 THEN SELECT 1; END IF; \
                    SET x = CASE WHEN x = 1 THEN 2 ELSE 3 END; \
                    WHILE x > 0 DO SET x = x - 1; END WHILE; END; CALL p();";
        let t = texts(Dialect::Mysql, proc);
        assert_eq!(t.len(), 2, "{t:?}");
        assert_eq!(t[1], "CALL p()");
        // A plain transaction BEGIN is not a compound body.
        assert_eq!(
            texts(Dialect::Sqlite, "BEGIN; UPDATE a SET x = 1; COMMIT;").len(),
            3
        );
        // MySQL triggers without BEGIN end at the first semicolon.
        assert_eq!(
            texts(
                Dialect::Mysql,
                "CREATE TRIGGER t BEFORE INSERT ON a FOR EACH ROW SET NEW.x = 1; SELECT 2"
            )
            .len(),
            2
        );
    }

    #[test]
    fn drops_comment_only_statements() {
        assert_eq!(
            texts(Dialect::Sqlite, "-- only a comment\n; /* x */ ;select 1"),
            vec!["select 1"]
        );
    }

    #[test]
    fn utf16_offsets_and_statement_at_cursor() {
        let sql = "select '😀';\nselect 2;";
        let s = split(Dialect::Sqlite, sql);
        assert_eq!(s[0].from, 0);
        assert_eq!(s[0].to, 11); // emoji is 2 UTF-16 units
        assert_eq!(s[1].from, 13);
        assert_eq!(statement_at(&s, 15).unwrap().text, "select 2");
        assert_eq!(statement_at(&s, 3).unwrap().text, "select '😀'");
        assert_eq!(statement_at(&s, 12).unwrap().text, "select '😀'");
    }
}
