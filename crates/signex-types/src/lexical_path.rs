//! Path arithmetic that never touches the filesystem, shared by the app
//! (sheet references, table links) and the exporters (a contents row's link
//! to its PDF page), so both resolve a relative sheet link the same way.

use std::path::{Component, Path, PathBuf};

/// Lexically normalize `path` — resolve `.` and `..` components without
/// touching the filesystem (unlike `std::fs::canonicalize`, which needs the
/// path to exist). A `..` with nothing left to pop is kept as a literal `..`
/// component, so a reference that escapes its root still fails the caller's
/// `starts_with(root)` check instead of silently collapsing onto the root.
pub fn lexically_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                _ => out.push(".."),
            },
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_dot_pops_a_folder_and_dot_is_dropped() {
        assert_eq!(
            lexically_normalize(Path::new("/p/sheets/./../power/buck.snxsch")),
            PathBuf::from("/p/power/buck.snxsch")
        );
    }

    #[test]
    fn a_dot_dot_with_nothing_to_pop_is_kept() {
        assert_eq!(
            lexically_normalize(Path::new("../x.snxsch")),
            PathBuf::from("../x.snxsch")
        );
    }
}
