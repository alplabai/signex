//! Epic #2326 slice 2, PR 2.0, block A test 1: the dirty matrix rows whose
//! entry point has a catalog id, driven through the declared test adapter
//! so the rows survive the move of the edit tails behind the core seam. The
//! in-crate rows are `app/edit_tail_seam_tests/dirty_matrix.rs`.

use std::path::{Path, PathBuf};

use signex_app::test_adapter::{Origin, Outcome, TestAdapter};
use signex_types::project::ProjectData;
use signex_types::schematic::{Point, SchematicSheet, Wire};

/// A sheet with one wire from (0, 0) to (10.16, 0).
fn sheet_text() -> String {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        wires: vec![Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.16, 0.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode the sheet")
}

/// A project listing `names` as its pages, so an edit list may span them.
fn project_data(dir: &Path, names: &[&str]) -> ProjectData {
    let sheets = names
        .iter()
        .map(|name| signex_types::project::SheetEntry {
            name: (*name).to_string(),
            filename: (*name).to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        })
        .collect();
    ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: None,
        pcb_file: None,
        sheets,
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
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// A project holding the sheets `names`, each opened from the tree in order,
/// so the last one is the active sheet. Their paths, in order.
fn open_project(adapter: &mut TestAdapter, names: &[&str]) -> Vec<PathBuf> {
    let dir = adapter.root().join("P");
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<PathBuf> = names.iter().map(|name| dir.join(name)).collect();
    for path in &paths {
        std::fs::write(path, sheet_text()).unwrap();
    }
    let project = dir.join("P.snxprj");
    signex_types::project::write_project(&project, &project_data(&dir, names)).unwrap();
    let open = format!("path={}", project.display());
    let (outcome, pending) = adapter.execute("open_document", &[&open], Origin::Person);
    assert_eq!(outcome, Outcome::Ran, "the project opens");
    adapter.deliver(pending).unwrap();
    for path in &paths {
        let read = adapter
            .open_from_tree(path)
            .expect("the project tree opens the sheet");
        adapter.deliver(read).unwrap();
    }
    paths
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

/// Run `id` with `args` and every background step it starts.
fn run(adapter: &mut TestAdapter, id: &str, args: &[&str], origin: Origin) {
    let (outcome, pending) = adapter.execute(id, args, origin);
    assert_eq!(outcome, Outcome::Ran, "{id} {args:?} runs");
    adapter.deliver(pending).unwrap();
}

fn is_dirty(adapter: &mut TestAdapter, path: &Path) -> bool {
    let info = adapter
        .query("document_info", &[&document(path)])
        .expect("document_info answers");
    info["dirty"] == true
}

fn revision(adapter: &mut TestAdapter, path: &Path) -> u64 {
    adapter
        .query("read_document", &[&document(path)])
        .expect("read_document answers")["revision"]
        .as_u64()
        .expect("a revision")
}

/// An agent's `apply_edits` on `on`, with every sheet of `also` expected at
/// its revision. The answer (and its group) does not come back through the
/// adapter, so the `revert` row is in-crate.
fn apply_edits(adapter: &mut TestAdapter, on: &Path, also: &[&Path], edits: &str) {
    let mut args = vec![
        document(on),
        format!("expect_revision={}", revision(adapter, on)),
        format!("edits={edits}"),
    ];
    if !also.is_empty() {
        let others: serde_json::Map<String, serde_json::Value> = also
            .iter()
            .map(|path| (path.display().to_string(), revision(adapter, path).into()))
            .collect();
        args.push(format!(
            "expect_revisions={}",
            serde_json::Value::from(others)
        ));
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let (outcome, pending) = adapter.execute("apply_edits", &refs, Origin::Agent);
    assert_eq!(outcome, Outcome::Ran, "apply_edits runs");
    adapter.deliver(pending).unwrap();
}

const JUNCTION: &str = r#"[{"id":"place_junction","args":{"at":[5.08,0]}}]"#;

/// The checks of every row: dirty after the edit, clean after the undo,
/// dirty again after the redo, on each of `sheets`.
fn assert_round_trip(
    adapter: &mut TestAdapter,
    row: &str,
    sheets: &[&Path],
    undo: &dyn Fn(&mut TestAdapter),
    redo: &dyn Fn(&mut TestAdapter),
) {
    for sheet in sheets {
        assert!(is_dirty(adapter, sheet), "{row}: dirty after the edit");
    }
    undo(adapter);
    for sheet in sheets {
        assert!(!is_dirty(adapter, sheet), "{row}: clean after undo");
    }
    redo(adapter);
    for sheet in sheets {
        assert!(is_dirty(adapter, sheet), "{row}: dirty after redo");
    }
}

#[test]
fn a_placement_by_document_then_undo_and_redo_by_document_round_trip_the_dirty_mark() {
    let mut adapter = TestAdapter::new();
    let [a, _b] = open_project(&mut adapter, &["a.snxsch", "b.snxsch"])
        .try_into()
        .unwrap();
    assert!(!is_dirty(&mut adapter, &a));
    run(
        &mut adapter,
        "place_junction",
        &["at=5.08,0", &document(&a)],
        Origin::Person,
    );
    let by_id = |id: &'static str| {
        let a = a.clone();
        move |adapter: &mut TestAdapter| run(adapter, id, &[&document(&a)], Origin::Person)
    };
    assert_round_trip(
        &mut adapter,
        "placement by document",
        &[&a],
        &by_id("undo"),
        &by_id("redo"),
    );
}

#[test]
fn a_net_colour_by_document_then_undo_and_redo_by_document_round_trip_the_dirty_mark() {
    let mut adapter = TestAdapter::new();
    let [a, _b] = open_project(&mut adapter, &["a.snxsch", "b.snxsch"])
        .try_into()
        .unwrap();
    run(
        &mut adapter,
        "color_net",
        &["at=5.08,0", "color=#ff0000", &document(&a)],
        Origin::Person,
    );
    let by_id = |id: &'static str| {
        let a = a.clone();
        move |adapter: &mut TestAdapter| run(adapter, id, &[&document(&a)], Origin::Person)
    };
    assert_round_trip(
        &mut adapter,
        "execute_on_sheet (color_net)",
        &[&a],
        &by_id("undo"),
        &by_id("redo"),
    );
}

/// A person's Undo and Redo of the active sheet, as a key or the menu runs
/// them: no operands.
fn person_step(id: &'static str) -> impl Fn(&mut TestAdapter) {
    move |adapter: &mut TestAdapter| run(adapter, id, &[], Origin::Person)
}

#[test]
fn an_agents_edit_list_on_one_sheet_round_trips_the_dirty_mark_through_the_persons_undo() {
    let mut adapter = TestAdapter::new();
    let [a] = open_project(&mut adapter, &["a.snxsch"])
        .try_into()
        .unwrap();
    apply_edits(&mut adapter, &a, &[], JUNCTION);
    assert_round_trip(
        &mut adapter,
        "agent apply_edits, single sheet",
        &[&a],
        &person_step("undo"),
        &person_step("redo"),
    );
}

#[test]
fn an_agents_edit_list_across_two_sheets_round_trips_both_dirty_marks() {
    let mut adapter = TestAdapter::new();
    let [a, b] = open_project(&mut adapter, &["a.snxsch", "b.snxsch"])
        .try_into()
        .unwrap();
    let edits = format!(
        r#"[{{"id":"place_junction","args":{{"at":[5.08,0]}}}},{{"id":"place_junction","args":{{"at":[5.08,0],"document":"{}"}}}}]"#,
        a.display()
    );
    apply_edits(&mut adapter, &b, &[&a], &edits);
    assert_round_trip(
        &mut adapter,
        "agent apply_edits, cross-sheet",
        &[&a, &b],
        &person_step("undo"),
        &person_step("redo"),
    );
}

#[test]
fn a_single_agent_command_round_trips_the_dirty_mark_through_the_persons_undo() {
    let mut adapter = TestAdapter::new();
    let [a] = open_project(&mut adapter, &["a.snxsch"])
        .try_into()
        .unwrap();
    run(
        &mut adapter,
        "place_text",
        &["at=20.32,20.32", "text=edit-tail-seam", &document(&a)],
        Origin::Agent,
    );
    assert_round_trip(
        &mut adapter,
        "agent single command",
        &[&a],
        &person_step("undo"),
        &person_step("redo"),
    );
}
