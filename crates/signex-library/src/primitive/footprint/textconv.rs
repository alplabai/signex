//! The readable text of a footprint file for diffs (#3109): each embedded
//! model's `data` block becomes one summary line, so a changed model shows
//! as its `hash` line and this line instead of thousands of base64 lines.
//! `signex-cli library textconv` prints it for git's `textconv` driver.

/// Hex digits of the hash the summary line shows.
const SUMMARY_HASH_DIGITS: usize = 12;

/// The fields of the `[[models]]` entry being read.
#[derive(Default)]
struct Entry {
    hash: Option<String>,
    encoding: Option<String>,
    size: Option<String>,
}

impl Entry {
    fn summary(&self) -> String {
        let hash = self.hash.as_deref().unwrap_or("?");
        let short: String = hash.chars().take(SUMMARY_HASH_DIGITS).collect();
        let ellipsis = if short.len() < hash.len() { "…" } else { "" };
        format!(
            "data = <{}, {} bytes, sha256 {short}{ellipsis}>",
            self.encoding.as_deref().unwrap_or("?"),
            self.size.as_deref().unwrap_or("?"),
        )
    }
}

/// The value of `line` when it reads `key = <value>`, quotes removed.
fn value_of<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line
        .trim()
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?;
    Some(rest.trim().trim_matches('"'))
}

/// `text` with every `[[models]]` `data` value replaced by a line like
/// `data = <deflate+base64, 482113 bytes, sha256 ab12cd34ef56…>`. Every
/// other line is kept byte for byte; text that is not a footprint file
/// comes back unchanged.
pub fn textconv_footprint_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(1 << 20));
    let mut entry: Option<Entry> = None;
    let mut lines = text.split_inclusive('\n');
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            entry = (trimmed == "[[models]]").then(Entry::default);
            out.push_str(line);
            continue;
        }
        let Some(fields) = entry.as_mut() else {
            out.push_str(line);
            continue;
        };
        if let Some(rest) = value_of(line, "data") {
            let newline = if line.ends_with('\n') { "\n" } else { "" };
            // A `'''` block runs to the line that closes it.
            let mut closed = !rest.starts_with("'''") || (rest.len() >= 6 && rest.ends_with("'''"));
            let mut last = line;
            while !closed {
                match lines.next() {
                    Some(next) => {
                        closed = next.trim_end().ends_with("'''");
                        last = next;
                    }
                    None => break,
                }
            }
            let newline = if last.ends_with('\n') { "\n" } else { newline };
            out.push_str(&fields.summary());
            out.push_str(newline);
            continue;
        }
        if let Some(hash) = value_of(line, "hash") {
            fields.hash = Some(hash.to_string());
        } else if let Some(encoding) = value_of(line, "encoding") {
            fields.encoding = Some(encoding.to_string());
        } else if let Some(size) = value_of(line, "size") {
            fields.size = Some(size.to_string());
        }
        out.push_str(line);
    }
    out
}
