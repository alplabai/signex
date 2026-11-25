//! #2908: the writes of a copy. A project library kept as a `*.snxlib/`
//! folder is copied whole, and a failure part way takes back every file the
//! copy wrote and every folder it made, the folders `atomic_write` created
//! for them included.

use std::fs;
use std::path::{Path, PathBuf};

use signex_types::project::{ProjectData, parse_project};

use super::plan::plan_copy;
use super::tests::{newer_project, put, snapshot};
use super::write::{CopyOutcome, write_copy};

/// `newer_project` read as a project this build writes, so no keys check
/// stands before the writes these tests are about.
fn current_project(root: &Path) -> (PathBuf, ProjectData) {
    let (dir, _) = newer_project(root);
    let mut data = parse_project(&dir.join("P.snxprj")).expect("parse");
    data.unknown.newer_minor_source = None;
    (dir, data)
}

#[test]
fn a_library_kept_as_a_folder_is_copied_whole() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, mut data) = current_project(tmp.path());
    let folder_lib = [
        "libs/F.snxlib/library.toml",
        "libs/F.snxlib/symbols/a.snxsym",
        "libs/F.snxlib/deep/more/b.bin",
    ];
    for rel in folder_lib {
        put(&dir.join(rel), rel.as_bytes());
    }
    let mut entry = data.libraries[0].clone();
    entry.path = PathBuf::from("libs/F.snxlib");
    data.libraries.push(entry);
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("out");

    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &out, Vec::new()).expect("plan");
    let CopyOutcome::Written(_) = write_copy(&plan).expect("the copy") else {
        panic!("a current project is not checked");
    };
    for rel in folder_lib {
        assert_eq!(
            fs::read(out.join("Copy").join(rel)).expect(rel),
            rel.as_bytes(),
            "{rel}"
        );
    }
}

/// The project with a sheet under a folder named as the copy's `.snxprj`:
/// the sheet's write makes that folder (and one below it), so the `.snxprj`
/// written last fails on it.
fn failing_last_write(root: &Path) -> (PathBuf, ProjectData) {
    let (dir, mut data) = current_project(root);
    let rel = "Copy.snxprj/deep/x.snxsch";
    put(&dir.join(rel), b"sheet");
    let mut sheet = data.sheets[0].clone();
    sheet.filename = rel.to_string();
    data.sheets.push(sheet);
    (dir, data)
}

#[test]
fn a_failure_removes_every_file_and_folder_the_copy_made() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, data) = failing_last_write(tmp.path());
    let before = snapshot(&dir);
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("out");

    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &out, Vec::new()).expect("plan");
    let why = write_copy(&plan).expect_err("the project file cannot be written");
    assert!(why.contains("Copy.snxprj"), "{why}");
    assert_eq!(
        fs::read_dir(&out).expect("out").count(),
        0,
        "the copy's folder and everything in it are gone"
    );
    assert_eq!(snapshot(&dir), before, "the original folder is untouched");
}

#[test]
fn a_failure_keeps_a_folder_that_was_there_before_and_empties_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, data) = failing_last_write(tmp.path());
    let out = tmp.path().join("out");
    let target = out.join("Copy");
    fs::create_dir_all(&target).expect("an empty target folder");

    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &out, Vec::new()).expect("plan");
    write_copy(&plan).expect_err("the project file cannot be written");
    assert!(target.is_dir(), "a folder the copy did not make stays");
    assert_eq!(fs::read_dir(&target).expect("target").count(), 0);
}
