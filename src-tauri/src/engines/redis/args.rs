//! Editor scripts: one command per line, arguments quoted like redis-cli
//! (`"double quotes"` with `\n \r \t \b \a \\ \" \xHH` escapes, `'single quotes'` with
//! only `\'`). A quoted argument may span lines; `#` starts a comment line.

use crate::db::split::Statement;
use crate::error::{AppError, AppResult};

/// Splits a script into commands. Offsets are UTF-16 units (editor positions).
pub fn split(script: &str) -> Vec<Statement> {
    let chars: Vec<char> = script.chars().collect();
    let mut out = Vec::new();
    let mut utf16 = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        // Skip blank space before a command.
        if chars[i].is_whitespace() {
            utf16 += chars[i].len_utf16();
            i += 1;
            continue;
        }
        let start = i;
        let from = utf16;
        let end = line_end(&chars, i);
        let text: String = chars[start..end].iter().collect();
        let to = from
            + chars[start..end]
                .iter()
                .map(|c| c.len_utf16())
                .sum::<usize>();
        utf16 = to;
        i = end;
        let trimmed = text.trim_end();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let trailing: usize = text[trimmed.len()..].chars().map(char::len_utf16).sum();
        out.push(Statement {
            text: trimmed.to_string(),
            from,
            to: to - trailing,
        });
    }
    out
}

/// Where the command starting at `i` ends: the first newline outside quotes.
fn line_end(chars: &[char], mut i: usize) -> usize {
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some('"') if c == '\\' => i += 1,
            Some('\'') if c == '\\' && chars.get(i + 1) == Some(&'\'') => i += 1,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\n' => return i,
            None if (c == '"' || c == '\'') && at_word_start(chars, i) => quote = Some(c),
            None => {}
        }
        i += 1;
    }
    chars.len()
}

fn at_word_start(chars: &[char], i: usize) -> bool {
    i == 0 || chars[i - 1].is_whitespace()
}

/// Splits one command into its arguments as bytes (redis-cli `sdssplitargs`).
pub fn parse(line: &str) -> AppResult<Vec<Vec<u8>>> {
    let chars: Vec<char> = line.chars().collect();
    let mut args = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let mut cur = Vec::new();
        i = match chars[i] {
            '"' => double_quoted(&chars, i + 1, &mut cur)?,
            '\'' => single_quoted(&chars, i + 1, &mut cur)?,
            _ => bare(&chars, i, &mut cur),
        };
        args.push(cur);
    }
    Ok(args)
}

fn push_char(out: &mut Vec<u8>, c: char) {
    let mut buf = [0u8; 4];
    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
}

fn bare(chars: &[char], mut i: usize, out: &mut Vec<u8>) -> usize {
    while i < chars.len() && !chars[i].is_whitespace() {
        push_char(out, chars[i]);
        i += 1;
    }
    i
}

fn unbalanced() -> AppError {
    AppError::validation("Invalid argument(s): unbalanced quotes.")
        .hint("Close every quote, and put a space after a closing quote.")
}

/// After a closing quote there must be whitespace or the end of the line.
fn after_quote(chars: &[char], i: usize) -> AppResult<usize> {
    match chars.get(i) {
        None => Ok(i),
        Some(c) if c.is_whitespace() => Ok(i),
        Some(_) => Err(unbalanced()),
    }
}

fn double_quoted(chars: &[char], mut i: usize, out: &mut Vec<u8>) -> AppResult<usize> {
    while i < chars.len() {
        match chars[i] {
            '"' => return after_quote(chars, i + 1),
            '\\' if i + 1 < chars.len() => {
                let next = chars[i + 1];
                if next == 'x' {
                    if let Some(b) = hex_byte(chars, i + 2) {
                        out.push(b);
                        i += 4;
                        continue;
                    }
                }
                let b = match next {
                    'n' => b'\n',
                    'r' => b'\r',
                    't' => b'\t',
                    'b' => 0x08,
                    'a' => 0x07,
                    other => {
                        push_char(out, other);
                        i += 2;
                        continue;
                    }
                };
                out.push(b);
                i += 2;
            }
            c => {
                push_char(out, c);
                i += 1;
            }
        }
    }
    Err(unbalanced())
}

fn hex_byte(chars: &[char], i: usize) -> Option<u8> {
    let hi = chars.get(i)?.to_digit(16)?;
    let lo = chars.get(i + 1)?.to_digit(16)?;
    Some((hi * 16 + lo) as u8)
}

fn single_quoted(chars: &[char], mut i: usize, out: &mut Vec<u8>) -> AppResult<usize> {
    while i < chars.len() {
        match chars[i] {
            '\\' if chars.get(i + 1) == Some(&'\'') => {
                out.push(b'\'');
                i += 2;
            }
            '\'' => return after_quote(chars, i + 1),
            c => {
                push_char(out, c);
                i += 1;
            }
        }
    }
    Err(unbalanced())
}

/// Shows an argument the way redis-cli would accept it back: bare when it has no
/// spaces, quotes or control bytes, otherwise double-quoted with escapes.
pub fn quote(arg: &[u8]) -> String {
    let plain = !arg.is_empty()
        && std::str::from_utf8(arg).is_ok_and(|s| {
            s.chars()
                .all(|c| !c.is_whitespace() && !c.is_control() && c != '"' && c != '\'')
        });
    if plain {
        return String::from_utf8_lossy(arg).into_owned();
    }
    let mut out = String::from("\"");
    for chunk in arg.utf8_chunks() {
        for c in chunk.valid().chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => {
                    let mut buf = [0u8; 4];
                    for b in c.encode_utf8(&mut buf).bytes() {
                        out.push_str(&format!("\\x{b:02x}"));
                    }
                }
                c => out.push(c),
            }
        }
        for b in chunk.invalid() {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out.push('"');
    out
}

/// Longest argument shown in a preview (confirmation dialog, history).
const PREVIEW_ARG: usize = 200;

/// A command line for previews: arguments quoted, long values shortened.
pub fn preview(args: &[Vec<u8>]) -> String {
    args.iter()
        .map(|a| {
            if a.len() > PREVIEW_ARG {
                let mut cut = a[..PREVIEW_ARG].to_vec();
                while std::str::from_utf8(&cut).is_err() && !cut.is_empty() {
                    cut.pop();
                }
                let shown = quote(&cut);
                match shown.strip_suffix('"') {
                    Some(open) => format!("{open}…\""),
                    None => format!("{shown}…"),
                }
            } else {
                quote(a)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(line: &str) -> Vec<String> {
        parse(line)
            .unwrap()
            .into_iter()
            .map(|a| String::from_utf8(a).unwrap())
            .collect()
    }

    #[test]
    fn parses_like_redis_cli() {
        assert_eq!(strs("SET k v"), ["SET", "k", "v"]);
        assert_eq!(strs("  set  \"a b\"  'c d' "), ["set", "a b", "c d"]);
        assert_eq!(
            strs(r#"SET k "line\nnext \"q\"""#),
            ["SET", "k", "line\nnext \"q\""]
        );
        assert_eq!(strs(r"SET k 'it\'s'"), ["SET", "k", "it's"]);
        assert_eq!(strs(r"SET k 'no\nescape'"), ["SET", "k", "no\\nescape"]);
        assert_eq!(parse(r#"SET k "\x00\xff""#).unwrap()[2], vec![0u8, 0xff]);
        assert_eq!(strs("SET k \"\""), ["SET", "k", ""]);
        assert_eq!(strs("GET a\"b"), ["GET", "a\"b"]);
    }

    #[test]
    fn rejects_unbalanced_quotes() {
        assert!(parse("SET k \"open").is_err());
        assert!(parse("SET k 'open").is_err());
        assert!(parse("SET k \"a\"b").is_err());
    }

    #[test]
    fn splits_one_command_per_line() {
        let s = split("SET a 1\n\n# note\n  GET a  \nSET b \"x\ny\"\r\nDEL a");
        let texts: Vec<_> = s.iter().map(|st| st.text.as_str()).collect();
        assert_eq!(texts, ["SET a 1", "GET a", "SET b \"x\ny\"", "DEL a"]);
        assert_eq!((s[0].from, s[0].to), (0, 7));
        assert_eq!((s[1].from, s[1].to), (18, 23));
    }

    #[test]
    fn utf16_offsets() {
        let s = split("SET k 😀\nGET k");
        assert_eq!((s[0].from, s[0].to), (0, 8));
        assert_eq!(s[1].from, 9);
    }

    #[test]
    fn quotes_round_trip() {
        for arg in [
            &b"plain"[..],
            b"a b",
            b"",
            b"q\"uote",
            b"new\nline",
            b"\x00\xff",
            b"it's",
        ] {
            let line = format!("SET {}", quote(arg));
            assert_eq!(parse(&line).unwrap()[1], arg, "{line}");
        }
        assert_eq!(quote(b"user:1"), "user:1");
        assert!(preview(&[b"SET".to_vec(), vec![b'a'; 300]]).ends_with("a…"));
        assert!(preview(&[vec![b' '; 300]]).ends_with("…\""));
    }
}
