//! Whether a path read from a git tree may be written under the working
//! tree. The branch switch writes LFS files itself, so libgit2's own checks
//! do not cover them.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

/// Characters HFS+ ignores when it compares file names.
fn is_hfs_ignorable(c: char) -> bool {
    matches!(
        c,
        '\u{200C}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{206A}'..='\u{206F}' | '\u{FEFF}'
    )
}

/// Whether `name` reaches the `.git` folder on any filesystem: its NTFS short
/// name, a trailing dot or space, a stream suffix, or HFS+ ignorable marks.
fn names_git_folder(name: &str) -> bool {
    let bare: String = name.chars().filter(|c| !is_hfs_ignorable(*c)).collect();
    let stem = bare
        .split(':')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' ']);
    stem.eq_ignore_ascii_case(".git") || stem.eq_ignore_ascii_case("git~1")
}

/// Only plain names, none of them the `.git` folder.
fn has_safe_names(rel: &Path) -> bool {
    let mut has_name = false;
    for component in rel.components() {
        let Component::Normal(name) = component else {
            return false;
        };
        let name = name.to_string_lossy();
        if names_git_folder(&name) || (cfg!(windows) && name.contains(':')) {
            return false;
        }
        has_name = true;
    }
    has_name
}

/// No folder on the way to `rel` is a link. A folder not there yet is fine:
/// the write creates it.
fn parents_are_real_folders(workdir: &Path, rel: &Path) -> std::io::Result<bool> {
    let Some(parent) = rel.parent() else {
        return Ok(true);
    };
    let mut at = workdir.to_path_buf();
    for part in parent.components() {
        at.push(part);
        match fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return Ok(false),
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
                return Ok(true);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

/// Whether writing or removing `rel` under `workdir` stays inside the working
/// tree and out of `.git`. A parent that cannot be read counts as unsafe.
pub(crate) fn is_safe_to_write(workdir: &Path, rel: &Path) -> bool {
    has_safe_names(rel) && parents_are_real_folders(workdir, rel).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_path_is_safe() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_safe_to_write(dir.path(), Path::new("models/m.step")));
        assert!(is_safe_to_write(dir.path(), Path::new(".github/m.step")));
    }

    #[test]
    fn a_path_leaving_the_tree_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for rel in ["../x", "a/../../x", "./x", ""] {
            assert!(!is_safe_to_write(dir.path(), Path::new(rel)), "{rel}");
        }
        let absolute = dir.path().join("x");
        assert!(!is_safe_to_write(dir.path(), &absolute));
    }

    #[test]
    fn every_spelling_of_the_git_folder_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            ".git/hooks/x",
            ".GIT/hooks/x",
            "a/.git/x",
            "git~1/hooks/x",
            ".git./hooks/x",
            ".git /hooks/x",
            ".git::$INDEX_ALLOCATION/x",
            ".g\u{200C}it/hooks/x",
        ] {
            assert!(!is_safe_to_write(dir.path(), Path::new(rel)), "{rel}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_parent_folder_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        assert!(!is_safe_to_write(dir.path(), Path::new("link/x")));
        assert!(!is_safe_to_write(dir.path(), Path::new("link/deep/x")));
    }
}
