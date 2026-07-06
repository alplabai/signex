//! v0.22 Phase 8.1 — `LocalGitProjectAdapter` integration tests.
//!
//! Mirrors `local_git_adapter.rs`'s 9-scenario coverage at the
//! project-scope level. Each test creates a tempdir representing a
//! project root (containing a fake `.snxprj` + sibling `.snxsch` /
//! `.snxpcb`) and walks the adapter through realistic save +
//! commit + history + restore flows.
//!
//! These exercise the public API only — internal helpers like
//! `commit_touches_path` are tested indirectly via `file_history`.

#![cfg(feature = "local-git")]

use std::fs;
use std::path::Path;

use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use tempfile::TempDir;

fn write_file(root: &Path, rel: &str, content: &str) {
    let abs = root.join(rel);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&abs, content).unwrap();
}

fn read_file(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn open_or_init_creates_dot_git_directory() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    assert!(!root.join(".git").exists());

    let _adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    assert!(
        root.join(".git").is_dir(),
        ".git directory should be created on first open"
    );
}

#[test]
fn open_or_init_is_idempotent_on_existing_repo() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let _a1 = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    // Open a second time — must not blow away history or fail.
    let _a2 = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    assert!(root.join(".git").is_dir());
}

#[test]
fn open_or_init_fails_when_root_is_not_a_directory() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("not-a-dir.txt");
    fs::write(&file_path, "x").unwrap();

    let err = LocalGitProjectAdapter::open_or_init(file_path).unwrap_err();
    assert!(format!("{err:?}").contains("not a directory"));
}

#[test]
fn commit_path_creates_first_commit_on_unborn_head() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    write_file(&root, "my.snxprj", "[project]\nname = \"my\"\n");
    let oid = adapter
        .commit_path(Path::new("my.snxprj"), "Initial commit")
        .unwrap();
    assert_ne!(oid.to_string(), "0000000000000000000000000000000000000000");

    // History should now have one entry.
    let entries = adapter.file_history(Path::new("my.snxprj"), 10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].subject, "Initial commit");
}

#[test]
fn commit_path_chains_subsequent_commits() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    write_file(&root, "my.snxprj", "v1");
    adapter
        .commit_path(Path::new("my.snxprj"), "v1")
        .unwrap();
    write_file(&root, "my.snxprj", "v2");
    adapter
        .commit_path(Path::new("my.snxprj"), "v2")
        .unwrap();
    write_file(&root, "my.snxprj", "v3");
    adapter
        .commit_path(Path::new("my.snxprj"), "v3")
        .unwrap();

    let entries = adapter.file_history(Path::new("my.snxprj"), 10).unwrap();
    assert_eq!(entries.len(), 3);
    // Newest first.
    assert_eq!(entries[0].subject, "v3");
    assert_eq!(entries[1].subject, "v2");
    assert_eq!(entries[2].subject, "v1");
}

#[test]
fn file_history_filters_by_path() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    // Two unrelated files.
    write_file(&root, "main.snxsch", "schematic v1");
    adapter
        .commit_path(Path::new("main.snxsch"), "sch v1")
        .unwrap();
    write_file(&root, "board.snxpcb", "pcb v1");
    adapter
        .commit_path(Path::new("board.snxpcb"), "pcb v1")
        .unwrap();
    write_file(&root, "main.snxsch", "schematic v2");
    adapter
        .commit_path(Path::new("main.snxsch"), "sch v2")
        .unwrap();

    let sch_history = adapter.file_history(Path::new("main.snxsch"), 10).unwrap();
    assert_eq!(sch_history.len(), 2);
    assert_eq!(sch_history[0].subject, "sch v2");
    assert_eq!(sch_history[1].subject, "sch v1");

    let pcb_history = adapter.file_history(Path::new("board.snxpcb"), 10).unwrap();
    assert_eq!(pcb_history.len(), 1);
    assert_eq!(pcb_history[0].subject, "pcb v1");
}

#[test]
fn file_history_respects_limit() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    for i in 0..5 {
        write_file(&root, "x.snxsch", &format!("v{i}"));
        adapter
            .commit_path(Path::new("x.snxsch"), &format!("commit {i}"))
            .unwrap();
    }
    let entries = adapter.file_history(Path::new("x.snxsch"), 3).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].subject, "commit 4");
    assert_eq!(entries[2].subject, "commit 2");
}

#[test]
fn file_history_empty_on_unborn_head() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    // Just init'd — no commits yet.
    let entries = adapter.file_history(Path::new("x.snxsch"), 10).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn restore_at_round_trips_a_prior_version() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    write_file(&root, "sheet.snxsch", "version-A content");
    let oid_a = adapter
        .commit_path(Path::new("sheet.snxsch"), "A")
        .unwrap();
    write_file(&root, "sheet.snxsch", "version-B content");
    let _oid_b = adapter
        .commit_path(Path::new("sheet.snxsch"), "B")
        .unwrap();

    // Working tree currently shows B.
    assert_eq!(read_file(&root, "sheet.snxsch"), "version-B content");

    // Restore A — file content reverts.
    adapter
        .restore_at(Path::new("sheet.snxsch"), oid_a)
        .unwrap();
    assert_eq!(read_file(&root, "sheet.snxsch"), "version-A content");
}

#[test]
fn write_gitattributes_writes_the_v022_spec_with_lfs_off() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    adapter.write_gitattributes(false).unwrap();
    let body = fs::read_to_string(root.join(".gitattributes")).unwrap();
    assert!(body.contains("*.snxsch\ttext eol=lf"));
    assert!(body.contains("*.snxpcb\ttext eol=lf"));
    assert!(body.contains("*.snxprj\ttext eol=lf"));
    assert!(body.contains("*.step\tbinary"));
    assert!(body.contains("*.pdf\tbinary"));
    assert!(
        !body.contains("filter=lfs"),
        "LFS line must not appear when use_lfs=false"
    );
}

#[test]
fn write_gitattributes_includes_lfs_filter_when_use_lfs_is_on() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    adapter.write_gitattributes(true).unwrap();
    let body = fs::read_to_string(root.join(".gitattributes")).unwrap();
    assert!(
        body.contains("assets/3d-models/**\tfilter=lfs diff=lfs merge=lfs -text"),
        "LFS line must appear when use_lfs=true; got:\n{body}"
    );
}

#[test]
fn restore_at_from_sha_round_trips_via_string_oid() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    write_file(&root, "x.snxsch", "v1");
    let oid_a = adapter
        .commit_path(Path::new("x.snxsch"), "v1")
        .unwrap();
    write_file(&root, "x.snxsch", "v2");
    adapter.commit_path(Path::new("x.snxsch"), "v2").unwrap();

    // Restore by string SHA — what the History panel hands us.
    adapter
        .restore_at_from_sha(Path::new("x.snxsch"), &oid_a.to_string())
        .unwrap();
    assert_eq!(read_file(&root, "x.snxsch"), "v1");
}

#[test]
fn restore_at_from_sha_rejects_invalid_sha() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    let err = adapter
        .restore_at_from_sha(Path::new("x.snxsch"), "not-a-sha")
        .unwrap_err();
    assert!(format!("{err:?}").contains("invalid sha"));
}

#[test]
fn write_gitattributes_overwrites_existing_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    fs::write(root.join(".gitattributes"), "stale-content").unwrap();
    adapter.write_gitattributes(false).unwrap();
    let body = fs::read_to_string(root.join(".gitattributes")).unwrap();
    assert!(!body.contains("stale-content"));
    assert!(body.contains("*.snxsch"));
}

#[test]
fn commit_external_change_creates_a_user_edit_commit() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    write_file(&root, "sheet.snxsch", "in-app edit");
    adapter
        .commit_path(Path::new("sheet.snxsch"), "in-app save")
        .unwrap();

    // Simulate the user editing the file in a text editor outside
    // of Signex.
    write_file(&root, "sheet.snxsch", "out-of-app edit");
    let abs = root.join("sheet.snxsch");
    let _oid = adapter
        .commit_external_change(&abs, "")
        .unwrap();

    let entries = adapter.file_history(Path::new("sheet.snxsch"), 10).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(
        entries[0].subject.contains("User edit (out of app)"),
        "got subject: {}",
        entries[0].subject
    );
}

#[test]
fn commit_path_rejects_absolute_rel_path() {
    // v0.23 — `commit_path` guards against `rel_path` escaping the
    // project root. An absolute path would land git2's `add_path`
    // outside the working tree and corrupt the index.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    let abs_path = if cfg!(windows) {
        Path::new("C:\\Windows\\System32\\drivers\\etc\\hosts")
    } else {
        Path::new("/etc/passwd")
    };
    let err = adapter
        .commit_path(abs_path, "should fail")
        .expect_err("absolute rel_path must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("project-relative") || msg.contains("`..`"),
        "expected guard message, got {msg}"
    );
}

#[test]
fn commit_path_rejects_parent_dir_traversal() {
    // v0.23 — `..` components in `rel_path` would break out of the
    // project root. Reject them in the guard.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    let err = adapter
        .commit_path(Path::new("../escape.txt"), "should fail")
        .expect_err("parent-dir rel_path must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("project-relative") || msg.contains("`..`"),
        "expected guard message, got {msg}"
    );
}

#[test]
fn commit_path_populates_history_diff_stats() {
    // v0.23 — file_history rows now carry `additions`, `deletions`,
    // and `files_changed` populated from `Diff::stats`. v0.22 left
    // them empty, which made the History panel unable to render
    // change summaries.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    // First commit — 3 lines added (root commit path counts newlines).
    write_file(&root, "main.snxsch", "line1\nline2\nline3\n");
    adapter
        .commit_path(Path::new("main.snxsch"), "v1")
        .unwrap();

    // Second commit — 2 lines added, 1 deleted relative to the first.
    write_file(&root, "main.snxsch", "line1\nline2_modified\nline4\nline5\n");
    adapter
        .commit_path(Path::new("main.snxsch"), "v2")
        .unwrap();

    let entries = adapter.file_history(Path::new("main.snxsch"), 10).unwrap();
    assert_eq!(entries.len(), 2, "expected two commits in history");
    let v2 = &entries[0]; // newest first
    assert_eq!(v2.files_changed, vec!["main.snxsch".to_string()]);
    // Diff stats are non-zero — exact counts depend on git2's diff
    // algorithm (one modification + two additions = 3 inserts, 1
    // deletion in the typical line-based diff). Assert non-empty.
    assert!(
        v2.additions > 0,
        "expected v2 additions > 0, got {}",
        v2.additions
    );
    assert!(
        v2.deletions > 0,
        "expected v2 deletions > 0, got {}",
        v2.deletions
    );

    let v1 = &entries[1];
    assert_eq!(v1.files_changed, vec!["main.snxsch".to_string()]);
    // Root commit — additions = newline count of initial blob (3).
    assert_eq!(v1.additions, 3);
    assert_eq!(v1.deletions, 0);
}

#[test]
fn file_history_caps_walker_iterations_for_dos_protection() {
    // v0.23 — the walker visits at most `10 * limit` commits before
    // giving up so a fork that rarely touches `rel_path` doesn't
    // hang the History panel. Empty result on a never-committed
    // path even when the graph has plenty of commits.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();

    // 30 commits to a different file. The walker should give up
    // before scanning all 30 when looking for `untouched.snxsch`.
    for i in 0..30 {
        write_file(&root, "other.snxsch", &format!("v{i}\n"));
        adapter
            .commit_path(Path::new("other.snxsch"), &format!("commit {i}"))
            .unwrap();
    }

    // Limit 1 → walker visits at most 10 commits. Path is never
    // touched so the result is empty without hanging.
    let entries = adapter
        .file_history(Path::new("untouched.snxsch"), 1)
        .unwrap();
    assert!(entries.is_empty());
}
