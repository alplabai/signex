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
const GOLDEN_RESET_A: &[&str] = &["R4", "R5", "C1", "R6"];
const GOLDEN_RESET_B: &[&str] = &["R7", "R8", "U2", "U2", "R9"];
const GOLDEN_RESET_C: &[&str] = &["R10", "C2", "R11", "U3"];
const GOLDEN_RESET_D: &[&str] = &["C3", "R12", "R13", "U3"];
const GOLDEN_DUPES_A: &[&str] = &["R?", "R?", "C?", "R?"];
const GOLDEN_DUPES_B: &[&str] = &["R?", "R?", "U?", "U?", "R?"];
const GOLDEN_DUPES_C: &[&str] = &["R?", "C?", "R?", "U1"];
const GOLDEN_DUPES_D: &[&str] = &["C?", "R?", "R?", "U1"];
