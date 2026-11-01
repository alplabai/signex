//! Which kind of file a path names, judged by its extension in any letter
//! case (#1832).
//!
//! A file opened as `Board.SNXPCB` is a board, so every later decision about
//! it (how a save completes, whether a disk baseline is kept, whether a path
//! is a library package) must give the same answer the open did. They all ask
//! here, so open and the decisions after it cannot disagree.

use std::path::Path;

/// Whether `path` carries extension `ext` (given without the dot), ignoring
/// ASCII letter case. Every native extension is ASCII.
pub(crate) fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

#[cfg(test)]
mod tests {
    use super::has_extension;
    use std::path::Path;

    #[test]
    fn an_extension_matches_in_any_letter_case() {
        assert!(has_extension(Path::new("Board.snxpcb"), "snxpcb"));
        assert!(has_extension(Path::new("Board.SNXPCB"), "snxpcb"));
        assert!(has_extension(Path::new("Board.SnxPcb"), "snxpcb"));
    }

    #[test]
    fn a_different_or_missing_extension_does_not_match() {
        assert!(!has_extension(Path::new("Board.snxsch"), "snxpcb"));
        assert!(!has_extension(Path::new("snxpcb"), "snxpcb"));
        assert!(!has_extension(Path::new("Board.snxpcb.bak"), "snxpcb"));
    }
}
