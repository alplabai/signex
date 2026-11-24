//! The lines one commit changed in one file, for the History panel's Show
//! differences… (mockup row 39). Read only.

use std::path::Path;

use crate::adapter::LibraryError;

/// Most lines one diff carries; the rest is cut and the diff says so.
pub const MAX_DIFF_LINES: usize = 2000;

/// Unchanged lines kept around each change.
const CONTEXT_LINES: u32 = 3;

/// What a line of a diff is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

/// One line of a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// Its number in the parent's file; `None` for an added line.
    pub old_line: Option<u32>,
    /// Its number in the commit's file; `None` for a removed line.
    pub new_line: Option<u32>,
    /// The line without its line ending.
    pub text: String,
}

/// A run of changed lines with their context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<DiffLine>,
}

/// What can be shown of the file's change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDiffBody {
    /// No hunks: the commit changed no line of the file.
    Text {
        hunks: Vec<DiffHunk>,
        /// More than [`MAX_DIFF_LINES`] lines; the rest is not listed.
        truncated: bool,
    },
    /// Not text, so there are no lines to compare.
    Binary,
}

/// The change `sha` made to one file against its first parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Relative to the repository root, forward slashes.
    pub path: String,
    pub added: usize,
    pub removed: usize,
    pub body: FileDiffBody,
}

fn backend(what: &str, e: git2::Error) -> LibraryError {
    LibraryError::Backend(format!("{what}: {e}"))
}

fn line_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(['\n', '\r'])
        .to_owned()
}

/// The lines `sha` changed in `rel_path` (forward slashes) against its first
/// parent, against nothing for a root commit.
pub fn commit_file_diff(
    project_dir: &Path,
    sha: &str,
    rel_path: &str,
) -> Result<FileDiff, LibraryError> {
    let repo = git2::Repository::open(project_dir).map_err(|e| backend("git open", e))?;
    let oid = git2::Oid::from_str(sha).map_err(|e| backend("git sha", e))?;
    let commit = repo
        .find_commit(oid)
        .map_err(|e| backend("git find commit", e))?;
    let tree = commit.tree().map_err(|e| backend("git commit tree", e))?;
    let parent = match commit.parent(0) {
        Ok(parent) => Some(parent.tree().map_err(|e| backend("git parent tree", e))?),
        Err(_) => None,
    };
    let mut options = git2::DiffOptions::new();
    options
        .pathspec(rel_path)
        .disable_pathspec_match(true)
        .context_lines(CONTEXT_LINES);
    let diff = repo
        .diff_tree_to_tree(parent.as_ref(), Some(&tree), Some(&mut options))
        .map_err(|e| backend("git diff", e))?;
    let unchanged = FileDiff {
        path: rel_path.to_owned(),
        added: 0,
        removed: 0,
        body: FileDiffBody::Text {
            hunks: Vec::new(),
            truncated: false,
        },
    };
    if diff.deltas().len() == 0 {
        return Ok(unchanged);
    }
    let binary = FileDiff {
        body: FileDiffBody::Binary,
        ..unchanged
    };
    let Some(patch) = git2::Patch::from_diff(&diff, 0).map_err(|e| backend("git patch", e))? else {
        return Ok(binary);
    };
    if patch.delta().flags().is_binary() {
        return Ok(binary);
    }
    let (_, added, removed) = patch.line_stats().map_err(|e| backend("git stats", e))?;
    let (hunks, truncated) = read_hunks(&patch)?;
    Ok(FileDiff {
        added,
        removed,
        body: FileDiffBody::Text { hunks, truncated },
        ..binary
    })
}

/// The hunks of `patch`, cut after [`MAX_DIFF_LINES`] lines.
fn read_hunks(patch: &git2::Patch<'_>) -> Result<(Vec<DiffHunk>, bool), LibraryError> {
    let mut hunks = Vec::new();
    let mut listed = 0;
    for h in 0..patch.num_hunks() {
        let (hunk, count) = patch.hunk(h).map_err(|e| backend("git hunk", e))?;
        let mut lines = Vec::new();
        for l in 0..count {
            if listed == MAX_DIFF_LINES {
                if !lines.is_empty() {
                    hunks.push(DiffHunk {
                        old_start: hunk.old_start(),
                        new_start: hunk.new_start(),
                        lines,
                    });
                }
                return Ok((hunks, true));
            }
            let line = patch
                .line_in_hunk(h, l)
                .map_err(|e| backend("git line", e))?;
            // The end-of-file newline markers are not lines of the file.
            let kind = match line.origin() {
                '+' => DiffLineKind::Added,
                '-' => DiffLineKind::Removed,
                ' ' => DiffLineKind::Context,
                _ => continue,
            };
            lines.push(DiffLine {
                kind,
                old_line: line.old_lineno(),
                new_line: line.new_lineno(),
                text: line_text(line.content()),
            });
            listed += 1;
        }
        hunks.push(DiffHunk {
            old_start: hunk.old_start(),
            new_start: hunk.new_start(),
            lines,
        });
    }
    Ok((hunks, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit_tree(repo: &git2::Repository, msg: &str) -> git2::Oid {
        let mut index = repo.index().unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .unwrap()
    }

    fn commit_file(repo: &git2::Repository, path: &str, body: &[u8], msg: &str) -> String {
        std::fs::write(repo.workdir().unwrap().join(path), body).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();
        commit_tree(repo, msg).to_string()
    }

    fn kinds(diff: &FileDiff) -> Vec<(DiffLineKind, String)> {
        let FileDiffBody::Text { hunks, .. } = &diff.body else {
            panic!("a text diff: {diff:?}");
        };
        hunks
            .iter()
            .flat_map(|h| h.lines.iter().map(|l| (l.kind, l.text.clone())))
            .collect()
    }

    #[test]
    fn an_edit_lists_the_removed_and_added_lines_with_their_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit_file(&repo, "a.snxsch", b"one\ntwo\nthree\n", "base");
        let edit = commit_file(&repo, "a.snxsch", b"one\nTWO\nthree\nfour\n", "edit");

        let diff = commit_file_diff(dir.path(), &edit, "a.snxsch").unwrap();

        assert_eq!((diff.added, diff.removed), (2, 1));
        use DiffLineKind::{Added, Context, Removed};
        assert_eq!(
            kinds(&diff),
            [
                (Context, "one".to_owned()),
                (Removed, "two".to_owned()),
                (Added, "TWO".to_owned()),
                (Context, "three".to_owned()),
                (Added, "four".to_owned()),
            ]
        );
        let FileDiffBody::Text { hunks, truncated } = &diff.body else {
            unreachable!();
        };
        assert!(!truncated);
        let removed = &hunks[0].lines[1];
        assert_eq!((removed.old_line, removed.new_line), (Some(2), None));
        let added = &hunks[0].lines[4];
        assert_eq!((added.old_line, added.new_line), (None, Some(4)));
    }

    #[test]
    fn a_root_commit_adds_every_line() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let root = commit_file(&repo, "a.snxsch", b"one\ntwo\n", "base");
        let diff = commit_file_diff(dir.path(), &root, "a.snxsch").unwrap();
        assert_eq!((diff.added, diff.removed), (2, 0));
        assert!(kinds(&diff).iter().all(|(k, _)| *k == DiffLineKind::Added));
    }

    #[test]
    fn a_deletion_removes_every_line() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit_file(&repo, "a.snxsch", b"one\ntwo\n", "base");
        std::fs::remove_file(dir.path().join("a.snxsch")).unwrap();
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new("a.snxsch")).unwrap();
        index.write().unwrap();
        let gone = commit_tree(&repo, "remove").to_string();

        let diff = commit_file_diff(dir.path(), &gone, "a.snxsch").unwrap();

        assert_eq!((diff.added, diff.removed), (0, 2));
        assert!(
            kinds(&diff)
                .iter()
                .all(|(k, _)| *k == DiffLineKind::Removed)
        );
    }

    #[test]
    fn a_binary_file_has_no_lines_to_compare() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit_file(&repo, "Q2.step", b"\0\x01\x02", "base");
        let edit = commit_file(&repo, "Q2.step", b"\0\x03\x04", "edit");
        let diff = commit_file_diff(dir.path(), &edit, "Q2.step").unwrap();
        assert_eq!(diff.body, FileDiffBody::Binary);
    }

    /// Another file of the commit, or a name the file only starts with,
    /// is not the file asked for.
    #[test]
    fn a_file_the_commit_left_alone_has_no_lines() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit_file(&repo, "a.snxsch", b"one\n", "base");
        let other = commit_file(&repo, "a.snxsch.bak", b"two\n", "other");
        let diff = commit_file_diff(dir.path(), &other, "a.snxsch").unwrap();
        assert_eq!((diff.added, diff.removed), (0, 0));
        assert!(kinds(&diff).is_empty());
    }

    #[test]
    fn a_long_diff_is_cut_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let body: String = (0..MAX_DIFF_LINES + 10).map(|n| format!("{n}\n")).collect();
        let root = commit_file(&repo, "a.snxsch", body.as_bytes(), "base");

        let diff = commit_file_diff(dir.path(), &root, "a.snxsch").unwrap();

        assert_eq!(
            diff.added,
            MAX_DIFF_LINES + 10,
            "the counts are the whole change"
        );
        let FileDiffBody::Text { truncated, .. } = &diff.body else {
            unreachable!();
        };
        assert!(truncated);
        assert_eq!(kinds(&diff).len(), MAX_DIFF_LINES);
    }

    #[test]
    fn an_unknown_commit_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit_file(&repo, "a.snxsch", b"one\n", "base");
        let missing = "0".repeat(40);
        assert!(commit_file_diff(dir.path(), &missing, "a.snxsch").is_err());
    }
}
