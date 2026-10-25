//! Merge a Signex-managed block into a user-owned text file (`.gitignore`,
//! `.gitattributes`) without touching a byte outside the block.
//!
//! Pure text in, text out: no IO here. Callers read the file, call
//! [`merge_managed_block`], and write the result atomically.

/// First line of the managed block.
pub const BEGIN_MARKER: &str = "# >>> signex (managed) >>>";
/// Last line of the managed block.
pub const END_MARKER: &str = "# <<< signex (managed) <<<";

/// Why a managed block could not be merged. The file is left untouched.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManagedBlockError {
    /// More than one begin or end marker: Signex cannot tell which block
    /// is its own.
    #[error(
        "duplicate Signex managed block markers ({begins} begin, {ends} end); fix them by hand"
    )]
    DuplicateMarkers { begins: usize, ends: usize },
    /// A begin without an end, an end without a begin, or an end above its
    /// begin.
    #[error("unbalanced Signex managed block markers; fix or remove them by hand")]
    Unbalanced,
}

/// Merge `body` into `existing` as the managed block.
///
/// - `None` or empty text: the result is just the block.
/// - No block present: the block is appended; every existing byte is kept,
///   and a `\n` is added first only if the text does not end with one.
/// - One well-formed block: only the lines from the begin marker to the end
///   marker are replaced. Merging the same body again returns identical text.
/// - Duplicate or unbalanced markers: [`ManagedBlockError`], nothing guessed.
pub fn merge_managed_block(
    existing: Option<&str>,
    body: &str,
) -> Result<String, ManagedBlockError> {
    let text = existing.unwrap_or("");
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let positions = |marker: &str| -> Vec<usize> {
        (0..lines.len())
            .filter(|&i| lines[i].trim_end_matches(['\r', '\n']) == marker)
            .collect()
    };
    let begins = positions(BEGIN_MARKER);
    let ends = positions(END_MARKER);

    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => {
            let mut out = String::from(text);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&render_block(body, "\n", true));
            Ok(out)
        }
        (&[begin], &[end]) if begin < end => {
            let eol = if lines[begin].ends_with("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let mut out: String = lines[..begin].concat();
            out.push_str(&render_block(body, eol, lines[end].ends_with('\n')));
            out.push_str(&lines[end + 1..].concat());
            Ok(out)
        }
        (b, e) if b.len() > 1 || e.len() > 1 => Err(ManagedBlockError::DuplicateMarkers {
            begins: b.len(),
            ends: e.len(),
        }),
        _ => Err(ManagedBlockError::Unbalanced),
    }
}

fn render_block(body: &str, eol: &str, trailing_newline: bool) -> String {
    let mut out = String::from(BEGIN_MARKER);
    out.push_str(eol);
    for line in body.lines() {
        out.push_str(line);
        out.push_str(eol);
    }
    out.push_str(END_MARKER);
    if trailing_newline {
        out.push_str(eol);
    }
    out
}

/// Append every pattern line of `rows` that `text` does not already carry
/// (exact line match), after the existing content. Comment and blank lines
/// of `rows` ride along only when at least one pattern line is new, so a
/// repeat call is a no-op. Used for the user-chosen rows that live outside
/// the managed block.
pub fn append_missing_lines(text: &str, rows: &str) -> String {
    let present: Vec<&str> = text.lines().map(str::trim_end).collect();
    let is_pattern = |l: &str| !l.trim().is_empty() && !l.trim_start().starts_with('#');
    let any_new = rows
        .lines()
        .any(|l| is_pattern(l) && !present.contains(&l.trim_end()));
    if !any_new {
        return text.to_owned();
    }
    let mut out = String::from(text);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for line in rows.lines() {
        if is_pattern(line) && present.contains(&line.trim_end()) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[path = "managed_block_tests.rs"]
mod managed_block_tests;
