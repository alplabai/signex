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
        key = key.to_ascii_lowercase();
    }
    if fold.backslash_is_separator {
        key = key.replace('\\', "/");
    }
    collapse_dot_segments(&key)
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
