//! Running a `.sql` file (ImportSQLWizard): the file is read line by line and cut into
//! statements with the editor's splitter (`db::split`), a buffer at a time, so dumps of
//! any size stream through. Dump conventions are handled on the way: MySQL `DELIMITER`
//! lines, Postgres `COPY … FROM stdin` blocks and psql meta-commands (`\connect`).

use std::collections::VecDeque;
use std::io::BufRead;

use crate::db::split;
use crate::error::{AppError, AppResult};
use crate::models::Dialect;

/// The buffer is split once it holds this much text (it grows for huge statements).
const SPLIT_AT: usize = 256 * 1024;
/// COPY data is passed on in pieces of about this size.
const COPY_PIECE: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    /// A statement, its 1-based number and the line it starts on.
    Statement {
        index: u64,
        line: u64,
        sql: String,
    },
    /// `COPY … FROM stdin` (Postgres), followed by `CopyData` pieces and `CopyEnd`.
    CopyStart {
        index: u64,
        line: u64,
        sql: String,
    },
    CopyData(Vec<u8>),
    CopyEnd,
}

/// Iterates over the pieces of a SQL file.
pub struct StatementReader<R: BufRead> {
    reader: R,
    driver: Dialect,
    buf: String,
    /// File line of the buffer's first character.
    buf_line: u64,
    line_no: u64,
    split_at: usize,
    ready: VecDeque<Piece>,
    index: u64,
    delimiter: String,
    copy: Option<Vec<u8>>,
    done: bool,
    /// psql meta-commands left out.
    pub skipped_commands: u64,
}

impl<R: BufRead> StatementReader<R> {
    pub fn new(reader: R, driver: Dialect) -> Self {
        Self::with_split_at(reader, driver, SPLIT_AT)
    }

    /// For tests: split after `split_at` bytes to exercise the chunk boundaries.
    pub fn with_split_at(reader: R, driver: Dialect, split_at: usize) -> Self {
        Self {
            reader,
            driver,
            buf: String::new(),
            buf_line: 1,
            line_no: 0,
            split_at,
            ready: VecDeque::new(),
            index: 0,
            delimiter: ";".into(),
            copy: None,
            done: false,
            skipped_commands: 0,
        }
    }

    fn read_line(&mut self) -> AppResult<Option<String>> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).map_err(|e| {
            let at = self.line_no + 1;
            AppError::validation(format!("Could not read line {at} of the file: {e}"))
                .hint("SQL files must be UTF-8.")
        })?;
        if n == 0 {
            return Ok(None);
        }
        self.line_no += 1;
        Ok(Some(line))
    }

    /// Reads until at least one piece is ready or the file ends.
    fn fill(&mut self) -> AppResult<()> {
        while self.ready.is_empty() && !self.done {
            match self.read_line()? {
                None => {
                    self.done = true;
                    if let Some(rest) = self.copy.take() {
                        self.end_copy(rest);
                    }
                    self.split(true);
                }
                Some(line) => self.line(line),
            }
        }
        Ok(())
    }

    fn line(&mut self, line: String) {
        if self.copy.is_some() {
            return self.copy_line(line);
        }
        let trimmed = line.trim();
        if self.driver == Dialect::Mysql && starts_with_word(trimmed, "DELIMITER") {
            self.delimiter = trimmed[9..].trim().to_string();
            return self.skip_line();
        }
        if self.driver == Dialect::Postgres && trimmed.starts_with('\\') && self.at_top_level() {
            self.skipped_commands += 1;
            return self.skip_line();
        }
        if self.driver == Dialect::Postgres && is_copy_from_stdin(trimmed) && self.at_top_level() {
            self.split(true);
            self.index += 1;
            let sql = trimmed.trim_end_matches(';').to_string();
            let (index, line) = (self.index, self.line_no);
            self.ready.push_back(Piece::CopyStart { index, line, sql });
            self.copy = Some(Vec::new());
            return self.skip_line();
        }
        let mut line = line;
        if self.delimiter != ";" {
            line = replace_delimiter(&line, &self.delimiter);
        }
        self.buf.push_str(&line);
        if self.buf.len() >= self.split_at {
            self.split(false);
        }
    }

    /// A line that is not part of any statement: the buffer must not count it.
    fn skip_line(&mut self) {
        if self.buf.trim().is_empty() {
            self.buf.clear();
            self.buf_line = self.line_no + 1;
        } else {
            // Keep line numbers right: the dropped line becomes an empty one.
            self.buf.push('\n');
        }
    }

    /// True when the buffer holds only whole statements (so a line can't be inside a
    /// string or a body). Cheap check: the buffer is blank or ends a statement.
    fn at_top_level(&self) -> bool {
        let t = self.buf.trim_end();
        t.is_empty() || t.ends_with(';')
    }

    fn copy_line(&mut self, line: String) {
        if line.trim_end_matches(['\r', '\n']) == "\\." {
            let rest = self.copy.take().unwrap_or_default();
            self.end_copy(rest);
            self.buf_line = self.line_no + 1;
            return;
        }
        let data = self.copy.get_or_insert_with(Vec::new);
        data.extend_from_slice(line.as_bytes());
        if data.len() >= COPY_PIECE {
            let piece = std::mem::take(data);
            self.ready.push_back(Piece::CopyData(piece));
        }
    }

    fn end_copy(&mut self, rest: Vec<u8>) {
        if !rest.is_empty() {
            self.ready.push_back(Piece::CopyData(rest));
        }
        self.ready.push_back(Piece::CopyEnd);
    }

    /// Splits the buffer. Unless `all`, the last statement stays in the buffer: it may
    /// continue in the next lines.
    fn split(&mut self, all: bool) {
        let stmts = split::split(self.driver, &self.buf);
        if !all && stmts.len() < 2 {
            // One statement (or comment) still growing: wait for more text first.
            self.split_at = self.buf.len() * 2;
            return;
        }
        let emit = if all { stmts.len() } else { stmts.len() - 1 };
        let mut lines = LineIndex::new(&self.buf);
        for st in &stmts[..emit] {
            self.index += 1;
            let line = self.buf_line + lines.newlines_before(st.from);
            let (index, sql) = (self.index, st.text.clone());
            self.ready.push_back(Piece::Statement { index, line, sql });
        }
        let at = match stmts.get(emit) {
            Some(last) => utf16_to_byte(&self.buf, last.from),
            None => self.buf.len(),
        };
        self.buf_line += self.buf[..at].matches('\n').count() as u64;
        self.buf.drain(..at);
        self.split_at = SPLIT_AT.max(self.buf.len() * 2);
    }
}

impl<R: BufRead> Iterator for StatementReader<R> {
    type Item = AppResult<Piece>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(e) = self.fill() {
            self.done = true;
            self.ready.clear();
            return Some(Err(e));
        }
        self.ready.pop_front().map(Ok)
    }
}

fn starts_with_word(line: &str, word: &str) -> bool {
    line.len() > word.len()
        && line[..word.len()].eq_ignore_ascii_case(word)
        && line[word.len()..].starts_with(char::is_whitespace)
}

/// `COPY public.t (a, b) FROM stdin;` as pg_dump writes it.
fn is_copy_from_stdin(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    starts_with_word(line, "COPY")
        && upper
            .trim_end_matches(';')
            .trim_end()
            .ends_with("FROM STDIN")
}

/// `END$$` → `END;` when the delimiter is `$$` (MySQL scripts and dumps).
fn replace_delimiter(line: &str, delimiter: &str) -> String {
    let body = line.trim_end();
    match body.strip_suffix(delimiter) {
        Some(head) => format!("{head};{}", &line[body.len()..]),
        None => line.to_string(),
    }
}

/// Byte index of a UTF-16 offset (the splitter's unit).
fn utf16_to_byte(s: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units >= utf16 {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

/// Counts newlines before UTF-16 offsets, walking forward once.
struct LineIndex<'a> {
    chars: std::str::Chars<'a>,
    units: usize,
    newlines: u64,
}

impl<'a> LineIndex<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            chars: s.chars(),
            units: 0,
            newlines: 0,
        }
    }

    /// Offsets must be asked in increasing order.
    fn newlines_before(&mut self, utf16: usize) -> u64 {
        while self.units < utf16 {
            let Some(c) = self.chars.next() else { break };
            self.units += c.len_utf16();
            if c == '\n' {
                self.newlines += 1;
            }
        }
        self.newlines
    }
}
