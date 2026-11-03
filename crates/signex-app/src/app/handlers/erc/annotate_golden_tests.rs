//! The designators a project-wide Annotate / Reset Duplicate Designators hands
//! out, pinned as literals on a project with open and unopened sheets (#859).
//!
//! The literals were recorded from the synchronous implementation before the
//! pass moved off the update loop; any change to them is a change in what the
//! user's project ends up numbered as.

use std::path::{Path, PathBuf};

use signex_types::format::SnxSchematic;
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::app::path_ops::settle_task;

fn symbol(reference: &str, unit: u32, y: f64) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        unit,
        position: Point::new(0.0, y),
        ..Default::default()
    }
}

fn sheet(children: &[&str], refs: &[(&str, u32)]) -> SchematicSheet {
    let mut sheet = sheet_with_net("R?", "NET", children);
    sheet.symbols = refs
        .iter()
        .enumerate()
        .map(|(i, (reference, unit))| symbol(reference, *unit, i as f64 * 10.0))
        .collect();
    sheet
}

fn write(dir: &Path, name: &str, sheet: SchematicSheet) {
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.join(name), text).expect("write sheet");
}

/// Declared pages `b_root` (open, active) and `a_page` (unopened); `c_child`
/// (unopened) and `d_cached` (open, not active) are reached through `b_root`.
fn project() -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-annotate-golden-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    write(
        &dir,
        "a_page.snxsch",
        sheet(&[], &[("R?", 1), ("R3", 1), ("C?", 1), ("R3", 1)]),
    );
    write(
        &dir,
        "c_child.snxsch",
        sheet(&[], &[("R?", 1), ("C?", 1), ("R3", 1), ("U1", 1)]),
    );
    let mut app = app_workspace(&dir.to_string_lossy(), &["b_root.snxsch", "a_page.snxsch"]);
    let root = dir.join("b_root.snxsch");
    open_with(
        &mut app.document_state,
        &root,
        sheet(
            &["c_child.snxsch", "d_cached.snxsch"],
            &[("R?", 1), ("R?", 1), ("U?", 1), ("U?", 2), ("R1", 1)],
        ),
    );
    open_with(
        &mut app.document_state,
        &dir.join("d_cached.snxsch"),
        sheet(&[], &[("C?", 1), ("R?", 1), ("R1", 1), ("U1", 2)]),
    );
    app.document_state.active_path = Some(root);
    (app, dir)
}

fn refs_of(sheet: &SchematicSheet) -> Vec<String> {
    sheet.symbols.iter().map(|s| s.reference.clone()).collect()
}

/// `[a_page, b_root, c_child, d_cached]`, open sheets from their engines and
/// the others from disk.
fn all_refs(app: &Signex, dir: &Path) -> [Vec<String>; 4] {
    let name = ["a_page", "b_root", "c_child", "d_cached"];
    name.map(|n| {
        let path = dir.join(format!("{n}.snxsch"));
        match app.document_state.engines.get(&path) {
            Some(engine) => refs_of(engine.document()),
            None => {
                let text = std::fs::read_to_string(&path).expect("sheet on disk");
                refs_of(&SnxSchematic::parse(&text).expect("parse").sheet)
            }
        }
    })
}

fn expect(rows: [&[&str]; 4]) -> [Vec<String>; 4] {
    rows.map(|row| row.iter().map(|s| (*s).to_string()).collect())
}

fn run_annotate(mode: signex_engine::AnnotateMode) -> [Vec<String>; 4] {
    let (mut app, dir) = project();
    let task = app.handle_annotate(mode);
    settle_task(&mut app, task);
    let refs = all_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    refs
}

#[test]
fn incremental_annotate_numbers_open_and_unopened_sheets_as_before() {
    assert_eq!(
        run_annotate(signex_engine::AnnotateMode::Incremental),
        expect([
            GOLDEN_INCREMENTAL_A,
            GOLDEN_INCREMENTAL_B,
            GOLDEN_INCREMENTAL_C,
            GOLDEN_INCREMENTAL_D
        ])
    );
}

#[test]
fn reset_and_renumber_numbers_open_and_unopened_sheets_as_before() {
    assert_eq!(
        run_annotate(signex_engine::AnnotateMode::ResetAndRenumber),
        expect([
            GOLDEN_RESET_A,
            GOLDEN_RESET_B,
            GOLDEN_RESET_C,
            GOLDEN_RESET_D
        ])
    );
}

#[test]
fn reset_duplicates_resets_the_same_references_as_before() {
    let (mut app, dir) = project();
    let task = app.handle_reset_duplicate_designators();
    settle_task(&mut app, task);
    let refs = all_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        refs,
        expect([
            GOLDEN_DUPES_A,
            GOLDEN_DUPES_B,
            GOLDEN_DUPES_C,
            GOLDEN_DUPES_D
        ])
    );
}

const GOLDEN_INCREMENTAL_A: &[&str] = &["R4", "R3", "C1", "R3"];
const GOLDEN_INCREMENTAL_B: &[&str] = &["R5", "R6", "U2", "U2", "R1"];
const GOLDEN_INCREMENTAL_C: &[&str] = &["R7", "C2", "R3", "U1"];
const GOLDEN_INCREMENTAL_D: &[&str] = &["C3", "R8", "R1", "U1"];
const GOLDEN_RESET_A: &[&str] = &["R1", "R2", "C1", "R3"];
const GOLDEN_RESET_B: &[&str] = &["R4", "R5", "U1", "U1", "R6"];
const GOLDEN_RESET_C: &[&str] = &["R7", "C2", "R8", "U2"];
const GOLDEN_RESET_D: &[&str] = &["C3", "R9", "R10", "U2"];
// With b_root's last symbol (R1) locked: it keeps R1 and the rest number from R2.
const GOLDEN_RESET_LOCKED_A: &[&str] = &["R2", "R3", "C1", "R4"];
const GOLDEN_RESET_LOCKED_B: &[&str] = &["R5", "R6", "U1", "U1", "R1"];
const GOLDEN_RESET_LOCKED_C: &[&str] = &["R7", "C2", "R8", "U2"];
const GOLDEN_RESET_LOCKED_D: &[&str] = &["C3", "R9", "R10", "U2"];
const GOLDEN_DUPES_A: &[&str] = &["R?", "R?", "C?", "R?"];
const GOLDEN_DUPES_B: &[&str] = &["R?", "R?", "U?", "U?", "R?"];
const GOLDEN_DUPES_C: &[&str] = &["R?", "C?", "R?", "U1"];
const GOLDEN_DUPES_D: &[&str] = &["C?", "R?", "R?", "U1"];

// ---- #1753 item 3, #1835: locked parts, a child placed twice, parked engines.

#[test]
fn a_locked_part_keeps_its_designator_and_the_rest_number_around_it() {
    let (mut app, dir) = project();
    let root = dir.join("b_root.snxsch");
    let locked = app.document_state.engines[&root].document().symbols[4].uuid;
    app.ui_state.annotate_locked.insert(locked);
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let refs = all_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        refs,
        expect([
            GOLDEN_RESET_LOCKED_A,
            GOLDEN_RESET_LOCKED_B,
            GOLDEN_RESET_LOCKED_C,
            GOLDEN_RESET_LOCKED_D
        ])
    );
}

#[test]
fn a_parked_engine_is_numbered_in_memory_marked_dirty_and_not_written() {
    let (mut app, dir) = project();
    let parked = dir.join("d_cached.snxsch");
    assert!(!app.document_state.tabs.iter().any(|t| t.path == parked));
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let refs = refs_of(app.document_state.engines[&parked].document());
    let (dirty, written) = (
        app.document_state.is_document_dirty(&parked),
        parked.exists(),
    );
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(refs, GOLDEN_RESET_D);
    assert!(dirty, "the numbered parked sheet awaits the user's save");
    assert!(!written, "annotate never writes a sheet an engine holds");
}

/// `b_root` (open, active) places `m_child` twice; `m_child` is unopened.
fn project_with_a_child_placed_twice() -> (Signex, PathBuf, [Uuid; 2]) {
    placed_twice(&[("Q?", 1)], sheet(&[], &[("Q5", 1), ("Q6", 1)]))
}

fn placed_twice(root_refs: &[(&str, u32)], child: SchematicSheet) -> (Signex, PathBuf, [Uuid; 2]) {
    let dir = std::env::temp_dir().join(format!("signex-annotate-twice-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    write(&dir, "m_child.snxsch", child);
    let mut app = app_workspace(&dir.to_string_lossy(), &["b_root.snxsch"]);
    let ids = [Uuid::from_u128(1), Uuid::from_u128(2)];
    let mut root = sheet(&["m_child.snxsch", "m_child.snxsch"], root_refs);
    for (child, id) in root.child_sheets.iter_mut().zip(ids) {
        child.uuid = id;
    }
    let root_path = dir.join("b_root.snxsch");
    open_with(&mut app.document_state, &root_path, root);
    app.document_state.active_path = Some(root_path);
    (app, dir, ids)
}

fn designators_at(sheet: &SchematicSheet, id: Uuid) -> Vec<String> {
    let path = signex_types::schematic::InstancePath::from_chain(vec![id]);
    sheet
        .symbols
        .iter()
        .map(|s| s.designator_in(&path).to_string())
        .collect()
}

#[test]
fn a_child_placed_twice_gets_its_own_designators_per_placement_and_exports() {
    let (mut app, dir, [a, b]) = project_with_a_child_placed_twice();
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let text = std::fs::read_to_string(dir.join("m_child.snxsch")).expect("child on disk");
    let child = SnxSchematic::parse(&text).expect("parse").sheet;
    let root = app.document_state.engines[&dir.join("b_root.snxsch")]
        .document()
        .clone();
    let (at_a, at_b) = (designators_at(&child, a), designators_at(&child, b));

    let mut sheets = std::collections::HashMap::new();
    sheets.insert(dir.join("b_root.snxsch"), root.clone());
    sheets.insert(dir.join("m_child.snxsch"), child);
    let graph = crate::app::project_sheets::project_graph(
        &sheets,
        Some(&dir),
        Some(&dir.join("b_root.snxsch")),
    );
    let root_key = crate::app::project_sheets::sheet_key(&dir.join("b_root.snxsch"), Some(&dir));
    let netlist = signex_net::build_project_netlist(&signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &[signex_net::ProjectRoot { key: root_key }],
    });
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(refs_of(&root), ["Q1"]);
    assert_eq!(at_a, ["Q2", "Q3"]);
    assert_eq!(at_b, ["Q4", "Q5"]);
    assert!(
        !netlist.issues.iter().any(|issue| matches!(
            issue,
            signex_net::StitchIssue::SharedReferenceAcrossInstances { .. }
        )),
        "{:?}",
        netlist.issues
    );
}

#[test]
fn a_part_added_later_to_a_child_placed_twice_is_numbered_per_placement() {
    let (mut app, dir, [a, b]) = project_with_a_child_placed_twice();
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let child_path = dir.join("m_child.snxsch");
    let mut child = SnxSchematic::parse(&std::fs::read_to_string(&child_path).expect("child"))
        .expect("parse")
        .sheet;
    child.symbols.push(symbol("Q?", 1, 100.0));
    write(&dir, "m_child.snxsch", child);

    let task = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    settle_task(&mut app, task);
    let text = std::fs::read_to_string(&child_path).expect("child on disk");
    let child = SnxSchematic::parse(&text).expect("parse").sheet;
    std::fs::remove_dir_all(&dir).ok();

    let (new_a, new_b) = (
        designators_at(&child, a).pop().expect("new part"),
        designators_at(&child, b).pop().expect("new part"),
    );
    assert_ne!(new_a, new_b, "one shared designator across placements");
    assert!(!new_a.ends_with('?') && !new_b.ends_with('?'));
}

#[test]
fn the_preview_numbers_a_child_placed_twice_as_the_run_applies_it() {
    let (mut app, dir, [a, b]) = placed_twice(&[("Q1", 1), ("Q2", 1)], sheet(&[], &[("Q?", 1)]));
    // Sorts after `m_child`: it must be numbered past both placements.
    write(&dir, "z_other.snxsch", sheet(&[], &[("Q?", 1)]));
    let pages = &mut app.document_state.projects[0].data.sheets;
    let mut extra = pages[0].clone();
    extra.filename = "z_other.snxsch".to_string();
    pages.push(extra);
    let rows_of = |app: &Signex, prefix: &str| -> Vec<String> {
        app.preview_project_annotations()
            .into_iter()
            .filter(|row| row.sheet.starts_with(prefix))
            .map(|row| row.proposed)
            .collect()
    };
    let rows = rows_of(&app, "m_child");
    let other_row = rows_of(&app, "z_other");

    let task = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    settle_task(&mut app, task);
    let text = std::fs::read_to_string(dir.join("m_child.snxsch")).expect("child on disk");
    let child = SnxSchematic::parse(&text).expect("parse").sheet;
    let text = std::fs::read_to_string(dir.join("z_other.snxsch")).expect("other on disk");
    let other = SnxSchematic::parse(&text).expect("parse").sheet;
    std::fs::remove_dir_all(&dir).ok();

    let applied = [designators_at(&child, a), designators_at(&child, b)].concat();
    assert_eq!(applied, ["Q3", "Q4"]);
    assert_eq!(rows, applied, "one preview row per placement");
    assert_eq!(
        other.symbols[0].reference, "Q5",
        "the run numbers it past both"
    );
    assert_eq!(other_row, ["Q5"], "a later sheet is not shifted by one");
}

#[test]
fn reset_and_renumber_refuses_the_run_when_a_sheet_would_be_skipped() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project();
    let skipped = dir.join("c_child.snxsch");
    app.document_state.pending_opens.insert(skipped);
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let refs = all_refs(&app, &dir);
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .map(|n| n.detail.clone())
        .unwrap_or_default();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        refs,
        expect([
            &["R?", "R3", "C?", "R3"],
            &["R?", "R?", "U?", "U?", "R1"],
            &["R?", "C?", "R3", "U1"],
            &["C?", "R?", "R1", "U1"],
        ]),
        "nothing was renumbered"
    );
    assert!(
        notice.contains("c_child.snxsch") && notice.contains("Nothing was changed"),
        "{notice}"
    );
}

#[test]
fn incremental_annotate_still_skips_a_sheet_it_cannot_write() {
    let (mut app, dir) = project();
    app.document_state
        .pending_opens
        .insert(dir.join("c_child.snxsch"));
    let task = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    settle_task(&mut app, task);
    let refs = all_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        refs[2],
        ["R?", "C?", "R3", "U1"],
        "the blocked sheet is as it was"
    );
    assert_ne!(
        refs[1],
        ["R?", "R?", "U?", "U?", "R1"],
        "the others are numbered"
    );
}

#[test]
fn reset_duplicates_resets_a_child_placed_twice_per_placement() {
    let mut child = sheet(&[], &[("Q1", 1)]);
    let (a, b) = (
        signex_types::schematic::InstancePath::from_chain(vec![Uuid::from_u128(1)]),
        signex_types::schematic::InstancePath::from_chain(vec![Uuid::from_u128(2)]),
    );
    child.symbols[0].set_instance_designator(&a, "Q1".to_string());
    child.symbols[0].set_instance_designator(&b, "Q3".to_string());
    let (mut app, dir, _) = placed_twice(&[("Q1", 1)], child);
    let task = app.handle_reset_duplicate_designators();
    settle_task(&mut app, task);
    let text = std::fs::read_to_string(dir.join("m_child.snxsch")).expect("child on disk");
    let child = SnxSchematic::parse(&text).expect("parse").sheet;
    let root = refs_of(app.document_state.engines[&dir.join("b_root.snxsch")].document());
    std::fs::remove_dir_all(&dir).ok();
    let symbol = &child.symbols[0];
    assert_eq!(root, ["Q?"]);
    assert_eq!(
        symbol.designator_in(&a),
        "Q?",
        "the duplicate placement was reset"
    );
    assert_eq!(
        symbol.designator_in(&b),
        "Q3",
        "the other placement keeps its own"
    );
}

#[test]
fn reset_and_renumber_refuses_the_run_when_a_sheet_cannot_be_read() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project();
    std::fs::write(dir.join("c_child.snxsch"), "not a schematic").expect("corrupt");
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let root = refs_of(app.document_state.engines[&dir.join("b_root.snxsch")].document());
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .map(|n| n.detail.clone())
        .unwrap_or_default();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        root,
        ["R?", "R?", "U?", "U?", "R1"],
        "nothing was renumbered"
    );
    assert!(
        notice.contains("c_child.snxsch") && notice.contains("could not be read"),
        "{notice}"
    );
}

#[test]
fn a_child_left_with_one_placement_is_numbered_where_it_resolves() {
    // A child placed twice and annotated; then one sheet symbol is deleted
    // and a new part goes on the root.
    let (mut app, dir, [_, b]) = project_with_a_child_placed_twice();
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let root_path = dir.join("b_root.snxsch");
    let mut root = app.document_state.engines[&root_path].document().clone();
    root.child_sheets.retain(|child| child.uuid == b);
    root.symbols.push(symbol("Q?", 1, 100.0));
    open_with(&mut app.document_state, &root_path, root);

    let task = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    settle_task(&mut app, task);
    let root_refs = refs_of(app.document_state.engines[&root_path].document());
    let text = std::fs::read_to_string(dir.join("m_child.snxsch")).expect("child on disk");
    let child = SnxSchematic::parse(&text).expect("parse").sheet;
    std::fs::remove_dir_all(&dir).ok();
    let at_b = designators_at(&child, b);
    assert!(
        !at_b.contains(&root_refs[1]),
        "the root's new part got {} although the remaining placement resolves to {at_b:?} (child reference {:?})",
        root_refs[1],
        refs_of(&child)
    );
}
