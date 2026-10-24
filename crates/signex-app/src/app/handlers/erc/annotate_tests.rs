//! Multi-unit parts through the real Annotate and Reset Duplicate Designators
//! handlers (#691): units of one part are one part, wherever they sit.

use std::path::PathBuf;

use signex_types::format::SnxSchematic;
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

fn unit(reference: &str, unit: u32, y: f64) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
        lib_id: "Amplifier:LM358".to_string(),
        reference: reference.to_string(),
        unit,
        position: Point::new(0.0, y),
        ..Default::default()
    }
}

fn sheet_of(children: &[&str], symbols: Vec<Symbol>) -> SchematicSheet {
    let mut sheet = sheet_with_net("R?", "NET", children);
    sheet.symbols = symbols;
    sheet
}

/// `a_root.snxsch` is open and active; `z_child.snxsch` is an unopened
/// hierarchical child on disk. Returns the app and the project directory.
fn project(root: Vec<Symbol>, child: Vec<Symbol>) -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-annotate-units-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let child_text = SnxSchematic::new(sheet_of(&[], child))
        .write_string()
        .expect("serialize child");
    std::fs::write(dir.join("z_child.snxsch"), child_text).expect("write child");

    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    let root_path = dir.join("a_root.snxsch");
    open_with(
        &mut app.document_state,
        &root_path,
        sheet_of(&["z_child.snxsch"], root),
    );
    app.document_state.active_path = Some(root_path);
    (app, dir)
}

fn root_refs(app: &Signex, dir: &std::path::Path) -> Vec<String> {
    app.document_state
        .engines
        .get(&dir.join("a_root.snxsch"))
        .expect("root engine")
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

fn child_refs(dir: &std::path::Path) -> Vec<String> {
    let text = std::fs::read_to_string(dir.join("z_child.snxsch")).expect("child on disk");
    SnxSchematic::parse(&text)
        .expect("child parses")
        .sheet
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

#[test]
fn reset_duplicates_leaves_units_of_one_part_alone() {
    let (mut app, dir) = project(vec![unit("U1", 1, 0.0), unit("U1", 2, 10.0)], vec![]);
    let _ = app.handle_reset_duplicate_designators();
    let refs = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(refs, ["U1", "U1"]);
}

#[test]
fn reset_duplicates_still_resets_a_real_duplicate() {
    let (mut app, dir) = project(vec![unit("U1", 1, 0.0), unit("U1", 1, 10.0)], vec![]);
    let _ = app.handle_reset_duplicate_designators();
    let refs = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(refs, ["U?", "U?"]);
}

#[test]
fn reset_duplicates_leaves_units_split_across_sheets_alone() {
    let (mut app, dir) = project(vec![unit("U1", 1, 0.0)], vec![unit("U1", 2, 0.0)]);
    let _ = app.handle_reset_duplicate_designators();
    let (root, child) = (root_refs(&app, &dir), child_refs(&dir));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        (root, child),
        (vec!["U1".to_string()], vec!["U1".to_string()])
    );
}

#[test]
fn reset_and_renumber_keeps_a_part_split_across_sheets_together() {
    let (mut app, dir) = project(vec![unit("U1", 1, 0.0)], vec![unit("U1", 2, 0.0)]);
    let _ = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    let (root, child) = (root_refs(&app, &dir), child_refs(&dir));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, child, "one part, one designator across sheets");
    assert!(!root[0].ends_with('?'));
}

#[test]
fn a_locked_unannotated_unit_keeps_its_siblings_unannotated() {
    let locked = unit("U?", 1, 0.0);
    let locked_uuid = locked.uuid;
    let (mut app, dir) = project(vec![locked, unit("U?", 2, 10.0)], vec![]);
    app.ui_state.annotate_locked.insert(locked_uuid);
    let _ = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    let refs = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(refs, ["U?", "U?"]);
}
