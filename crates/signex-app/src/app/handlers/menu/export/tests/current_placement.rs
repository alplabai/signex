//! #1133 — "Current page" prints the copy of a placed-twice child the user
//! descended through, not a guess. Fixtures are real files because the descend
//! routes open sheets from disk.

use std::path::{Path, PathBuf};

use signex_output::PageRange;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{InstancePath, SelectedItem, SelectedKind, SymbolInstance};
use uuid::Uuid;

use super::{app_workspace, sheet_with_net};
use crate::app::Signex;
use crate::app::handlers::menu::export::{build_page_scope, isolate_undeclared_current};

/// Sheet symbol "A" and "B" in `top.snxsch`, both placing `child.snxsch`.
pub(crate) const PLACEMENT_A: u128 = 1;
pub(crate) const PLACEMENT_B: u128 = 2;

fn entry(placement: u128, reference: &str) -> SymbolInstance {
    SymbolInstance {
        project: "p".to_string(),
        path: InstancePath::from_chain(vec![Uuid::from_u128(placement)]).to_string(),
        reference: reference.to_string(),
        unit: 1,
    }
}

fn write(path: &Path, sheet: signex_types::schematic::SchematicSheet) {
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// `top` places `child` as A (R1) and B (R101); only `top` is a declared page
/// and only `top` is open, opened directly. Returns `(app, top, child)`.
pub(crate) fn placed_twice_on_disk(dir: &tempfile::TempDir) -> (Signex, PathBuf, PathBuf) {
    placed_twice_listing(dir, &["top.snxsch"])
}

/// [`placed_twice_on_disk`] with `listed` as the project's declared pages.
fn placed_twice_listing(dir: &tempfile::TempDir, listed: &[&str]) -> (Signex, PathBuf, PathBuf) {
    let _ = crate::diagnostics::init_logging();
    let (top, child) = (
        dir.path().join("top.snxsch"),
        dir.path().join("child.snxsch"),
    );
    let mut top_sheet = sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch", "child.snxsch"]);
    for (cs, (name, id)) in top_sheet
        .child_sheets
        .iter_mut()
        .zip([("A", PLACEMENT_A), ("B", PLACEMENT_B)])
    {
        cs.name = name.to_string();
        cs.uuid = Uuid::from_u128(id);
        // Side by side, so a canvas hit-test can pick one of them.
        cs.position = signex_types::schematic::Point::new(((id - 1) * 50) as f64, 0.0);
        cs.size = (10.0, 10.0);
    }
    let mut child_sheet = sheet_with_net("R1", "CHILD_NET", &[]);
    child_sheet.labels[0].label_type = signex_types::schematic::LabelType::Net;
    child_sheet.symbols[0].instances = vec![entry(PLACEMENT_A, "R1"), entry(PLACEMENT_B, "R101")];
    write(&top, top_sheet);
    write(&child, child_sheet);
    let mut app = app_workspace(&dir.path().to_string_lossy(), listed);
    app.open_or_focus_sheet_path(top.clone());
    (app, top, child)
}

/// Descend through the sheet symbol `placement` of the focused `top` by the
/// canvas route (double-click / context menu both end in this call).
pub(crate) fn descend_by_canvas(app: &mut Signex, placement: u128) {
    app.interaction_state.active_canvas_mut().selected = vec![SelectedItem::new(
        Uuid::from_u128(placement),
        SelectedKind::ChildSheet,
    )];
    assert!(app.open_selected_child_sheet());
}

/// What "Current page" prints for the focused `child`: its first symbol's
/// designator and the placement the page is labelled with.
pub(crate) fn current_page(app: &Signex, child: &Path) -> (String, String) {
    let (mut ctx, issues) = build_page_scope(&app.document_state).expect("scope");
    assert!(isolate_undeclared_current(
        &mut ctx,
        &issues,
        &app.document_state,
        child,
        app.document_state.tab_placements.get(child),
        &PageRange::Current
    ));
    let page = &ctx.sheets[0];
    (
        page.schematic.symbols[0].reference.clone(),
        page.placement.to_string(),
    )
}

pub(crate) fn placement_text(id: u128) -> String {
    format!("/{}", Uuid::from_u128(id))
}

#[test]
fn descending_through_b_prints_b() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    assert_eq!(
        current_page(&app, &child),
        ("R101".into(), placement_text(PLACEMENT_B))
    );
}

#[test]
fn descending_through_a_prints_a() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_A);
    assert_eq!(
        current_page(&app, &child),
        ("R1".into(), placement_text(PLACEMENT_A))
    );
}

#[test]
fn a_child_opened_directly_keeps_the_smallest_occurrence_guess() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    app.open_or_focus_sheet_path(child.clone());
    assert_eq!(
        current_page(&app, &child),
        ("R1".into(), placement_text(PLACEMENT_A))
    );
}

#[test]
fn descending_again_through_another_placement_on_the_open_tab_follows_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_A);
    app.open_or_focus_sheet_path(top.clone());
    descend_by_canvas(&mut app, PLACEMENT_B);
    assert_eq!(app.document_state.tabs.len(), 2, "same tab, no copy");
    assert_eq!(
        current_page(&app, &child),
        ("R101".into(), placement_text(PLACEMENT_B))
    );

    app.open_or_focus_sheet_path(top);
    descend_by_canvas(&mut app, PLACEMENT_A);
    assert_eq!(
        current_page(&app, &child),
        ("R1".into(), placement_text(PLACEMENT_A))
    );
}

#[test]
fn closing_the_child_tab_drops_its_placement() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    let index = app.document_state.active_tab;
    let _ = app.close_tab_now(index);
    app.open_or_focus_sheet_path(child.clone());
    assert_eq!(
        current_page(&app, &child),
        ("R1".into(), placement_text(PLACEMENT_A)),
        "a reopened tab was opened directly"
    );
}

#[test]
fn the_print_preview_renders_the_descended_copy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    let _ = app.handle_print_preview_requested();
    let preview = app.document_state.preview.as_mut().expect("preview");
    assert_eq!(preview.source, child);
    preview.pdf_options.page_range = PageRange::Current;

    let job = app.preview_raster_job().expect("a page to render");

    assert_eq!(job.ctx.sheets.len(), 1);
    assert_eq!(
        job.ctx.sheets[0].placement.to_string(),
        placement_text(PLACEMENT_B)
    );
    assert_eq!(job.ctx.sheets[0].schematic.symbols[0].reference, "R101");
}

#[test]
fn a_deleted_sheet_symbol_does_not_leave_its_placement_printing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    let engine = app
        .document_state
        .engines
        .get_mut(&top)
        .expect("top engine");
    engine
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(
                Uuid::from_u128(PLACEMENT_B),
                SelectedKind::ChildSheet,
            )],
        })
        .expect("delete the sheet symbol");
    assert_eq!(engine.document().child_sheets.len(), 1);

    assert_eq!(
        current_page(&app, &child),
        ("R1".into(), placement_text(PLACEMENT_A)),
        "the vanished placement falls back to the guess"
    );
}

#[test]
fn a_rebound_sheet_symbol_does_not_leave_its_placement_printing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    write(
        &dir.path().join("other.snxsch"),
        sheet_with_net("R900", "O", &[]),
    );
    let engine = app
        .document_state
        .engines
        .get_mut(&top)
        .expect("top engine");
    let mut document = engine.document().clone();
    document.child_sheets[1].filename = "other.snxsch".into();
    *engine = signex_engine::Engine::new(document).expect("engine");

    assert_eq!(
        current_page(&app, &child),
        ("R1".into(), placement_text(PLACEMENT_A))
    );
}

#[test]
fn double_clicking_a_sheet_symbol_records_that_placement() {
    use crate::canvas::CanvasEvent;
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    app.refresh_panel_ctx();

    // Sheet symbol B sits at x = 50..60.
    let _ = app.update(crate::app::Message::CanvasEvent(
        CanvasEvent::DoubleClicked {
            world_x: 55.0,
            world_y: 5.0,
            screen_x: 0.0,
            screen_y: 0.0,
        },
    ));

    assert_eq!(app.active_tab_path().as_deref(), Some(child.as_path()));
    assert_eq!(
        current_page(&app, &child),
        ("R101".into(), placement_text(PLACEMENT_B))
    );
}

#[test]
fn save_as_forgets_the_placement_of_the_old_file() {
    use crate::app::{FileMsg, Message};
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    let copy = dir.path().join("copy.snxsch");

    let write = app.update(Message::File(FileMsg::SaveAs {
        from_path: child.clone(),
        to_path: copy.clone(),
    }));
    // The write runs off the UI thread; deliver its completion to `update`.
    let mut stream = iced_runtime::task::into_stream(write).expect("a write task");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let done = runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = futures::StreamExt::next(&mut stream).await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in done {
        let _ = app.update(message);
    }

    assert_eq!(app.active_tab_path().as_deref(), Some(copy.as_path()));
    assert_eq!(app.document_state.tab_placements.get(&child), None);
    assert_eq!(app.document_state.tab_placements.get(&copy), None);
}

/// Messages-panel warnings about `dir`'s vanished placements.
fn dead_placement_warnings(dir: &Path) -> usize {
    let _ = crate::diagnostics::init_logging();
    let marker = dir.to_string_lossy().into_owned();
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|e| {
            e.message.contains(&marker) && e.message.contains("was opened through sheet placement")
        })
        .count()
}

#[test]
fn renaming_the_child_file_drops_its_placement_without_a_warning() {
    use crate::app::{Message, RenameDialogState, RenameMsg};
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    // Let the recovery check the open queued report, as the runtime does
    // after the update that opened the tab: a rename waits for it (#1323).
    let checks = app.start_due_recovery_checks();
    crate::app::handlers::menu::export::write::settle(&mut app, checks);
    let renamed = dir.path().join("renamed.snxsch");
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: child.clone(),
        tree_path: vec![0],
        buffer: "renamed.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });

    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(renamed.exists(), "the file moved");
    assert_eq!(app.document_state.tab_placements.get(&child), None);
    assert_eq!(app.document_state.tab_placements.get(&renamed), None);
    assert_eq!(dead_placement_warnings(dir.path()), 0);
}

#[test]
fn a_vanished_placement_is_reported_once_across_renders() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    let _ = app.handle_print_preview_requested();
    app.document_state
        .preview
        .as_mut()
        .expect("preview")
        .pdf_options
        .page_range = PageRange::Current;
    let engine = app
        .document_state
        .engines
        .get_mut(&top)
        .expect("top engine");
    engine
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(
                Uuid::from_u128(PLACEMENT_B),
                SelectedKind::ChildSheet,
            )],
        })
        .expect("delete the sheet symbol");

    let first = app.preview_raster_job().expect("a page");
    let second = app.preview_raster_job().expect("a page");

    assert_eq!(
        first.ctx.sheets[0].placement.to_string(),
        placement_text(PLACEMENT_A)
    );
    assert_eq!(
        second.ctx.sheets[0].placement.to_string(),
        placement_text(PLACEMENT_A)
    );
    assert_eq!(app.document_state.tab_placements.get(&child), None);
    assert_eq!(dead_placement_warnings(dir.path()), 1);
}

#[test]
fn a_vanished_copy_of_a_declared_page_is_reported_too() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = placed_twice_listing(&dir, &["top.snxsch", "child.snxsch"]);
    descend_by_canvas(&mut app, PLACEMENT_B);
    let engine = app
        .document_state
        .engines
        .get_mut(&top)
        .expect("top engine");
    engine
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(
                Uuid::from_u128(PLACEMENT_B),
                SelectedKind::ChildSheet,
            )],
        })
        .expect("delete the sheet symbol");
    let _ = app.handle_print_preview_requested();
    app.document_state
        .preview
        .as_mut()
        .expect("preview")
        .pdf_options
        .page_range = PageRange::Current;

    let job = app.preview_raster_job().expect("a page");
    let _ = app.preview_raster_job();

    assert_eq!(job.ctx.sheets[job.ctx.current_sheet_index()].path, child);
    assert_eq!(dead_placement_warnings(dir.path()), 1);
}

/// `top` places `mid.snxsch` as M1 (uuid 1) and M2 (uuid 2); `mid` places
/// `leaf.snxsch` as L (uuid 3); `leaf` holds R1 under /M1/L and R201 under
/// /M2/L. Only `top` is declared and open. Returns `(app, top, mid, leaf)`.
pub(crate) fn three_levels_on_disk(dir: &tempfile::TempDir) -> (Signex, PathBuf, PathBuf, PathBuf) {
    let path = |name: &str| dir.path().join(name);
    let mut top = sheet_with_net("R_TOP", "TOP_NET", &["mid.snxsch", "mid.snxsch"]);
    for (cs, (name, id)) in top.child_sheets.iter_mut().zip([("M1", 1), ("M2", 2)]) {
        cs.name = name.to_string();
        cs.uuid = Uuid::from_u128(id);
    }
    let mut mid = sheet_with_net("R_MID", "MID_NET", &["leaf.snxsch"]);
    mid.child_sheets[0].name = "L".to_string();
    mid.child_sheets[0].uuid = Uuid::from_u128(3);
    let mut leaf = sheet_with_net("R1", "LEAF_NET", &[]);
    leaf.labels[0].label_type = signex_types::schematic::LabelType::Net;
    let chain = |parent: u128| {
        InstancePath::from_chain(vec![Uuid::from_u128(parent), Uuid::from_u128(3)]).to_string()
    };
    leaf.symbols[0].instances = [(1, "R1"), (2, "R201")]
        .map(|(parent, reference)| SymbolInstance {
            project: "p".to_string(),
            path: chain(parent),
            reference: reference.to_string(),
            unit: 1,
        })
        .to_vec();
    write(&path("top.snxsch"), top);
    write(&path("mid.snxsch"), mid);
    write(&path("leaf.snxsch"), leaf);
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.open_or_focus_sheet_path(path("top.snxsch"));
    (
        app,
        path("top.snxsch"),
        path("mid.snxsch"),
        path("leaf.snxsch"),
    )
}

fn focus_tab(app: &mut Signex, path: &Path) {
    app.open_or_focus_sheet_path(path.to_path_buf());
}

#[test]
fn a_grandchild_remembers_the_whole_chain_it_was_reached_through() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, _mid, leaf) = three_levels_on_disk(&dir);
    descend_by_canvas(&mut app, 2);
    descend_by_canvas(&mut app, 3);
    let chain = InstancePath::from_chain(vec![Uuid::from_u128(2), Uuid::from_u128(3)]);
    assert_eq!(
        current_page(&app, &leaf),
        ("R201".into(), chain.to_string())
    );
}

#[test]
fn up_hierarchy_hands_the_parent_the_placement_the_child_came_from() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, mid, leaf) = three_levels_on_disk(&dir);
    descend_by_canvas(&mut app, 2);
    descend_by_canvas(&mut app, 3);
    // Re-enter mid through the other placement; the leaf tab stays as it was.
    focus_tab(&mut app, &top);
    descend_by_canvas(&mut app, 1);
    focus_tab(&mut app, &leaf);

    app.ascend_to_parent_sheet();
    assert_eq!(app.active_tab_path().as_deref(), Some(mid.as_path()));
    descend_by_canvas(&mut app, 3);

    let chain = InstancePath::from_chain(vec![Uuid::from_u128(2), Uuid::from_u128(3)]);
    assert_eq!(
        current_page(&app, &leaf),
        ("R201".into(), chain.to_string())
    );
}

#[test]
fn a_declared_child_placed_twice_prints_the_copy_descended_through() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, child) = placed_twice_listing(&dir, &["top.snxsch", "child.snxsch"]);
    descend_by_canvas(&mut app, PLACEMENT_B);

    let (mut ctx, issues) = build_page_scope(&app.document_state).expect("scope");
    assert!(
        !isolate_undeclared_current(
            &mut ctx,
            &issues,
            &app.document_state,
            &child,
            app.document_state.tab_placements.get(&child),
            &PageRange::Current
        ),
        "a declared page keeps the caller's file filter"
    );

    let page = &ctx.sheets[ctx.current_sheet_index()];
    assert_eq!(page.path, child);
    assert_eq!(page.placement.to_string(), placement_text(PLACEMENT_B));
    assert_eq!(page.schematic.symbols[0].reference, "R101");
}
