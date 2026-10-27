//! Two Signex processes saving primitives into the same library (#893).
//!
//! A primitive container is read, merged and rewritten on every save, so
//! two processes editing different symbols of one container could drop
//! each other's edit; and two processes committing to one repository race
//! on `.git/index.lock`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::primitive::{Symbol, SymbolFile};
use uuid::Uuid;

fn versioned_library(dir: &Path) -> PathBuf {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Lib").join("Lib.snxlib");
    let manifest = SnxlibManifest {
        extra: Default::default(),
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "Lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    drop(
        LocalGitAdapter::init(
            &file,
            manifest,
            LibraryInitOptions {
                enable_git: true,
                use_lfs: false,
            },
        )
        .expect("init succeeds"),
    );
    file
}

/// Another process holding `.git/index.lock` for a moment (its own commit)
/// must not fail this save: the commit waits for the lock instead.
#[test]
fn a_save_waits_for_another_process_holding_the_git_index_lock() {
    let dir = tempfile::tempdir().unwrap();
    let file = versioned_library(dir.path());
    let index_lock = file.parent().unwrap().join(".git").join("index.lock");
    std::fs::write(&index_lock, b"").unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        std::fs::remove_file(index_lock).unwrap();
    });

    let adapter = LocalGitAdapter::open(&file).unwrap();
    let saved = adapter.save_symbol(Symbol::empty("R"), "add R");
    release.join().unwrap();

    saved.expect("the save waits for the index lock instead of failing");
}

/// Two processes editing different symbols of one container both keep
/// their edit: neither rewrite drops the other's symbol change.
#[test]
fn concurrent_edits_to_one_symbol_container_lose_no_change() {
    let dir = tempfile::tempdir().unwrap();
    let file = versioned_library(dir.path());
    let a = Symbol::empty("A");
    let b = Symbol::empty("B");
    let mut container = SymbolFile::from_symbol(a.clone());
    container.symbols.push(b.clone());
    let symbols_dir = file.parent().unwrap().join("symbols");
    std::fs::create_dir_all(&symbols_dir).unwrap();
    std::fs::write(
        symbols_dir.join("pair.snxsym"),
        container.to_toml_string().unwrap(),
    )
    .unwrap();

    let writers: Vec<_> = [a, b]
        .into_iter()
        .map(|symbol| {
            let file = file.clone();
            std::thread::spawn(move || {
                let adapter = LocalGitAdapter::open(&file).unwrap();
                for i in 0..15 {
                    let mut edit = symbol.clone();
                    edit.name = format!("{}-{i}", symbol.name);
                    adapter.save_symbol(edit, "edit").unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }

    let text = std::fs::read_to_string(symbols_dir.join("pair.snxsym")).unwrap();
    assert!(text.contains("A-14"), "A's last edit was lost:\n{text}");
    assert!(text.contains("B-14"), "B's last edit was lost:\n{text}");
}

/// #900 review: two processes committing different files of one repository
/// never revert each other. Each commit is checked along first-parent
/// history: a file's version may only grow. A commit built from a shared
/// on-disk index that the other process rewrote in between would write back
/// that file's older version.
#[test]
fn concurrent_commits_to_one_repository_never_revert_each_other() {
    use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
    signex_library::git_identity::test_identity::install();
    const COMMITS: u32 = 100;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    for name in ["a.txt", "b.txt"] {
        std::fs::write(root.join(name), "0").unwrap();
    }
    let seed = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    for name in ["a.txt", "b.txt"] {
        seed.commit_path(Path::new(name), "seed").unwrap();
    }
    drop(seed);

    let writers: Vec<_> = ["a.txt", "b.txt"]
        .into_iter()
        .map(|name| {
            let root = root.clone();
            std::thread::spawn(move || {
                let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
                for i in 1..=COMMITS {
                    std::fs::write(root.join(name), i.to_string()).unwrap();
                    adapter
                        .commit_path(Path::new(name), &format!("{name} {i}"))
                        .unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }

    let repo = git2::Repository::open(&root).unwrap();
    // A file not committed yet (the first seed commit) counts as version 0.
    let version = |tree: &git2::Tree<'_>, name: &str| -> u32 {
        tree.get_name(name).map_or(0, |entry| {
            let blob = repo.find_blob(entry.id()).unwrap();
            std::str::from_utf8(blob.content())
                .unwrap()
                .parse()
                .unwrap()
        })
    };
    let mut history = Vec::new();
    let mut commit = repo.head().unwrap().peel_to_commit().unwrap();
    loop {
        let tree = commit.tree().unwrap();
        history.push((
            version(&tree, "a.txt"),
            version(&tree, "b.txt"),
            commit.id(),
        ));
        match commit.parent(0) {
            Ok(parent) => commit = parent,
            Err(_) => break,
        }
    }
    history.reverse();
    for pair in history.windows(2) {
        let ((a0, b0, _), (a1, b1, id)) = (pair[0], pair[1]);
        assert!(
            a1 >= a0 && b1 >= b0,
            "commit {id} reverted a file: ({a0}, {b0}) -> ({a1}, {b1})"
        );
    }
    let last = history.last().unwrap();
    assert_eq!((last.0, last.1), (COMMITS, COMMITS));
}

/// #900 review: an `index.lock` a crashed git left behind never goes away by
/// itself. The commit does not need the index (its tree is built from the
/// parent commit), so the save goes through at once instead of freezing for
/// the whole lock wait; only the best-effort index refresh is skipped, with
/// a warning. Signex never removes the lock.
#[test]
fn a_stale_git_index_lock_does_not_block_or_fail_the_save() {
    let dir = tempfile::tempdir().unwrap();
    let file = versioned_library(dir.path());
    let root = file.parent().unwrap().to_path_buf();
    let lock = root.join(".git").join("index.lock");
    let stale = std::fs::File::create(&lock).unwrap();
    stale
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(60))
        .unwrap();
    drop(stale);

    let adapter = LocalGitAdapter::open(&file).unwrap();
    let started = std::time::Instant::now();
    adapter
        .save_symbol(Symbol::empty("R"), "add R")
        .expect("the commit does not need the index lock");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(lock.exists(), "the lock is never removed by Signex");
    let repo = git2::Repository::open(&root).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert!(String::from_utf8_lossy(head.message_bytes()).starts_with("add R"));
}

/// Hold the write lock of the library at `file` from this thread, as another
/// Signex process would while it writes.
fn hold_write_lock(file: &Path) -> std::fs::File {
    let lock = signex_library::write_lock_path(file);
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    let handle = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock)
        .unwrap();
    handle.lock().unwrap();
    handle
}

/// #902: the app editor's save reads, merges and writes its container under
/// the library write lock. While another process holds it, the read waits,
/// so it sees what that process wrote.
#[test]
fn rewriting_a_library_file_reads_it_only_after_another_process_releases_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let file = versioned_library(dir.path());
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let target = file.parent().unwrap().join("symbols").join("pair.snxsym");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"before").unwrap();

    let held = hold_write_lock(&file);
    let other = target.clone();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(&other, b"written by the other process").unwrap();
        drop(held);
    });
    let mut seen = Vec::new();
    let written = adapter
        .rewrite_file_locked(&target, "merge pair", &mut |current| {
            seen = current.unwrap_or_default().to_vec();
            Ok(Some(b"merged".to_vec()))
        })
        .unwrap();
    release.join().unwrap();

    assert!(written);
    assert_eq!(seen, b"written by the other process");
    assert_eq!(std::fs::read(&target).unwrap(), b"merged");
    // #904 review: committed before the lock was released, so another
    // process cannot replace the file while git reads it.
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.summary().unwrap(), Some("merge pair"));
    let blob = head
        .tree()
        .unwrap()
        .get_path(Path::new("symbols/pair.snxsym"))
        .unwrap()
        .to_object(&repo)
        .unwrap()
        .peel_to_blob()
        .unwrap();
    assert_eq!(blob.content(), b"merged");
}

#[test]
fn rewriting_a_file_outside_the_library_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = versioned_library(dir.path());
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let outside = dir.path().join("elsewhere.snxsym");

    let error = adapter
        .rewrite_file_locked(&outside, "x", &mut |_| Ok(Some(b"x".to_vec())))
        .unwrap_err();

    assert!(error.to_string().contains("is not under"), "got {error}");
    assert!(!outside.exists());
}
