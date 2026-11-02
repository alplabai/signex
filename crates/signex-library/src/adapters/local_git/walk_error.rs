//! Classification of a `walkdir` error raised while listing the symbols folder.

use std::io::ErrorKind;
use std::path::Path;

/// What `scan_symbol_files` does with one walk error.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WalkErrorAction {
    /// The scan fails with a backend error.
    Fail,
    /// The folder vanished: the scan is empty.
    Empty,
    /// Record a problem file; lookups stay undetermined, never NotFound.
    Problem,
    /// The entry is gone or irrelevant; keep walking.
    Skip,
}

/// Only an unreadable root fails the scan. Below it an entry that vanished
/// is gone, and an unreadable stray (non-symbol) file is ignored. A symbol
/// file, or an error with no path (a failed readdir that may hide any
/// symbol), is a problem so the rest of the folder is not silently unlisted.
pub(super) fn walk_error_action(
    depth: usize,
    io_kind: Option<ErrorKind>,
    path: Option<&Path>,
    suffix: &str,
) -> WalkErrorAction {
    let vanished = io_kind == Some(ErrorKind::NotFound);
    if depth == 0 {
        return if vanished {
            WalkErrorAction::Empty
        } else {
            WalkErrorAction::Fail
        };
    }
    if vanished {
        return WalkErrorAction::Skip;
    }
    let Some(path) = path else {
        return WalkErrorAction::Problem;
    };
    let is_symbol_file = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(suffix));
    if is_symbol_file {
        WalkErrorAction::Problem
    } else {
        WalkErrorAction::Skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SFX: &str = ".snxsym";

    #[test]
    fn unreadable_root_fails_and_vanished_root_is_empty() {
        let p = Path::new("/lib/symbols");
        assert_eq!(
            walk_error_action(0, Some(ErrorKind::PermissionDenied), Some(p), SFX),
            WalkErrorAction::Fail
        );
        assert_eq!(walk_error_action(0, None, None, SFX), WalkErrorAction::Fail);
        assert_eq!(
            walk_error_action(0, Some(ErrorKind::NotFound), Some(p), SFX),
            WalkErrorAction::Empty
        );
    }

    #[test]
    fn a_vanished_entry_is_skipped() {
        let p = Path::new("/lib/symbols/a.snxsym");
        assert_eq!(
            walk_error_action(1, Some(ErrorKind::NotFound), Some(p), SFX),
            WalkErrorAction::Skip
        );
    }

    #[test]
    fn an_unreadable_symbol_file_is_a_problem() {
        let p = Path::new("/lib/symbols/a.snxsym");
        assert_eq!(
            walk_error_action(1, Some(ErrorKind::PermissionDenied), Some(p), SFX),
            WalkErrorAction::Problem
        );
    }

    #[test]
    fn an_unreadable_stray_entry_is_skipped() {
        let p = Path::new("/lib/symbols/notes.txt");
        assert_eq!(
            walk_error_action(1, Some(ErrorKind::PermissionDenied), Some(p), SFX),
            WalkErrorAction::Skip
        );
    }

    #[test]
    fn a_pathless_error_is_never_skipped() {
        // A readdir failure mid-iteration carries no path and may hide symbols.
        assert_ne!(
            walk_error_action(1, Some(ErrorKind::Other), None, SFX),
            WalkErrorAction::Skip
        );
        assert_eq!(
            walk_error_action(1, None, None, SFX),
            WalkErrorAction::Problem
        );
    }
}
