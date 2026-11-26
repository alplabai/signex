//! The git side of an imported library (#3114): an imported library is a
//! snapshot of its source, committed once per import. Signex owns the
//! repository; the converter only writes plain files
//! (`docs/LIBRARY_CONVERTER_CONTRACT.md` §5).

use std::path::Path;

use crate::adapter::LibraryError;
use crate::adapters::local_git::{is_write_lock_file, lfs, skip_write_lock_files};
use crate::git_identity::identity_for_repo;

/// The trailer key that marks a commit as an import.
pub const IMPORT_TRAILER: &str = "Signex-Import";

/// The `.gitattributes` line that routes footprint diffs through textconv
/// (`docs/CLI.md`, "Library tools").
pub const FOOTPRINT_DIFF_ATTRIBUTE: &str = "*.snxfpt diff=snxfpt";

/// The repo-local setting that names the textconv driver.
pub const FOOTPRINT_TEXTCONV_KEY: &str = "diff.snxfpt.textconv";

/// The textconv command for footprint files.
pub const FOOTPRINT_TEXTCONV: &str = "signex-cli library textconv";

/// What a library folder's repository holds before an update.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SnapshotState {
    /// The folder is the root of a git repository.
    pub has_repo: bool,
    /// Working-tree paths that differ from the last commit, untracked ones
    /// included.
    pub uncommitted: Vec<String>,
    /// Commits reachable from HEAD that are not imports.
    pub local_commits: usize,
}

fn backend(what: &str, e: impl std::fmt::Display) -> LibraryError {
    LibraryError::Backend(format!("{what}: {e}"))
}

/// Whether `message` carries the [`IMPORT_TRAILER`] in its last paragraph
/// (the trailer block). The subject never counts.
pub fn is_import_commit(message: &str) -> bool {
    let message = message.replace("\r\n", "\n");
    let paragraphs: Vec<&str> = message
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let [_, .., trailers] = paragraphs.as_slice() else {
        return false;
    };
    trailers.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(key, value)| key == IMPORT_TRAILER && !value.trim().is_empty())
    })
}

/// Read the repository at `root`. A folder with no `.git` reports
/// `has_repo: false` and nothing else.
pub fn read_snapshot_state(root: &Path) -> Result<SnapshotState, LibraryError> {
    if !root.join(".git").exists() {
        return Ok(SnapshotState::default());
    }
    let repo = git2::Repository::open(root).map_err(|e| backend("git open", e))?;
    let mut options = git2::StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    let statuses = repo
        .statuses(Some(&mut options))
        .map_err(|e| backend("git status", e))?;
    let uncommitted = statuses
        .iter()
        .filter(|entry| entry.status() != git2::Status::CURRENT)
        .filter_map(|entry| entry.path().ok().map(str::to_owned))
        .filter(|path| !is_write_lock_file(Path::new(path)))
        .collect();
    Ok(SnapshotState {
        has_repo: true,
        uncommitted,
        local_commits: count_local_commits(&repo)?,
    })
}

fn count_local_commits(repo: &git2::Repository) -> Result<usize, LibraryError> {
    let Ok(head) = repo.head() else {
        return Ok(0);
    };
    let Some(tip) = head.target() else {
        return Ok(0);
    };
    let mut walk = repo.revwalk().map_err(|e| backend("git log", e))?;
    walk.push(tip).map_err(|e| backend("git log", e))?;
    let mut count = 0;
    for oid in walk {
        let oid = oid.map_err(|e| backend("git log", e))?;
        let commit = repo.find_commit(oid).map_err(|e| backend("git log", e))?;
        if !is_import_commit(&String::from_utf8_lossy(commit.message_bytes())) {
            count += 1;
        }
    }
    Ok(count)
}

/// Make `root` an import repository: `git init` when it has no `.git`
/// (with Signex's managed `.gitignore` block), the footprint diff line in
/// `.gitattributes`, and the repo-local textconv setting. Returns whether
/// this call created the repository. Safe to repeat.
pub fn prepare_snapshot_repo(root: &Path) -> Result<bool, LibraryError> {
    let created = !root.join(".git").exists();
    if created {
        let mut files = crate::vcs_init_files::VcsFilePlan::new(root, false, false, None, true)?;
        files.write()?;
        git2::Repository::init(root).map_err(|e| backend("git init", e))?;
    }
    ensure_footprint_attribute(root)?;
    let repo = git2::Repository::open(root).map_err(|e| backend("git open", e))?;
    let config = repo.config().map_err(|e| backend("git config", e))?;
    let mut local = config
        .open_level(git2::ConfigLevel::Local)
        .map_err(|e| backend("git config", e))?;
    local
        .set_str(FOOTPRINT_TEXTCONV_KEY, FOOTPRINT_TEXTCONV)
        .map_err(|e| backend("git config diff.snxfpt.textconv", e))?;
    Ok(created)
}

/// Add [`FOOTPRINT_DIFF_ATTRIBUTE`] to `.gitattributes` unless a line
/// already says it; the rest of the file is kept.
fn ensure_footprint_attribute(root: &Path) -> Result<(), LibraryError> {
    let path = root.join(".gitattributes");
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(backend(&format!("read {}", path.display()), e)),
    };
    let merged =
        signex_types::managed_block::append_missing_lines(&existing, FOOTPRINT_DIFF_ATTRIBUTE);
    if merged == existing {
        return Ok(());
    }
    signex_types::atomic_io::atomic_write(&path, merged.as_bytes())
        .map_err(|e| backend(&format!("write {}", path.display()), e))
}

/// Stage every file under `root` (deletions included, write-lock files
/// left out) and commit it on HEAD with `message`. `Ok(None)` when the tree
/// equals HEAD's, so an unchanged re-import adds no commit.
pub fn commit_snapshot(root: &Path, message: &str) -> Result<Option<String>, LibraryError> {
    let repo = git2::Repository::open(root).map_err(|e| backend("git open", e))?;
    let mut index = repo.index().map_err(|e| backend("git index", e))?;
    index
        .add_all(
            ["."].iter(),
            git2::IndexAddOption::DEFAULT,
            Some(&mut skip_write_lock_files),
        )
        .map_err(|e| backend("git add", e))?;
    index
        .update_all(["."].iter(), None)
        .map_err(|e| backend("git add (removals)", e))?;
    let converted = lfs::convert_index(&repo, &mut index).map_err(|e| backend("git lfs", e))?;
    if converted > 0 {
        lfs::install_pre_push_hook(&repo).map_err(|e| backend("git lfs hook", e))?;
    }
    index.write().map_err(|e| backend("git index write", e))?;
    let tree_id = index
        .write_tree()
        .map_err(|e| backend("git write tree", e))?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    if parent.as_ref().is_some_and(|p| p.tree_id() == tree_id) {
        return Ok(None);
    }
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| backend("git find tree", e))?;
    let (name, email) = identity_for_repo(&repo)?;
    let signature = git2::Signature::now(&name, &email).map_err(|e| backend("git signature", e))?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .map_err(|e| backend("git commit", e))?;
    Ok(Some(oid.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_dir() -> tempfile::TempDir {
        crate::git_identity::test_identity::install();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Lib.snxlib"), "format = \"snxlib/2\"\n").unwrap();
        dir
    }

    const IMPORT: &str = "Import Lib\n\nSignex-Import: example-import 1.0\n";

    #[test]
    fn a_fresh_folder_gets_a_repo_attributes_and_textconv() {
        let dir = library_dir();
        assert!(prepare_snapshot_repo(dir.path()).unwrap());
        assert!(!prepare_snapshot_repo(dir.path()).unwrap(), "repeatable");
        let attributes = std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
        assert_eq!(attributes.matches(FOOTPRINT_DIFF_ATTRIBUTE).count(), 1);
        let repo = git2::Repository::open(dir.path()).unwrap();
        let config = repo.config().unwrap();
        assert_eq!(
            config.get_string(FOOTPRINT_TEXTCONV_KEY).unwrap(),
            FOOTPRINT_TEXTCONV
        );
    }

    #[test]
    fn an_existing_attributes_file_keeps_its_lines() {
        let dir = library_dir();
        std::fs::write(dir.path().join(".gitattributes"), "*.step filter=lfs\n").unwrap();
        prepare_snapshot_repo(dir.path()).unwrap();
        let attributes = std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
        assert_eq!(attributes, "*.step filter=lfs\n*.snxfpt diff=snxfpt\n");
    }

    #[test]
    fn commits_count_and_an_unchanged_tree_adds_none() {
        let dir = library_dir();
        prepare_snapshot_repo(dir.path()).unwrap();
        assert!(commit_snapshot(dir.path(), IMPORT).unwrap().is_some());
        assert_eq!(commit_snapshot(dir.path(), IMPORT).unwrap(), None);
        let state = read_snapshot_state(dir.path()).unwrap();
        assert!(state.has_repo);
        assert!(state.uncommitted.is_empty(), "{state:?}");
        assert_eq!(state.local_commits, 0);
    }

    #[test]
    fn a_removed_file_is_committed_as_removed() {
        let dir = library_dir();
        std::fs::write(dir.path().join("old.snxsym"), "x\n").unwrap();
        prepare_snapshot_repo(dir.path()).unwrap();
        commit_snapshot(dir.path(), IMPORT).unwrap();
        std::fs::remove_file(dir.path().join("old.snxsym")).unwrap();
        commit_snapshot(dir.path(), IMPORT).unwrap().unwrap();
        let repo = git2::Repository::open(dir.path()).unwrap();
        let tree = repo.head().unwrap().peel_to_tree().unwrap();
        assert!(tree.get_name("old.snxsym").is_none());
    }

    #[test]
    fn edits_and_hand_commits_are_reported() {
        let dir = library_dir();
        prepare_snapshot_repo(dir.path()).unwrap();
        commit_snapshot(dir.path(), IMPORT).unwrap();
        std::fs::write(dir.path().join("mine.snxsym"), "x\n").unwrap();
        let state = read_snapshot_state(dir.path()).unwrap();
        assert_eq!(state.uncommitted, ["mine.snxsym"]);
        commit_snapshot(dir.path(), "my own symbol\n").unwrap();
        assert_eq!(read_snapshot_state(dir.path()).unwrap().local_commits, 1);
    }

    #[test]
    fn only_a_trailer_marks_an_import() {
        assert!(is_import_commit(IMPORT));
        assert!(!is_import_commit("Signex-Import: in the subject only\n"));
        assert!(!is_import_commit("fix a pad\n"));
    }

    #[test]
    fn a_folder_without_git_has_no_state() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            read_snapshot_state(dir.path()).unwrap(),
            SnapshotState::default()
        );
    }
}
