//! The managed `.gitignore` block in a library repository (#1729): OS and
//! temp clutter (`.DS_Store`, stranded `*.tmp`, write-lock sidecars) must
//! never reach the commit ledger or a commit. Why the block is merged at
//! the next commit and not on open: `docs/architecture/library-writes.md`,
//! "Library repositories and the commit ledger".

use std::path::Path;

use signex_types::project_vcs_files::{GITIGNORE_BLOCK_BODY, ensure_project_gitignore, merge_file};

use super::*;

pub(super) const GITIGNORE_FILE: &str = ".gitignore";

/// Write the managed block into `root_dir/.gitignore`, keeping any lines
/// already there. Used when a library is created with version control.
pub(super) fn write_ignore_rules(root_dir: &Path) -> Result<(), LibraryError> {
    ensure_project_gitignore(root_dir)
        .map_err(|e| LibraryError::Backend(format!("write .gitignore: {e}")))
}

/// Make `repo` ignore what the managed block ignores, in memory only, so an
/// older library whose `.gitignore` lacks the block reads the same as a new
/// one. Nothing is written.
pub(super) fn apply_managed_rules(repo: &git2::Repository) -> Result<(), git2::Error> {
    repo.add_ignore_rule(GITIGNORE_BLOCK_BODY)
}

/// Whether `rel` is clutter the managed block names. A rule without a slash
/// matches the file name in any directory; one with a slash
/// (`.signex/agent-journal.lock`) is anchored at the root and matches the
/// whole path.
pub(super) fn is_managed_clutter(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    GITIGNORE_BLOCK_BODY
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .any(|rule| {
            let Ok(spec) = git2::Pathspec::new([rule]) else {
                return false;
            };
            let subject = if rule.contains('/') { rel } else { name };
            spec.matches_path(Path::new(subject), git2::PathspecFlags::DEFAULT)
        })
}

impl LocalGitAdapter {
    /// Merge the managed block into the library's `.gitignore` before a
    /// commit. `Some` when the file changed, so the commit records it. A
    /// file that cannot be merged (broken markers, a link, read-only) is
    /// left alone, warned about once per cause, and the commit goes on.
    pub(super) fn merge_ignore_rules_for_commit(&self) -> Option<String> {
        let path = self.root_dir.join(GITIGNORE_FILE);
        let merged = match merge_file(&path, GITIGNORE_BLOCK_BODY) {
            Ok(merged) if merged.is_changed() => merged,
            Ok(_) => return None,
            Err(e) => {
                self.warn_ignore_rules_once(&e.to_string());
                return None;
            }
        };
        if let Err(e) = signex_types::atomic_io::atomic_write(&path, merged.merged.as_bytes()) {
            self.warn_ignore_rules_once(&format!("write .gitignore: {e}"));
            return None;
        }
        Some(GITIGNORE_FILE.to_owned())
    }

    fn warn_ignore_rules_once(&self, reason: &str) {
        if self.note_reported(reason) {
            tracing::warn!(
                target: "signex::library",
                "Could not add Signex's ignore rules to {}: {reason}",
                self.root_dir.join(GITIGNORE_FILE).display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_managed_clutter;

    #[test]
    fn a_rule_with_a_slash_matches_the_whole_path_and_one_without_the_name() {
        assert!(is_managed_clutter(".signex/agent-journal.lock"));
        assert!(is_managed_clutter("sub/dir/.DS_Store"));
        // Anchored at the root: the same name elsewhere is not the lock.
        assert!(!is_managed_clutter("sub/.signex/agent-journal.lock"));
        assert!(!is_managed_clutter(".signex/agent-edits.jsonl"));
    }
}
