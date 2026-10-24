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

/// #900 review: while this save waits on `.git/index.lock`, another process
/// commits a change to a different file and writes its new index. The
/// retried commit must build on that commit, not write back the index as it
/// was before it (which would silently revert the other change in HEAD).
#[test]
fn a_retried_commit_keeps_another_process_commit_made_meanwhile() {
    let dir = tempfile::tempdir().unwrap();
    let file = versioned_library(dir.path());
    let root = file.parent().unwrap().to_path_buf();
    let adapter = LocalGitAdapter::open(&file).unwrap();
    adapter.save_symbol(Symbol::empty("A"), "add A").unwrap();
    adapter.save_symbol(Symbol::empty("B"), "add B").unwrap();
    drop(adapter);
    let symbol_file = |name: &str| -> String {
        std::fs::read_dir(root.join("symbols"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .find(|f| {
                std::fs::read_to_string(root.join("symbols").join(f))
                    .unwrap()
                    .contains(&format!("\"{name}\""))
            })
            .expect("symbol file")
    };
    let a_file = symbol_file("A");

    // Another process starts its commit: it holds the index lock.
    let git_dir = root.join(".git");
    let lock = git_dir.join("index.lock");
    std::fs::copy(git_dir.join("index"), &lock).unwrap();

    let saver = {
        let file = file.clone();
        std::thread::spawn(move || {
            let adapter = LocalGitAdapter::open(&file).unwrap();
            let mut b = Symbol::empty("B");
            b.name = "B-edited".into();
            adapter.save_symbol(b, "edit B")
        })
    };
    std::thread::sleep(Duration::from_millis(300));

    // ... commits a change to A, writes its new index, and releases.
    let repo = git2::Repository::open(&root).unwrap();
    let a_text = std::fs::read_to_string(root.join("symbols").join(&a_file))
        .unwrap()
        .replace("\"A\"", "\"A-by-other\"");
    std::fs::write(root.join("symbols").join(&a_file), &a_text).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let blob = repo.blob(a_text.as_bytes()).unwrap();
    let symbols = head.tree().unwrap().get_name("symbols").unwrap().id();
    let mut sub = repo
        .treebuilder(Some(&repo.find_tree(symbols).unwrap()))
        .unwrap();
    sub.insert(&a_file, blob, 0o100644).unwrap();
    let sub_id = sub.write().unwrap();
    let mut top = repo.treebuilder(Some(&head.tree().unwrap())).unwrap();
    top.insert("symbols", sub_id, 0o040000).unwrap();
    let tree = repo.find_tree(top.write().unwrap()).unwrap();
    let sig = git2::Signature::now("Other", "other@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "other edits A", &tree, &[&head])
        .unwrap();
    let mut other_index = git2::Index::open(&lock).unwrap();
    other_index.read_tree(&tree).unwrap();
    other_index.write().unwrap();
    std::fs::rename(&lock, git_dir.join("index")).unwrap();

    saver.join().unwrap().expect("the save goes through");

    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let a_in_head = head
        .tree()
        .unwrap()
        .get_path(&Path::new("symbols").join(&a_file))
        .unwrap()
        .to_object(&repo)
        .unwrap()
        .peel_to_blob()
        .unwrap();
    assert!(
        String::from_utf8_lossy(a_in_head.content()).contains("A-by-other"),
        "the other process's change to A was reverted in HEAD"
    );
}

/// #900 review: an `index.lock` a crashed git left behind never goes away by
/// itself. The save is refused at once with a hint, instead of freezing for
/// the whole lock wait on every save.
#[test]
fn a_stale_git_index_lock_is_refused_at_once_with_a_hint() {
    let dir = tempfile::tempdir().unwrap();
    let file = versioned_library(dir.path());
    let lock = file.parent().unwrap().join(".git").join("index.lock");
    let stale = std::fs::File::create(&lock).unwrap();
    stale
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(60))
        .unwrap();
    drop(stale);

    let adapter = LocalGitAdapter::open(&file).unwrap();
    let started = std::time::Instant::now();
    let err = adapter
        .save_symbol(Symbol::empty("R"), "add R")
        .expect_err("refused while the stale lock is there");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(
        err.to_string()
            .contains("if no git process is running, remove it"),
        "{err}"
    );
    assert!(lock.exists(), "the lock is never removed by Signex");
}
