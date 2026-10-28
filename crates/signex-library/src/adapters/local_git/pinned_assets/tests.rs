//! #1471 — undoing a failed asset commit, including when the restore write
//! itself fails.

use super::*;
use crate::library_file::{FORMAT_TOKEN, LibrarySection};
use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

fn adapter(dir: &tempfile::TempDir) -> LocalGitAdapter {
    crate::git_identity::test_identity::install();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: uuid::Uuid::now_v7(),
        library: LibrarySection {
            name: "Undo".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(
        dir.path().join("Undo").join("Undo.snxlib"),
        manifest,
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init")
}

#[test]
fn a_failed_restore_write_keeps_disk_and_memory_on_the_saved_row() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = adapter(&dir);
    let on_disk = std::fs::read(adapter.file_path_buf()).unwrap();
    let mut guard = adapter.library_file.write().unwrap();
    let previous_file = guard.clone();
    let mut saved = guard.clone();
    saved.manifest.library.description = Some("the saved row stands in".into());
    *guard = saved.clone();

    let err = adapter.undo_asset_commit(
        &mut guard,
        on_disk.clone(),
        previous_file,
        LibraryError::Backend("git commit failed".into()),
        |_, _| Err(std::io::Error::other("disk full")),
    );

    let text = err.to_string();
    assert!(text.contains("could not be restored"), "{text}");
    assert!(
        text.contains("disk full") && text.contains("git commit failed"),
        "{text}"
    );
    assert_eq!(*guard, saved, "memory was not half undone");
    assert_eq!(
        std::fs::read(adapter.file_path_buf()).unwrap(),
        on_disk,
        "the failed write touched nothing"
    );
}

#[test]
fn a_successful_restore_puts_the_previous_file_back_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = adapter(&dir);
    let on_disk = std::fs::read(adapter.file_path_buf()).unwrap();
    let mut guard = adapter.library_file.write().unwrap();
    let previous_file = guard.clone();
    guard.manifest.library.description = Some("edit".into());

    let err = adapter.undo_asset_commit(
        &mut guard,
        on_disk,
        previous_file.clone(),
        LibraryError::Backend("git commit failed".into()),
        |path, bytes| std::fs::write(path, bytes),
    );

    assert!(err.to_string().contains("was not saved"), "{err}");
    assert_eq!(*guard, previous_file);
}

/// The bytes put back are read after the reload, so they can be another
/// writer's; memory goes back to the older copy, so the next edit must
/// reload them, not overwrite them with that copy (#416 review).
#[test]
fn an_outside_write_restored_by_the_undo_is_reloaded_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = adapter(&dir);
    let mut guard = adapter.library_file.write().unwrap();
    let previous_file = guard.clone();
    let mut outside = guard.clone();
    outside.manifest.library.description = Some("written by another process".into());
    let outside_bytes = outside.write().unwrap().into_bytes();

    let err = adapter.undo_asset_commit(
        &mut guard,
        outside_bytes,
        previous_file,
        LibraryError::Backend("git commit failed".into()),
        |path, bytes| std::fs::write(path, bytes),
    );
    assert!(err.to_string().contains("was not saved"), "{err}");
    drop(guard);

    adapter.create_empty_table("parts", "next edit").unwrap();
    let on_disk =
        LibraryFile::parse(&std::fs::read_to_string(adapter.file_path_buf()).unwrap()).unwrap();
    assert_eq!(
        on_disk.manifest.library.description.as_deref(),
        Some("written by another process")
    );
    assert!(on_disk.tables.contains_key("parts"));
}
