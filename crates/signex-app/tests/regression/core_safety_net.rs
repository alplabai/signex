//! The safety net of ADR-0006 (slice C, "Safety net before the high-risk
//! slices"): the persistence invariants that become state machines across
//! `complete`, pinned through the declared test adapter so a slice that
//! moves them keeps them. Ports of `document_save_async.rs` and
//! `document_save_ordering.rs`; the ledger-level tests stay in
//! `persist/document_save.rs`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use signex_app::test_adapter::{Origin, Outcome, TestAdapter};
use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use signex_types::project::ProjectData;
use signex_types::schematic::SchematicSheet;

pub(crate) fn sheet_text(paper: &str) -> String {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: paper.to_string(),
        ..Default::default()
    };
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode the sheet")
}

fn project_data(dir: &Path) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        fab_outputs: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// A project under the adapter's root holding the sheets `names`, open,
/// with every sheet opened from the tree. The sheets' paths, in order.
pub(crate) fn open_project(adapter: &mut TestAdapter, names: &[&str]) -> Vec<PathBuf> {
    let dir = adapter.root().join("P");
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<PathBuf> = names.iter().map(|name| dir.join(name)).collect();
    for path in &paths {
        std::fs::write(path, sheet_text("A4")).unwrap();
    }
    let project = dir.join("P.snxprj");
    signex_types::project::write_project(&project, &project_data(&dir)).unwrap();
    let open = format!("path={}", project.display());
    let (outcome, pending) = adapter.execute("open_document", &[&open], Origin::Person);
    assert_eq!(outcome, Outcome::Ran, "the project opens");
    adapter.deliver(pending).unwrap();
    for path in &paths {
        let read = adapter.open_from_tree(path).expect("the tree opens it");
        adapter.deliver(read).unwrap();
        assert!(is_open(adapter, path), "{} opens", path.display());
    }
    paths
}

pub(crate) fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

/// A text note `text` on the sheet at `path`, placed by an agent.
pub(crate) fn place_text(adapter: &mut TestAdapter, path: &Path, text: &str) {
    let text = format!("text={text}");
    let args = ["at=20.32,20.32", text.as_str(), &document(path)];
    let (outcome, pending) = adapter.execute("place_text", &args, Origin::Agent);
    assert_eq!(outcome, Outcome::Ran, "the edit runs");
    adapter.deliver(pending).unwrap();
}

fn save(adapter: &mut TestAdapter, path: &Path) -> signex_app::test_adapter::Pending {
    let (outcome, pending) = adapter.execute("save_document", &[&document(path)], Origin::Agent);
    assert_eq!(outcome, Outcome::Ran, "the save starts");
    pending
}

fn info(adapter: &mut TestAdapter, path: &Path) -> serde_json::Value {
    adapter
        .query("document_info", &[&document(path)])
        .expect("document_info answers")
}

pub(crate) fn is_dirty(adapter: &mut TestAdapter, path: &Path) -> bool {
    info(adapter, path)["dirty"] == true
}

pub(crate) fn is_open(adapter: &mut TestAdapter, path: &Path) -> bool {
    let listed = adapter
        .query("list_documents", &[])
        .expect("list_documents");
    let wanted = path.display().to_string();
    listed["documents"]
        .as_array()
        .is_some_and(|docs| docs.iter().any(|doc| doc["path"] == wanted.as_str()))
}

/// #997: an older write never lands after a newer one to the same path,
/// whatever order their background work runs in.
#[test]
fn an_older_save_delivered_last_never_overwrites_a_newer_one() {
    let mut adapter = TestAdapter::new();
    let [path] = open_project(&mut adapter, &["a.snxsch"])
        .try_into()
        .unwrap();
    place_text(&mut adapter, &path, "first-c2f1");
    let older = save(&mut adapter, &path);
    place_text(&mut adapter, &path, "second-9b3e");
    let newer = save(&mut adapter, &path);

    adapter.deliver(newer).unwrap();
    adapter.deliver(older).unwrap();

    let writes: Vec<_> = adapter
        .disk()
        .writes()
        .into_iter()
        .filter(|write| write.path == path)
        .collect();
    assert_eq!(
        writes.len(),
        1,
        "the older snapshot is skipped, not written"
    );
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(
        on_disk.contains("second-9b3e"),
        "the newest snapshot is on disk"
    );
    assert!(
        !is_dirty(&mut adapter, &path),
        "the newest save cleared dirty"
    );
}

/// The `SaveBarrier`: a History restore rewrites its file only after every
/// document write in flight, of any path, has finished.
#[test]
fn a_restore_waits_for_every_write_in_flight() {
    signex_library::git_identity::test_identity::install();
    let mut adapter = TestAdapter::with_deliver_limit(Duration::from_secs(2));
    let [a, b] = open_project(&mut adapter, &["a.snxsch", "b.snxsch"])
        .try_into()
        .unwrap();
    let restored = sheet_text("A2");
    std::fs::write(&a, &restored).unwrap();
    let repo = LocalGitProjectAdapter::open_or_init(a.parent().unwrap().to_path_buf()).unwrap();
    let sha = repo
        .commit_path(Path::new("a.snxsch"), "the version to restore")
        .unwrap()
        .expect("a commit is made")
        .to_string();
    let before = sheet_text("A3");
    std::fs::write(&a, &before).unwrap();

    place_text(&mut adapter, &b, "in-flight-4d21");
    let write_in_flight = save(&mut adapter, &b);
    let restore = adapter.restore(&a, &sha).expect("the restore starts");

    assert!(
        adapter.deliver(restore).is_err(),
        "the restore finished while a document write was still in flight"
    );
    assert_eq!(
        std::fs::read_to_string(&a).unwrap(),
        before,
        "the restore rewrote its file before the write in flight landed"
    );
    adapter.deliver(write_in_flight).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::fs::read_to_string(&a).unwrap() != restored {
        assert!(
            Instant::now() < deadline,
            "the restore never landed after the write"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The `complete` generation check, save half: a write's completion for a
/// document that moved on while it ran (edited, or its engine replaced:
/// both change the engine's revision) does not mark it clean.
#[test]
fn a_save_completion_for_a_document_that_moved_on_keeps_it_dirty() {
    let mut adapter = TestAdapter::new();
    let [path] = open_project(&mut adapter, &["a.snxsch"])
        .try_into()
        .unwrap();
    place_text(&mut adapter, &path, "saved-17aa");
    let saving = save(&mut adapter, &path);
    place_text(&mut adapter, &path, "after-5e0c");

    adapter.deliver(saving).unwrap();

    // Positive control: the save did land, so dirty is the completion's call.
    assert_eq!(
        writes_to(&adapter, &path),
        1,
        "the save wrote the file once"
    );
    assert!(
        is_dirty(&mut adapter, &path),
        "the snapshot predates the edit, so the edit is still unsaved"
    );
}

/// The `complete` generation check, read half (`finish_claim`): a tree read
/// whose request was cancelled while it ran (every document closed) is
/// dropped, not installed.
#[test]
fn a_tree_read_finishing_after_close_all_is_dropped() {
    let mut adapter = TestAdapter::new();
    let [b] = open_project(&mut adapter, &["b.snxsch"])
        .try_into()
        .unwrap();
    let a = b.with_file_name("a.snxsch");
    std::fs::write(&a, sheet_text("A4")).unwrap();

    let read = adapter.open_from_tree(&a).expect("the tree opens it");
    close_all(&mut adapter);
    adapter.deliver(read).unwrap();

    assert!(
        !is_open(&mut adapter, &a),
        "a read cancelled while it ran opened its document anyway"
    );
    assert!(!is_open(&mut adapter, &b));
}

/// The read half for a History restore (`finish_claim`): a restore cancelled
/// while it ran (every document closed) is not taken for the reopened
/// sheet's own. Its file is rewritten anyway, so a reopened sheet that
/// holds no edits shows the restored file; one with edits keeps them.
#[test]
fn a_restore_cancelled_by_close_all_shows_its_file_in_a_clean_reopened_sheet() {
    let (mut adapter, a, restore) = restore_then_close_all();
    let read = adapter.open_from_tree(&a).expect("the tree opens it");
    adapter.deliver(read).unwrap();
    assert_eq!(
        paper(&mut adapter, &a),
        "A4",
        "reopened before the restore ran"
    );

    adapter.deliver(restore).unwrap();

    assert_eq!(
        paper(&mut adapter, &a),
        "A2",
        "the clean reopened sheet shows the file the restore wrote"
    );
}

#[test]
fn a_restore_cancelled_by_close_all_keeps_the_edits_of_a_reopened_sheet() {
    let (mut adapter, a, restore) = restore_then_close_all();
    let read = adapter.open_from_tree(&a).expect("the tree opens it");
    adapter.deliver(read).unwrap();
    place_text(&mut adapter, &a, "reopened-edit-6c0d");

    adapter.deliver(restore).unwrap();

    // Positive control: the restore did rewrite the file.
    assert!(
        std::fs::read_to_string(&a).unwrap().contains("A2"),
        "the restore wrote the A2 version"
    );
    assert_eq!(paper(&mut adapter, &a), "A4");
    assert!(
        read_document(&mut adapter, &a).contains("reopened-edit-6c0d"),
        "a restore cancelled while it ran replaced an edited reopened sheet"
    );
    assert!(is_dirty(&mut adapter, &a));
}

/// How many document writes to `path` landed.
fn writes_to(adapter: &TestAdapter, path: &Path) -> usize {
    adapter
        .disk()
        .writes()
        .iter()
        .filter(|write| write.path == path)
        .count()
}

/// A project with the sheet `a.snxsch` (paper A4) whose history holds an A2
/// version; a restore of it started, then every document closed.
fn restore_then_close_all() -> (TestAdapter, PathBuf, signex_app::test_adapter::Pending) {
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
    let restore = adapter.restore(&a, &sha).expect("the restore starts");
    close_all(&mut adapter);
    (adapter, a, restore)
}

pub(crate) fn read_document(adapter: &mut TestAdapter, path: &Path) -> String {
    adapter
        .query("read_document", &[&document(path)])
        .expect("read_document answers")
        .to_string()
}

pub(crate) fn paper(adapter: &mut TestAdapter, path: &Path) -> String {
    let answer = adapter
        .query("read_document", &[&document(path)])
        .expect("read_document answers");
    answer["header"]["paper_size"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Close every document as a person does.
fn close_all(adapter: &mut TestAdapter) {
    let (outcome, closed) = adapter.execute("close_all_documents", &[], Origin::Person);
    assert_eq!(outcome, Outcome::Ran, "close all runs");
    adapter.deliver(closed).unwrap();
}

/// The `drag` gesture: a wire dragged one way moves, dirties its sheet and
/// takes one undo step.
#[test]
fn a_dragged_wire_moves_as_one_undo_step() {
    let mut adapter = TestAdapter::new();
    let [b] = open_project(&mut adapter, &["b.snxsch"])
        .try_into()
        .unwrap();
    let path = b.with_file_name("w.snxsch");
    std::fs::write(&path, wire_sheet_text()).unwrap();
    let read = adapter.open_from_tree(&path).expect("the tree opens it");
    adapter.deliver(read).unwrap();
    let before = wires(&mut adapter, &path);

    adapter
        .drag(&path, (20.32, 10.16), (20.32, 20.32), 4)
        .expect("the wire is dragged");

    let after = wires(&mut adapter, &path);
    assert_ne!(after, before, "the wire moved");
    assert!(after.contains("20.32"), "it landed 10.16 mm lower: {after}");
    assert!(is_dirty(&mut adapter, &path));
    let (outcome, undone) = adapter.execute("undo", &[], Origin::Person);
    assert_eq!(outcome, Outcome::Ran);
    adapter.deliver(undone).unwrap();
    assert_eq!(
        wires(&mut adapter, &path),
        before,
        "one undo step undoes it"
    );
    assert_eq!(info(&mut adapter, &path)["history"]["can_undo"], false);
}

fn wire_sheet_text() -> String {
    use signex_types::schematic::{Point, Wire};
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".into(),
        wires: vec![Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(10.16, 10.16),
            end: Point::new(30.48, 10.16),
            stroke_width: 0.0,
            color: None,
            locked: false,
            unknown_columns: Default::default(),
        }],
        ..Default::default()
    };
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode the sheet")
}

fn wires(adapter: &mut TestAdapter, path: &Path) -> String {
    let answer = adapter
        .query("read_document", &[&document(path)])
        .expect("read_document answers");
    answer["elements"]["wires"].to_string()
}
