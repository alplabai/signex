//! Block A tests 5 to 11: what each tail refreshes besides the dirty mark,
//! and the differences between the active-sheet tail (A1,
//! `finish_schematic_mutation`) and the by-path tail (A2,
//! `after_sheet_change`).

use std::path::Path;

use signex_engine::{Command, DocumentPatch};
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::{dirty, paper};
use crate::app::Signex;
use crate::app::command::queries::tests::{WIRE_A, app_with, path, uid};
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;
use crate::app::mutation_gateway::MutationOutcome;

fn colour_wire_a() -> Command {
    let mut colors = std::collections::HashMap::new();
    colors.insert(uid(WIRE_A), colours::color(6));
    Command::SetNetColors { colors }
}

pub(super) fn content_clears(app: &Signex) -> u32 {
    app.interaction_state.canvas.content_cache_clears
}

/// Start an agent's edit list on the sheet at `path`, as `run_as_one_step`
/// and `apply_edits` do.
pub(super) fn begin_agent_run(app: &mut Signex, path: &Path) {
    let edits = &mut app.document_state.agent_edits;
    edits.target = Some(path.to_path_buf());
    edits.owed = DocumentPatch::NONE;
}

// ---- Tests 5 (d) and 7 (f): inside an agent's run.

#[test]
fn inside_an_agents_run_both_tails_owe_the_refresh_and_leave_dirty_and_slots_alone() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let a = path("a.snxsch");
    let clears = content_clears(&app);
    begin_agent_run(&mut app, &a);

    assert!(app.apply_engine_command(paper("A3"), true, true));
    app.after_sheet_change(&a, DocumentPatch::WIRES);

    let owed = app.document_state.agent_edits.owed;
    assert_eq!(
        owed,
        DocumentPatch::PAPER | DocumentPatch::WIRES,
        "both owed"
    );
    assert!(!dirty(&app, &a), "the dirty mark waits for the run's end");
    assert_eq!(content_clears(&app), clears, "no slot was repainted");

    // The run's end: the owed refresh lands once, through the by-path tail.
    app.document_state.agent_edits.target = None;
    let owed = std::mem::take(&mut app.document_state.agent_edits.owed);
    app.after_sheet_change(&a, owed);
    assert!(dirty(&app, &a));
    assert!(content_clears(&app) > clears);
}

#[test]
fn a_gateway_call_inside_an_agents_run_reports_changed() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    begin_agent_run(&mut app, &path("a.snxsch"));
    assert_eq!(
        app.apply_engine_command_checked(paper("A3"), true, true),
        MutationOutcome::Changed
    );
}

// ---- Test 6 (e): paper by path on the active sheet.

/// A1 moves the panel context's paper and the drawn page with a PAPER patch
/// (`a_paper_change_and_its_undo_reach_the_panel_context_and_the_drawn_page`);
/// A2 shares that step since #2645, so on the active sheet both follow.
#[test]
fn a_paper_change_by_path_on_the_active_sheet_moves_the_panel_and_the_drawn_page() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let a = path("a.snxsch");
    let canvas = app.interaction_state.active_canvas();
    let page = (canvas.paper_width_mm, canvas.paper_height_mm);
    assert_eq!(page, crate::panels::paper_dimensions("A4"));
    assert_eq!(app.execute_on_sheet(&a, paper("A3")), Ok(true));
    assert_eq!(app.document_state.engines[&a].document().paper_size, "A3");
    assert_eq!(app.document_state.panel_ctx.paper_size, "A3");
    let canvas = app.interaction_state.active_canvas();
    assert_eq!(
        (canvas.paper_width_mm, canvas.paper_height_mm),
        crate::panels::paper_dimensions("A3")
    );
}

// ---- Test 8 (g): a selected wire the edit removed.

fn select_wire_a(app: &mut Signex) -> SelectedItem {
    let wire = SelectedItem::new(uid(WIRE_A), SelectedKind::Wire);
    app.interaction_state.active_canvas_mut().selected = vec![wire];
    wire
}

fn delete(item: SelectedItem) -> Command {
    Command::DeleteSelection { items: vec![item] }
}

fn selection_holds(app: &Signex, item: SelectedItem) -> bool {
    app.interaction_state
        .active_canvas()
        .selected
        .contains(&item)
}

#[test]
fn an_edit_that_removes_a_selected_wire_drops_it_from_the_selection_through_both_tails() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let wire = select_wire_a(&mut app);
    assert!(app.apply_engine_command(delete(wire), true, false));
    assert!(!selection_holds(&app, wire), "A1 prunes it");

    let mut app = app_with(&[("a.snxsch", 100)]);
    let wire = select_wire_a(&mut app);
    assert_eq!(
        app.execute_on_sheet(&path("a.snxsch"), delete(wire)),
        Ok(true)
    );
    assert!(
        !selection_holds(&app, wire),
        "A2 prunes it on the active sheet"
    );
}

// ---- Test 9 (h): the active sheet's ERC markers.

/// Findings cached for another sheet only, and a marker on the active canvas
/// that no recomputation from that cache would draw.
fn stale_marker(app: &mut Signex) {
    app.ui_state.erc_violations_by_path.insert(
        std::env::temp_dir().join("edit-tail-seam-other.snxsch"),
        Vec::new(),
    );
    app.interaction_state.active_canvas_mut().erc_markers = vec![crate::canvas::ErcMarker {
        x: 1.0,
        y: 2.0,
        severity: crate::canvas::ErcMarkerSeverity::Warning,
        net_level: false,
        caption: None,
    }];
}

fn markers(app: &Signex) -> usize {
    app.interaction_state.active_canvas().erc_markers.len()
}

#[test]
fn an_edit_refreshes_the_active_sheets_erc_markers_and_a_colour_edit_does_not() {
    let a = path("a.snxsch");
    type Edit = fn(&mut Signex, &Path, Command);
    let tails: [(&str, Edit); 2] = [
        ("A1", |app, _, command| {
            assert!(app.apply_engine_command(command, true, true));
        }),
        ("A2", |app, path, command| {
            assert_eq!(app.execute_on_sheet(path, command), Ok(true));
        }),
    ];
    for (tail, edit) in tails {
        let mut app = app_with(&[("a.snxsch", 100)]);
        stale_marker(&mut app);
        edit(&mut app, &a, colour_wire_a());
        assert_eq!(markers(&app), 1, "{tail}: a colour keeps the markers");
        edit(&mut app, &a, paper("A3"));
        assert_eq!(markers(&app), 0, "{tail}: an edit recomputes them");
    }
}

// ---- Test 10 (i): the properties summary.

#[test]
fn an_edit_with_selection_info_refreshes_the_properties_summary() {
    for (wanted, expected) in [(true, 1), (false, 99)] {
        let mut app = app_with(&[("a.snxsch", 100)]);
        select_wire_a(&mut app);
        app.document_state.panel_ctx.selection_count = 99;
        assert!(app.apply_engine_command(paper("A3"), true, wanted));
        assert_eq!(
            app.document_state.panel_ctx.selection_count, expected,
            "update_selection_info = {wanted}"
        );
    }
}

// ---- Test 11: the red dot on the project tree row.

fn red_dot(app: &Signex, file: &str) -> bool {
    fn find<'a>(
        rows: &'a [crate::panels::SheetInfo],
        file: &str,
    ) -> Option<&'a crate::panels::SheetInfo> {
        rows.iter().find_map(|row| {
            (row.filename == file)
                .then_some(row)
                .or_else(|| find(&row.children, file))
        })
    }
    let rows = &app.document_state.panel_ctx.projects[0].sheets;
    find(rows, file).expect("the sheet's tree row").is_dirty
}

#[test]
fn an_edit_puts_the_red_dot_on_the_tree_row_in_the_same_call() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    assert!(!red_dot(&app, "top.snxsch"));
    assert!(app.apply_engine_command(paper("A3"), true, true));
    assert!(red_dot(&app, "top.snxsch"), "A1");
    assert!(app.apply_engine_undo(true));
    assert!(!red_dot(&app, "top.snxsch"), "A1 undo");

    assert_eq!(app.execute_on_sheet(&top, paper("A3")), Ok(true));
    assert!(red_dot(&app, "top.snxsch"), "A2");
    assert_eq!(app.step_sheet_on(&top, true), Ok(true));
    assert!(!red_dot(&app, "top.snxsch"), "A2 undo");
}
