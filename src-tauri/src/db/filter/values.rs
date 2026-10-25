//! Reading filter values: IN lists and LIKE escaping.

/// The escape character for LIKE patterns.
pub(super) const LIKE_ESCAPE: char = '!';

/// Escapes LIKE wildcards so the value matches literally (FLT-4).
pub fn escape_like(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == LIKE_ESCAPE || ch == '%' || ch == '_' {
            out.push(LIKE_ESCAPE);
        }
        out.push(ch);
    }
    out
}

/// Splits `a, b, 'c, d', "e"` into items. Quotes group an item and are removed;
/// a doubled quote inside quotes is a literal quote.
pub fn parse_list(input: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut had_quotes = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match quote {
            Some(q) if ch == q => {
                if chars.peek() == Some(&q) {
                    cur.push(q);
                    chars.next();
                } else {
                    quote = None;
                }
            }
            Some(_) => cur.push(ch),
            None if ch == '\'' || ch == '"' => {
                // Whitespace before the opening quote is not part of the value.
                if cur.trim().is_empty() {
                    cur.clear();
                }
                quote = Some(ch);
                had_quotes = true;
            }
            None if ch == ',' => {
                push_item(&mut items, &mut cur, &mut had_quotes);
            }
            // Whitespace after a closing quote is not part of the value either.
            None if had_quotes && ch.is_whitespace() => {}
            None => cur.push(ch),
        }
    }
    push_item(&mut items, &mut cur, &mut had_quotes);
    items
}

fn push_item(items: &mut Vec<String>, cur: &mut String, had_quotes: &mut bool) {
    let item = if *had_quotes {
        cur.clone()
    } else {
        cur.trim().to_string()
    };
    if !item.is_empty() || *had_quotes {
        items.push(item);
    }
    cur.clear();
    *had_quotes = false;
}
