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

/// Rewrite the on-disk child as if a newer Signex had written it, and return
/// its exact bytes so a test can prove it was left alone.
fn make_child_newer_minor(dir: &std::path::Path) -> String {
    let path = dir.join("z_child.snxsch");
    let text = std::fs::read_to_string(&path).expect("child on disk");
    let newer = text.replacen("format = \"snxsch/1\"", "format = \"snxsch/1.7\"", 1);
    assert_ne!(newer, text, "fixture must carry the newer token");
    std::fs::write(&path, &newer).expect("write newer child");
    newer
}

fn notice_detail(app: &Signex) -> String {
    app.document_state
        .error_notice
        .as_ref()
        .map(|n| n.detail.clone())
        .unwrap_or_default()
}

#[test]
fn annotate_leaves_an_unopened_newer_minor_child_alone_and_names_it() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
    let newer = make_child_newer_minor(&dir);
    let _ = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    let (root, on_disk) = (
        root_refs(&app, &dir),
        std::fs::read_to_string(dir.join("z_child.snxsch")).expect("child"),
    );
    let notice = notice_detail(&app);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(on_disk, newer, "the newer-minor child is untouched");
    assert_eq!(root, ["R2"], "the child's R1 still counts as taken");
    assert!(notice.contains("z_child.snxsch"), "{notice}");
}

#[test]
fn reset_duplicates_leaves_an_unopened_newer_minor_child_alone_and_names_it() {
    let (mut app, dir) = project(vec![unit("R1", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
    let newer = make_child_newer_minor(&dir);
    let _ = app.handle_reset_duplicate_designators();
    let (root, on_disk) = (
        root_refs(&app, &dir),
        std::fs::read_to_string(dir.join("z_child.snxsch")).expect("child"),
    );
    let notice = notice_detail(&app);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(on_disk, newer, "the newer-minor child is untouched");
    assert_eq!(root, ["R?"], "the editable side of the duplicate is reset");
    assert!(notice.contains("z_child.snxsch"), "{notice}");
}

#[test]
fn bulk_passes_do_not_dirty_an_open_newer_minor_sheet() {
    for reset in [false, true] {
        let (mut app, dir) = project(vec![unit("R1", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
        make_child_newer_minor(&dir);
        let child_path = dir.join("z_child.snxsch");
        let engine = signex_engine::Engine::open(&child_path).expect("newer minor still loads");
        app.document_state
            .engines
            .insert(child_path.clone(), engine);
        if reset {
            let _ = app.handle_reset_duplicate_designators();
        } else {
            let _ = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
        }
        let child_refs: Vec<String> = app.document_state.engines[&child_path]
            .document()
            .symbols
            .iter()
            .map(|s| s.reference.clone())
            .collect();
        let dirty = app.document_state.dirty_paths.contains(&child_path);
        let notice = notice_detail(&app);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(child_refs, ["R1"], "reset={reset}");
        assert!(!dirty, "reset={reset}: read-only sheet must not be dirtied");
        assert!(notice.contains("z_child.snxsch"), "reset={reset}: {notice}");
    }
}

/// A cache slot that kept an older sheet because an engine edit never
/// refreshed it must not survive the next narrow edit with stale geometry:
/// the partial copy is only valid when the slot matched the engine just
/// before that edit (#171).
#[test]
fn undocked_slot_stale_after_annotate_heals_on_the_next_narrow_edit() {
    let (mut app, dir) = project(vec![unit("U?", 1, 0.0)], vec![]);
    let root_path = dir.join("a_root.snxsch");
    let child_path = dir.join("z_child.snxsch");
    open_with(
        &mut app.document_state,
        &child_path,
        sheet_of(&[], vec![unit("U?", 2, 0.0)]),
    );
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: child_path.clone(),
            title: "child".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    let engine = &app.document_state.engines[&child_path];
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(
            engine.document(),
            engine.revision(),
        ),
    ));
    app.interaction_state.canvases.insert(window, slot);

    // Annotate runs from the root; the child's engine changes, its slot does not.
    let _ = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    let annotated = app.document_state.engines[&child_path].document().symbols[0]
        .reference
        .clone();
    assert!(!annotated.ends_with('?'));
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(root_path.as_path())
    );

    // The child becomes active and takes a wire-only edit.
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "z_child".into(),
        path: child_path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(child_path);
    let wire = signex_types::schematic::Wire {
        uuid: Uuid::new_v4(),
        start: Point::new(0.0, 0.0),
        end: Point::new(5.0, 0.0),
        ..Default::default()
    };
    assert!(app.apply_engine_command(
        signex_engine::Command::PlaceWireSegment { wire },
        false,
        false
    ));

    let shown = app.interaction_state.canvases[&window]
        .active_snapshot()
        .expect("snapshot")
        .symbols[0]
        .reference
        .clone();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        shown, annotated,
        "the undocked slot must show the new designator"
    );
}
