//! #1435 — keys a `.snxlib` holds that this build does not model survive the
//! adapter's class and table edits: a rename keeps them on the renamed class
//! or table, and a class removed by one save does not bring them back when
//! re-added.

#![cfg(feature = "local-git")]

use std::path::Path;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{ClassEntry, FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use uuid::Uuid;

fn class(key: &str) -> ClassEntry {
    ClassEntry {
        key: key.into(),
        label: key.to_uppercase(),
    }
}

/// A library whose `res` class carries an unknown key, as another build
/// would have written it.
fn library_with_unknown_class_key(dir: &Path) -> (std::path::PathBuf, LocalGitAdapter) {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Unk").join("Unk.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "Unk".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: vec![class("res"), class("cap")],
    };
    let adapter = LocalGitAdapter::init(&file, manifest, LibraryInitOptions::default())
        .expect("init succeeds");
    let text = std::fs::read_to_string(&file).unwrap();
    let edited = text.replacen(
        "key = \"res\"\n",
        "key = \"res\"\nfuture_class_key = \"x\"\n",
        1,
    );
    assert_ne!(edited, text);
    std::fs::write(&file, edited).unwrap();
    (file, adapter)
}

#[test]
fn renaming_a_class_keeps_its_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let (file, adapter) = library_with_unknown_class_key(dir.path());
    adapter
        .rename_library_class("res", class("resistor"), "rename")
        .unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("key = \"resistor\"\nfuture_class_key = \"x\""),
        "{text}"
    );
}

#[test]
fn a_removed_class_re_added_does_not_get_its_old_unknown_keys_back() {
    let dir = tempfile::tempdir().unwrap();
    let (file, adapter) = library_with_unknown_class_key(dir.path());
    adapter
        .update_library_classes(vec![class("cap")], "remove res")
        .unwrap();
    adapter
        .update_library_classes(vec![class("cap"), class("res")], "re-add res")
        .unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("key = \"res\""), "{text}");
    assert!(!text.contains("future_class_key"), "{text}");
}

#[test]
fn renaming_a_table_keeps_its_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let (file, adapter) = library_with_unknown_class_key(dir.path());
    adapter.create_empty_table("res", "add table").unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    let edited = text.replacen(
        "[tables.res]
",
        "[tables.res]
future_table_key = 7
",
        1,
    );
    assert_ne!(edited, text);
    std::fs::write(&file, edited).unwrap();
    adapter.rename_table("res", "resistors", "rename").unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains(
            "[tables.resistors]
future_table_key = 7
"
        ),
        "{text}"
    );
}
