//! #1828 -- a primitive save into a library whose folder was moved or deleted
//! outside the app is refused; it must not re-create the folder.

#![cfg(feature = "local-git")]

use signex_library::adapter::LibraryAdapter;
use signex_library::primitive::{Body3D, BodyShape, Footprint};

use super::local_git_rollback::init;

#[test]
fn a_footprint_save_does_not_recreate_a_library_folder_that_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _lib_id, adapter) = init(&dir);
    let root = file.parent().unwrap().to_path_buf();
    std::fs::rename(&root, dir.path().join("moved_away")).unwrap();

    let mut fp = Footprint::empty("F");
    fp.body_3d = Body3D {
        shape: BodyShape::Extrude,
        ..fp.body_3d
    };
    let result = adapter.save_footprint(fp, "add");

    assert!(result.is_err(), "the save must be refused");
    assert!(!root.exists(), "the library folder must not be re-created");
}

#[test]
fn the_refusal_text_names_the_library_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _lib_id, adapter) = init(&dir);
    let root = file.parent().unwrap().to_path_buf();
    std::fs::rename(&root, dir.path().join("moved_away")).unwrap();

    let error = adapter
        .save_footprint(Footprint::empty("F"), "add")
        .expect_err("the save must be refused");

    assert!(
        error.to_string().contains(&root.display().to_string()),
        "the refusal must name the library folder, got: {error}"
    );
}
