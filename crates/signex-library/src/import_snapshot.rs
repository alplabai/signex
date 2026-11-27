//! The git side of an imported library (#3114): an imported library is a
//! snapshot of its source, committed once per import. Signex owns the
//! repository; the converter only writes plain files
//! (`docs/LIBRARY_CONVERTER_CONTRACT.md` §5).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

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

/// The file name of the CLI that provides the textconv driver.
const CLI_NAME: &str = if cfg!(windows) {
    "signex-cli.exe"
} else {
    "signex-cli"
};

/// The arguments after the CLI's path in the textconv command.
const TEXTCONV_ARGS: &str = "library textconv";

/// The textconv command for the CLI at `exe`: its absolute path quoted for
/// the shell git runs the driver in.
pub fn footprint_textconv_command(exe: &Path) -> String {
    let quoted = exe.to_string_lossy().replace('\'', "'\\''");
    format!("'{quoted}' {TEXTCONV_ARGS}")
}

/// The `signex-cli` executable, if one is installed: an absolute `PATH`
/// entry first, then the folder of the running app. Release builds ship the
/// app alone, so this is often `None`.
pub fn find_signex_cli() -> Option<PathBuf> {
    let app_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    find_signex_cli_in(std::env::var_os("PATH").as_deref(), app_dir.as_deref())
}

fn find_signex_cli_in(path_var: Option<&OsStr>, app_dir: Option<&Path>) -> Option<PathBuf> {
    let on_path = path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute());
    on_path
        .chain(app_dir.map(Path::to_path_buf))
        .map(|dir| dir.join(CLI_NAME))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

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
    /// HEAD's history holds at least one Signex import commit: the folder
    /// is a library Signex imported, so an update can run over it.
    pub has_import: bool,
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
    let (has_import, local_commits) = count_local_commits(&repo)?;
    Ok(SnapshotState {
        has_repo: true,
        uncommitted,
        local_commits,
        has_import,
    })
}

/// Whether HEAD's history holds an import commit, and the commits on HEAD
/// made after its newest one (all of them when there is none).
fn count_local_commits(repo: &git2::Repository) -> Result<(bool, usize), LibraryError> {
    let Some(tip) = repo.head().ok().and_then(|head| head.target()) else {
        return Ok((false, 0));
    };
    let log = |e: git2::Error| backend("git log", e);
    let walk_from_tip = || -> Result<git2::Revwalk<'_>, LibraryError> {
        let mut walk = repo.revwalk().map_err(log)?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
            .map_err(log)?;
        walk.push(tip).map_err(log)?;
        Ok(walk)
    };
    let mut newest_import = None;
    for oid in walk_from_tip()? {
        let oid = oid.map_err(log)?;
        let commit = repo.find_commit(oid).map_err(log)?;
        if is_import_commit(&String::from_utf8_lossy(commit.message_bytes())) {
            newest_import = Some(oid);
            break;
        }
    }
    let mut walk = walk_from_tip()?;
    if let Some(import) = newest_import {
        walk.hide(import).map_err(log)?;
    }
    Ok((newest_import.is_some(), walk.count()))
}

/// Put the working tree back to HEAD after a failed update over a tree that
/// was clean when it started: tracked files are checked out again and
/// untracked files the run added are removed; ignored files stay. Refused
/// for a library that uses Git LFS, whose checkout would write pointers.
pub fn restore_to_head(root: &Path) -> Result<(), LibraryError> {
    let attributes = std::fs::read_to_string(root.join(".gitattributes")).unwrap_or_default();
    if attributes.contains("filter=lfs") {
        return Err(LibraryError::Conflict(
            "the library uses Git LFS, so it was not restored automatically".into(),
        ));
    }
    let repo = git2::Repository::open(root).map_err(|e| backend("git open", e))?;
    let head = repo
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|e| backend("git head", e))?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.reset(head.as_object(), git2::ResetType::Hard, Some(&mut checkout))
        .map_err(|e| backend("git reset", e))?;
    remove_untracked(&repo, root)
}

/// Remove the untracked, not ignored files under `root` (the reset keeps
/// them), then the folders that emptied.
fn remove_untracked(repo: &git2::Repository, root: &Path) -> Result<(), LibraryError> {
    let mut options = git2::StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    let statuses = repo
        .statuses(Some(&mut options))
        .map_err(|e| backend("git status", e))?;
    for entry in statuses.iter().filter(|e| e.status().is_wt_new()) {
        let Ok(rel) = entry.path() else { continue };
        if is_write_lock_file(Path::new(rel)) {
            continue;
        }
        let path = root.join(rel);
        std::fs::remove_file(&path)
            .map_err(|e| backend(&format!("remove {}", path.display()), e))?;
        for dir in path.ancestors().skip(1).take_while(|d| *d != root) {
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
        }
    }
    Ok(())
}

/// Make `root` an import repository: `git init` when it has no `.git`
/// (with Signex's managed `.gitignore` block), the footprint diff line in
/// `.gitattributes`, and, when a `signex-cli` is installed, the repo-local
/// textconv setting. Without a driver git falls back to the plain text diff.
/// Returns whether this call created the repository. Safe to repeat.
pub fn prepare_snapshot_repo(root: &Path) -> Result<bool, LibraryError> {
    prepare_snapshot_repo_with(root, find_signex_cli().as_deref())
}

fn prepare_snapshot_repo_with(root: &Path, cli: Option<&Path>) -> Result<bool, LibraryError> {
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
    match cli {
        Some(exe) => local
            .set_str(FOOTPRINT_TEXTCONV_KEY, &footprint_textconv_command(exe))
            .map_err(|e| backend("git config diff.snxfpt.textconv", e))?,
        None => tracing::info!(
            target: "signex::library",
            "signex-cli not found: footprint diffs in this library stay plain text"
        ),
    }
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
        let cli = Path::new("/opt/it's/signex-cli");
        assert!(prepare_snapshot_repo_with(dir.path(), Some(cli)).unwrap());
        assert!(!prepare_snapshot_repo_with(dir.path(), Some(cli)).unwrap());
        let attributes = std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
        assert_eq!(attributes.matches(FOOTPRINT_DIFF_ATTRIBUTE).count(), 1);
        let repo = git2::Repository::open(dir.path()).unwrap();
        let config = repo.config().unwrap();
        assert_eq!(
            config.get_string(FOOTPRINT_TEXTCONV_KEY).unwrap(),
            "'/opt/it'\\''s/signex-cli' library textconv"
        );
    }

    #[test]
    fn without_a_signex_cli_the_attribute_is_written_and_textconv_is_not() {
        let dir = library_dir();
        prepare_snapshot_repo_with(dir.path(), None).unwrap();
        let attributes = std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
        assert!(attributes.contains(FOOTPRINT_DIFF_ATTRIBUTE));
        let repo = git2::Repository::open(dir.path()).unwrap();
        assert!(
            repo.config()
                .unwrap()
                .get_string(FOOTPRINT_TEXTCONV_KEY)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn signex_cli_is_found_on_an_absolute_path_entry_or_beside_the_app() {
        use std::os::unix::fs::PermissionsExt;
        let make = |dir: &Path, mode| {
            let exe = dir.join(CLI_NAME);
            std::fs::write(&exe, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(mode)).unwrap();
            exe
        };
        let (bin, app, empty) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        let fake = make(bin.path(), 0o755);
        let path = std::env::join_paths([Path::new("relative"), empty.path(), bin.path()]).unwrap();
        assert_eq!(find_signex_cli_in(Some(&path), None), Some(fake));
        let beside = make(app.path(), 0o755);
        assert_eq!(
            find_signex_cli_in(Some(empty.path().as_os_str()), Some(app.path())),
            Some(beside)
        );
        make(app.path(), 0o644);
        assert_eq!(find_signex_cli_in(None, Some(app.path())), None);
        assert_eq!(find_signex_cli_in(None, None), None);
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
    fn only_commits_after_the_newest_import_are_local() {
        let dir = library_dir();
        prepare_snapshot_repo(dir.path()).unwrap();
        commit_snapshot(dir.path(), IMPORT).unwrap();
        std::fs::write(dir.path().join("a.snxsym"), "a\n").unwrap();
        commit_snapshot(dir.path(), "mine\n").unwrap();
        assert_eq!(read_snapshot_state(dir.path()).unwrap().local_commits, 1);
        std::fs::write(dir.path().join("b.snxsym"), "b\n").unwrap();
        commit_snapshot(dir.path(), IMPORT).unwrap();
        assert_eq!(read_snapshot_state(dir.path()).unwrap().local_commits, 0);
    }

    #[test]
    fn only_a_history_with_an_import_commit_counts_as_imported() {
        let dir = library_dir();
        prepare_snapshot_repo(dir.path()).unwrap();
        assert!(
            !read_snapshot_state(dir.path()).unwrap().has_import,
            "no commit yet"
        );
        commit_snapshot(dir.path(), "mine\n").unwrap();
        let state = read_snapshot_state(dir.path()).unwrap();
        assert!(!state.has_import, "a hand commit is not an import");
        assert_eq!(state.local_commits, 1);
        std::fs::write(dir.path().join("a.snxsym"), "a\n").unwrap();
        commit_snapshot(dir.path(), IMPORT).unwrap();
        assert!(read_snapshot_state(dir.path()).unwrap().has_import);
    }

    #[test]
    fn restore_puts_tracked_files_back_and_drops_new_ones() {
        let dir = library_dir();
        std::fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        prepare_snapshot_repo(dir.path()).unwrap();
        commit_snapshot(dir.path(), IMPORT).unwrap();
        std::fs::write(dir.path().join("Lib.snxlib"), "half written").unwrap();
        std::fs::create_dir_all(dir.path().join("symbols")).unwrap();
        std::fs::write(dir.path().join("symbols/new.snxsym"), "x").unwrap();
        std::fs::write(dir.path().join("keep.log"), "x").unwrap();
        restore_to_head(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Lib.snxlib")).unwrap(),
            "format = \"snxlib/2\"\n"
        );
        assert!(!dir.path().join("symbols/new.snxsym").exists());
        assert!(dir.path().join("keep.log").exists(), "ignored files stay");
        assert!(
            read_snapshot_state(dir.path())
                .unwrap()
                .uncommitted
                .is_empty()
        );
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
