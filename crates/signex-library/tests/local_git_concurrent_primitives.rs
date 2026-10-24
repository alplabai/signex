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
                for i in 0..30 {
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
    assert!(text.contains("A-29"), "A's last edit was lost:\n{text}");
    assert!(text.contains("B-29"), "B's last edit was lost:\n{text}");
}
