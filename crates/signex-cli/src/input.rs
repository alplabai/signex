//! Input limits (#1923): what a run accepts before it looks at any content,
//! and a line reader that never holds more than one line's limit in memory.

use std::io::{self, BufRead, Read};

/// What one run accepts. Raising a limit is the caller's decision (a flag),
/// never the input's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Longest command line, in bytes, newline excluded.
    pub max_line_bytes: usize,
    /// Most command lines in one run.
    pub max_commands: u64,
    /// Largest file a command may name to be read, in bytes.
    pub max_file_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_line_bytes: 64 * 1024,
            max_commands: 10_000,
            max_file_bytes: 64 * 1024 * 1024,
        }
    }
}

/// One line of input, or why it cannot be one.
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    Text(String),
    TooLong,
    NotUtf8,
}

const DRAIN_CHUNK: u64 = 8 * 1024;

/// The next line of `reader`, `None` at the end of input. A line over `max`
/// bytes is skipped to its end without being kept.
pub fn read_line(reader: &mut impl BufRead, max: usize) -> io::Result<Option<Line>> {
    let mut buf = Vec::new();
    let read = reader
        .by_ref()
        .take(max as u64 + 1)
        .read_until(b'\n', &mut buf)?;
    if read == 0 {
        return Ok(None);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
    } else if buf.len() > max {
        drain_line(reader)?;
        return Ok(Some(Line::TooLong));
    }
    Ok(Some(match String::from_utf8(buf) {
        Ok(text) => Line::Text(text),
        Err(_) => Line::NotUtf8,
    }))
}

fn drain_line(reader: &mut impl BufRead) -> io::Result<()> {
    let mut chunk = Vec::new();
    loop {
        chunk.clear();
        let read = reader
            .by_ref()
            .take(DRAIN_CHUNK)
            .read_until(b'\n', &mut chunk)?;
        if read == 0 || chunk.last() == Some(&b'\n') {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn lines(input: &[u8], max: usize) -> Vec<Line> {
        let mut reader = Cursor::new(input.to_vec());
        let mut out = Vec::new();
        while let Some(line) = read_line(&mut reader, max).unwrap() {
            out.push(line);
        }
        out
    }

    fn text(s: &str) -> Line {
        Line::Text(s.to_string())
    }

    #[test]
    fn lines_split_on_newline_and_crlf_and_a_last_line_needs_no_newline() {
        assert_eq!(lines(b"a\r\nb\nc", 10), [text("a"), text("b"), text("c")]);
        assert_eq!(lines(b"", 10), []);
        assert_eq!(lines(b"\n", 10), [text("")]);
    }

    #[test]
    fn a_line_of_exactly_the_limit_passes_and_one_more_byte_is_too_long() {
        assert_eq!(lines(b"abcd\nx", 4), [text("abcd"), text("x")]);
        assert_eq!(lines(b"abcde\nx", 4), [Line::TooLong, text("x")]);
        assert_eq!(lines(b"abcd", 4), [text("abcd")]);
        assert_eq!(lines(b"abcde", 4), [Line::TooLong]);
    }

    #[test]
    fn a_huge_line_is_skipped_to_its_end_without_losing_the_next() {
        let mut input = vec![b'a'; 100_000];
        input.extend_from_slice(b"\nok\n");
        assert_eq!(lines(&input, 16), [Line::TooLong, text("ok")]);
    }

    #[test]
    fn a_line_that_is_not_utf8_is_reported_not_lossy_decoded() {
        assert_eq!(lines(b"\xff\xfe\nok", 10), [Line::NotUtf8, text("ok")]);
    }

    #[test]
    fn the_default_limits_are_the_documented_ones() {
        assert_eq!(
            Limits::default(),
            Limits {
                max_line_bytes: 65_536,
                max_commands: 10_000,
                max_file_bytes: 67_108_864,
            }
        );
    }
}
