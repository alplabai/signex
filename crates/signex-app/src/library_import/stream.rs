//! Split a converter's stdout into lines as it arrives, bounded: a line
//! longer than the limit is dropped (only its length is kept), and a last
//! line without its newline is still handed over at the end.

/// The longest stdout line read. A summary listing every written file
/// stays well under it.
pub const MAX_LINE_BYTES: usize = 4 << 20;

/// One stdout line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Text(String),
    /// A line over the limit; how many bytes it had.
    Oversized(usize),
}

/// Incremental line splitter.
#[derive(Debug)]
pub struct LineSplitter {
    max: usize,
    buf: Vec<u8>,
    /// Bytes of the current line dropped because it ran over `max`.
    dropped: Option<usize>,
}

impl Default for LineSplitter {
    fn default() -> Self {
        Self::new(MAX_LINE_BYTES)
    }
}

impl LineSplitter {
    pub fn new(max: usize) -> Self {
        Self {
            max,
            buf: Vec::new(),
            dropped: None,
        }
    }

    /// Feed a chunk; every line it completes is appended to `out`.
    pub fn push(&mut self, mut chunk: &[u8], out: &mut Vec<Line>) {
        while let Some(at) = chunk.iter().position(|&b| b == b'\n') {
            self.take(&chunk[..at]);
            out.push(self.end_line());
            chunk = &chunk[at + 1..];
        }
        self.take(chunk);
    }

    /// The last line, when stdout ended without a newline.
    pub fn finish(mut self) -> Option<Line> {
        if self.buf.is_empty() && self.dropped.is_none() {
            return None;
        }
        Some(self.end_line())
    }

    fn take(&mut self, bytes: &[u8]) {
        if let Some(dropped) = self.dropped.as_mut() {
            *dropped += bytes.len();
            return;
        }
        if self.buf.len() + bytes.len() > self.max {
            self.dropped = Some(self.buf.len() + bytes.len());
            self.buf = Vec::new();
            return;
        }
        self.buf.extend_from_slice(bytes);
    }

    fn end_line(&mut self) -> Line {
        if let Some(len) = self.dropped.take() {
            return Line::Oversized(len);
        }
        let bytes = std::mem::take(&mut self.buf);
        let text = String::from_utf8_lossy(&bytes);
        Line::Text(text.strip_suffix('\r').unwrap_or(&text).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(max: usize, chunks: &[&[u8]]) -> Vec<Line> {
        let mut s = LineSplitter::new(max);
        let mut out = Vec::new();
        for c in chunks {
            s.push(c, &mut out);
        }
        out.extend(s.finish());
        out
    }

    fn text(s: &str) -> Line {
        Line::Text(s.into())
    }

    #[test]
    fn lines_split_across_chunks_join() {
        assert_eq!(
            split(64, &[b"ab", b"c\nde", b"f\r\n", b"g"]),
            [text("abc"), text("def"), text("g")]
        );
    }

    #[test]
    fn an_oversized_line_is_dropped_and_the_next_one_kept() {
        let long = vec![b'x'; 100];
        let out = split(10, &[&long[..60], &long[60..], b"\nok\n"]);
        assert_eq!(out, [Line::Oversized(100), text("ok")]);
    }

    #[test]
    fn nothing_pending_yields_nothing_at_the_end() {
        assert_eq!(split(8, &[b"a\n"]), [text("a")]);
        assert!(split(8, &[]).is_empty());
    }
}
