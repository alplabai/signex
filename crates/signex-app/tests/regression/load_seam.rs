//! #2326 slice 3, PR 3.0: the load seam (open, History restore, Save As)
//! pinned through the declared test adapter before slice 3 moves it. The
//! in-crate half, for what the frozen API cannot reach, is
//! `src/app/load_seam_tests.rs`.

use std::path::Path;

use signex_app::test_adapter::{Origin, Outcome, TestAdapter};
use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use signex_types::schematic::{Point, SchematicSheet, Wire};

use super::core_safety_net::{
    document, is_dirty, is_open, open_project, paper, place_text, read_document, sheet_text,
};

/// A sheet whose two wires repeat one uuid: loading it repairs the copy.
fn repaired_sheet_text() -> String {
    let wire = |end_x: f64| Wire {
        uuid: uuid::Uuid::from_u128(7),
        end: Point::new(end_x, 0.0),
        ..Default::default()
    };
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".into(),
        wires: vec![wire(1.0), wire(2.0)],
        ..Default::default()
    };
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode the sheet")
}

fn undo(adapter: &mut TestAdapter) {
    let (outcome, undone) = adapter.execute("undo", &[], Origin::Person);
    assert_eq!(outcome, Outcome::Ran, "undo runs");
    adapter.deliver(undone).unwrap();
}

/// T1: the open marks the engine's position as the saved state, so an edit
/// undone back to it leaves the sheet clean.
#[test]
fn an_opened_sheet_is_clean_and_an_undo_back_to_the_open_state_is_clean() {
    let mut adapter = TestAdapter::new();
    let [path] = open_project(&mut adapter, &["a.snxsch"])
        .try_into()
        .unwrap();
    assert!(!is_dirty(&mut adapter, &path), "opened clean");

    place_text(&mut adapter, &path, "undo-me-3a71");
    assert!(is_dirty(&mut adapter, &path), "the edit dirties it");
    undo(&mut adapter);

    assert!(
        !read_document(&mut adapter, &path).contains("undo-me-3a71"),
        "the undo took the edit back"
    );
    assert!(
        !is_dirty(&mut adapter, &path),
        "back at the opened state, which is the file"
    );
}

/// T2: a sheet whose load repaired it differs from its file, so it opens
/// modified.
#[test]
fn a_sheet_repaired_on_open_opens_modified() {
    let mut adapter = TestAdapter::new();
    let [b] = open_project(&mut adapter, &["b.snxsch"])
        .try_into()
        .unwrap();
    let path = b.with_file_name("repaired.snxsch");
    std::fs::write(&path, repaired_sheet_text()).unwrap();

    let read = adapter.open_from_tree(&path).expect("the tree opens it");
    adapter.deliver(read).unwrap();

    assert!(is_open(&mut adapter, &path));
    assert!(is_dirty(&mut adapter, &path), "the file lacks the repair");
}

/// T5: a History restore of `a` whose read lands after the person switched
/// to `b` installs into `a`'s tab, never into the tab in view.
#[test]
fn a_restore_delivered_after_a_tab_switch_installs_the_sheet_behind_its_tab() {
    signex_library::git_identity::test_identity::install();
    let mut adapter = TestAdapter::new();
    let [a] = open_project(&mut adapter, &["a.snxsch"])
        .try_into()
        .unwrap();
    let opened = std::fs::read(&a).unwrap();
    std::fs::write(&a, sheet_text("A2")).unwrap();
    let repo = LocalGitProjectAdapter::open_or_init(a.parent().unwrap().to_path_buf()).unwrap();
    let sha = repo
        .commit_path(Path::new("a.snxsch"), "the version to restore")
        .unwrap()
        .expect("a commit is made")
        .to_string();
    std::fs::write(&a, opened).unwrap();
    let b = a.with_file_name("b.snxsch");
    std::fs::write(&b, sheet_text("A3")).unwrap();

    let restore = adapter.restore(&a, &sha).expect("the restore starts");
    let read = adapter.open_from_tree(&b).expect("the tree opens b");
    adapter.deliver(read).unwrap();
    adapter.deliver(restore).unwrap();

    assert_eq!(
        paper(&mut adapter, &a),
        "A2",
        "a holds the restored version"
    );
    assert_eq!(paper(&mut adapter, &b), "A3", "b, in view, is untouched");
}

/// T9: Save As moves the engine and the tab to the new file; the saved copy
/// is clean there and the old path names no open document. The old path's
/// dirty mark is pinned by the in-crate half (`src/app/load_seam_tests.rs`).
#[test]
fn save_as_rekeys_the_engine_and_the_tab() {
    let mut adapter = TestAdapter::new();
    let [from] = open_project(&mut adapter, &["a.snxsch"])
        .try_into()
        .unwrap();
    place_text(&mut adapter, &from, "carried-8d42");
    assert!(is_dirty(&mut adapter, &from));
    let to = from.with_file_name("copy.snxsch");

    let target = format!("path={}", to.display());
    let (outcome, saving) = adapter.execute(
        "save_document_as",
        &[&document(&from), &target],
        Origin::Person,
    );
    assert_eq!(outcome, Outcome::Ran, "the Save As starts");
    adapter.deliver(saving).unwrap();

    assert!(is_open(&mut adapter, &to), "the tab follows the new file");
    assert!(
        !is_open(&mut adapter, &from),
        "nothing is left at the old path"
    );
    assert!(
        read_document(&mut adapter, &to).contains("carried-8d42"),
        "the engine moved with its edits"
    );
    assert!(!is_dirty(&mut adapter, &to), "the new file holds the sheet");
}
