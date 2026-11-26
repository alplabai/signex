//! M3 of the slice 2 plan: after a multi-sheet run or its grouped undo, an
//! undocked window and a tile pane showing a member other than the active
//! sheet draw what that member's engine holds. Replace All and the grouped
//! undo are in `groups.rs`; Assign Footprints in `tests/regression`.

use std::path::Path;

use signex_engine::DocumentPatch;
use signex_types::schematic::SchematicSheet;

use super::groups::{tile, undock};
use super::members::renumbered;
use super::{paper, with_saved_cached_child};
use crate::app::Signex;
use crate::canvas::CanvasSlot;

/// The undocked windows and tile pane on `path`.
fn slots_on<'a>(app: &'a Signex, path: &Path) -> Vec<&'a CanvasSlot> {
    let windows = &app.ui_state.windows;
    app.interaction_state
        .canvases
        .iter()
        .filter(|(id, _)| {
            matches!(
                windows.get(id),
                Some(crate::app::state::WindowKind::UndockedTab { path: shown, .. })
                    if shown == path
            )
        })
        .map(|(_, slot)| slot)
        .chain(app.interaction_state.tile_panes.schematic.get(path))
        .collect()
}

/// What every window and pane on `path` draws, as `read` sees a sheet; at
/// least one of them must exist.
fn drawn<T: PartialEq + std::fmt::Debug>(
    app: &Signex,
    path: &Path,
    read: impl Fn(&SchematicSheet) -> T,
) -> T {
    let slots = slots_on(app, path);
    assert!(!slots.is_empty(), "something shows {}", path.display());
    let first = read(slots[0].active_snapshot().expect("a render cache"));
    for slot in &slots[1..] {
        assert_eq!(read(slot.active_snapshot().expect("a render cache")), first);
    }
    first
}

fn live<T>(app: &Signex, path: &Path, read: impl Fn(&SchematicSheet) -> T) -> T {
    read(app.document_state.engines[path].document())
}

// ---- Number Schematic Sheets (`commit_sheet_write`).

/// Show `sheet`, the file of the closed `path`, in an undocked window.
fn show_closed(app: &mut Signex, path: &Path, sheet: &SchematicSheet) {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "z".to_string(),
        },
    );
    let mut slot = CanvasSlot::default();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(sheet, 0),
    ));
    app.interaction_state.canvases.insert(window, slot);
}

#[test]
fn a_sheet_numbering_run_repaints_a_window_on_its_non_active_member() {
    use crate::app::sheet_numbering::apply_tests::{apply, closed_parent_sheet};
    let mut rig = renumbered();
    let other = rig.closed_parent.clone();
    show_closed(&mut rig.app, &other, &closed_parent_sheet());
    let page = |sheet: &SchematicSheet| {
        let instances = &sheet.child_sheets[0].instances;
        instances.first().map(|i| i.page.clone())
    };
    assert_eq!(drawn(&rig.app, &other, page), None, "drawn unnumbered");
    apply(&mut rig);
    assert_eq!(live(&rig.app, &other, page).as_deref(), Some("1"));
    assert_eq!(drawn(&rig.app, &other, page).as_deref(), Some("1"));
}

// ---- A board swap's linked sheet (`commit_linked_footprints`) and its
// ---- grouped undo from the board (`step_board_undo_group`).

fn units(sheet: &SchematicSheet) -> Vec<(uuid::Uuid, u32)> {
    sheet.symbols.iter().map(|s| (s.uuid, s.unit)).collect()
}

#[test]
fn a_board_gate_swap_and_its_undo_repaint_the_linked_non_active_sheet() {
    use crate::app::command::{CommandArgs, CommandOutcome};
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, sheet) =
        crate::app::handlers::pcb_component::part_swap_tests::app_with_part(dir.path());
    undock(&mut app, &sheet);
    tile(&mut app, &sheet);
    let before = live(&app, &sheet, units);
    let command = crate::keymap::AppCommandId::new("swap_gates").unwrap();
    let document = format!("document={}", board.display());
    let args =
        CommandArgs::parse_operands(&command, &[document.as_str(), "designator=U1", "units=1,2"])
            .unwrap();
    let ran = app.run_headless_command_in_test(&command, args).0;
    assert!(matches!(ran, CommandOutcome::Answered(_)), "{ran:?}");
    assert_ne!(live(&app, &sheet, units), before, "the gates traded");
    assert_eq!(drawn(&app, &sheet, units), live(&app, &sheet, units));

    assert!(app.step_board_history(&board, true));
    assert_eq!(live(&app, &sheet, units), before, "undone with the board");
    assert_eq!(drawn(&app, &sheet, units), before);
}

// ---- Annotate over an open sheet that is not the active one.

/// `a_root` (active) places `child`; both open, each with two `R?` parts.
fn root_and_open_child(dir: &tempfile::TempDir) -> (Signex, std::path::PathBuf) {
    use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
    let (root, child) = (
        dir.path().join("a_root.snxsch"),
        dir.path().join("child.snxsch"),
    );
    let unnumbered = |children: &[&str]| {
        let mut sheet = sheet_with_net("R?", "NET", children);
        let mut second = sheet.symbols[0].clone();
        second.uuid = uuid::Uuid::now_v7();
        second.position = signex_types::schematic::Point::new(0.0, 20.0);
        sheet.symbols.push(second);
        sheet
    };
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a_root.snxsch"]);
    open_with(&mut app.document_state, &child, unnumbered(&[]));
    open_with(
        &mut app.document_state,
        &root,
        unnumbered(&["child.snxsch"]),
    );
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "a_root.snxsch".into(),
        path: root.clone(),
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root);
    app.ui_state.annotate_undo = crate::annotate_undo::AnnotateUndo::Project;
    (app, child)
}

fn references(sheet: &SchematicSheet) -> Vec<String> {
    sheet.symbols.iter().map(|s| s.reference.clone()).collect()
}

#[test]
fn an_annotate_and_its_undo_repaint_an_open_non_active_child() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, child) = root_and_open_child(&dir);
    undock(&mut app, &child);
    tile(&mut app, &child);
    let before = live(&app, &child, references);
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    crate::app::path_ops::settle_task(&mut app, task);
    let annotated = live(&app, &child, references);
    assert_ne!(annotated, before, "the child was annotated");
    assert_eq!(drawn(&app, &child, references), annotated);

    app.handle_undo_requested();
    assert_eq!(live(&app, &child, references), before, "undone");
    assert_eq!(drawn(&app, &child, references), before);
}

// ---- The agent's edit target owes its repaint (A2's rule).

#[test]
fn a_member_that_is_the_agents_edit_target_owes_its_repaint() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = with_saved_cached_child(&dir);
    undock(&mut app, &child);
    let size = |sheet: &SchematicSheet| sheet.paper_size.clone();
    let shown = drawn(&app, &child, size);
    assert_ne!(shown, "A2");
    let engine = app.document_state.engines.get_mut(&child).unwrap();
    engine.execute(paper("A2")).unwrap();

    app.document_state.agent_edits.target = Some(child.clone());
    app.repaint_member_sheet(&child, DocumentPatch::PAPER);
    assert_eq!(
        drawn(&app, &child, size),
        shown,
        "not repainted in the list"
    );
    assert_eq!(app.document_state.agent_edits.owed, DocumentPatch::PAPER);

    app.document_state.agent_edits.target = None;
    app.repaint_member_sheet(&child, DocumentPatch::PAPER);
    assert_eq!(drawn(&app, &child, size), "A2", "repainted outside it");
}
