//! The comparison key behind [`super::scope::path_key_with`]: one string per
//! file as the volume names it, so two spellings of a file compare equal.
//!
//! Pure text, never the disk: the key runs on the UI thread.

use std::path::Path;

use super::case_fold::CaseFold;

/// `path` as a key under `fold`: Unicode-normalised (NFC) where the volume
/// ignores the NFC / NFD difference, case-folded where it ignores case,
/// separator-folded on a Windows host, with `.` and `a/..` resolved (#1149).
pub(super) fn fold_key(path: &Path, fold: CaseFold) -> String {
    let mut key = path.to_string_lossy().into_owned();
    if fold.normalize_unicode {
        key = unicode_normalization::UnicodeNormalization::nfc(key.as_str()).collect();
    }
    if fold.fold_case {
        // Full Unicode lowercase: NTFS and APFS both fold non-ASCII letters, not only A-Z.
        key = key.chars().map(fold_char).collect();
    }
    if fold.backslash_is_separator {
        key = key.replace('\\', "/");
    }
    collapse_dot_segments(&key)
}

/// Simple case fold in the shape of the NTFS / APFS upcase table: a letter
/// and its other-case form share one result, but a character that only
/// lowercases INTO an ASCII letter (the Kelvin sign `U+212A`) stays itself, so
/// a look-alike never shares an identity with the real path.
fn fold_char(c: char) -> char {
    fn single(mut chars: impl Iterator<Item = char>) -> Option<char> {
        let first = chars.next()?;
        chars.next().is_none().then_some(first)
    }
    let Some(upper) = single(c.to_uppercase()) else {
        return c;
    };
    match single(upper.to_lowercase()) {
        Some(lower) if single(lower.to_uppercase()) == Some(upper) => lower,
        _ => upper,
    }
}

/// Resolve `.` and `a/..` segments textually, so `W/./Top.snxsch` and
/// `W/Sub/../Top.snxsch` key as `W/Top.snxsch`.
///
/// Lexical on purpose: the key runs on the UI thread and must never touch the
/// disk. The limit is symlinks: a link and its target are two keys, and a
/// `..` after a symlinked directory is resolved against the link's own
/// parent. Telling those apart needs a `stat` or `canonicalize`, which a
/// caller can do off-thread before it builds the path.
fn collapse_dot_segments(key: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for (index, segment) in key.split('/').enumerate() {
        match segment {
            // An empty first segment is the root; later empties are `//`.
            "" if index > 0 => {}
            "." => {}
            ".." => match out.last() {
                Some(&last) if is_anchor(last) => {}
                Some(&last) if last != ".." => {
                    out.pop();
                }
                _ => out.push(".."),
            },
            other => out.push(other),
        }
    }
    match out.as_slice() {
        // Only the root survived: `join` would yield "".
        [""] => "/".to_string(),
        _ => out.join("/"),
    }
}

/// The root (`""` before the first `/`) or a drive prefix (`C:`), which `..`
/// cannot climb out of.
fn is_anchor(segment: &str) -> bool {
    segment.is_empty() || (segment.len() == 2 && segment.ends_with(':'))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn case_fold_covers_non_ascii_letters() {
        assert_eq!(
            fold_key(Path::new("/w/\u{c4}pfel.snxsch"), CaseFold::case_only()),
            fold_key(Path::new("/w/\u{e4}pfel.snxsch"), CaseFold::case_only()),
        );
    }

    #[test]
    fn dot_dot_after_a_symlinked_directory_collapses_lexically() {
        // Behaviour pin: the key never touches the disk, so `link/..` is
        // resolved against the link's own parent even when `link` is a
        // symlink to a directory elsewhere.
        assert_eq!(
            fold_key(Path::new("/w/link/../Top.snxsch"), CaseFold::PRESERVE),
            fold_key(Path::new("/w/Top.snxsch"), CaseFold::PRESERVE),
        );
    }
}
