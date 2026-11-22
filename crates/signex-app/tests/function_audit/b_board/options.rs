//! Row m93: board Properties with nothing selected, and the 2026-11-21
//! decisions on Snap Options and the board's one grid.

use pcb_geom::PickKind;
use signex_app::app::{Message, PcbMsg, PcbViewMsg, Signex, StatusBarRequest, UiMsg};
use signex_app::panels::{BoardField, BoardTab, ComponentSide, PanelKind, PcbBoardMsg};
use signex_app::pcb_canvas::SnapScope;
use signex_app::status_bar::{BoardCounter, BoardPopoverEvent};
use signex_engine::pcb::placement::grid_step_mm;
use signex_engine::pcb::snap::SnapObject;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::pcb_options::{DesignatorDisplay, GuideKind, PolygonNaming};
use uuid::Uuid;

use super::support::{Bench, all_kinds, assert_at, open};

fn status_grid(b: &Bench) -> f32 {
    b.app.status_grid(b.main).size_mm
}

fn type_grid(b: &mut Bench, text: &str) {
    b.board_panel(PcbBoardMsg::GridStepTyped(text.to_string()));
    b.board_panel(PcbBoardMsg::GridStepSubmitted);
}

fn field(b: &mut Bench, field: BoardField, text: &str) {
    b.board_panel(PcbBoardMsg::FieldTyped(field, text.to_string()));
    b.board_panel(PcbBoardMsg::FieldSubmitted(field));
}

fn pick_from_status_popover(b: &mut Bench, mm: f32) {
    let open = StatusBarRequest::BoardCounterClicked {
        window: None,
        counter: BoardCounter::Grid,
    };
    let message = Signex::status_bar_message(b.main, None, open);
    b.send(message);
    b.send(Message::Ui(UiMsg::StatusBar(
        StatusBarRequest::BoardPopover(BoardPopoverEvent::GridPresetPicked(mm)),
    )));
}

#[test]
fn a_typed_grid_step_is_the_one_grid_of_the_status_bar_and_properties() {
    let mut b = open(all_kinds());
    assert_eq!(status_grid(&b), 1.0);
    let edited = b.one_step(|b| {
        type_grid(b, "0.05");
        assert_eq!(status_grid(b), 0.05, "the status bar shows it at once");
        let shown = b.app.document_state.panel_ctx.pcb_board.as_ref();
        assert_eq!(shown.and_then(|s| s.grid_mm), Some(0.05), "Grid Manager");
    });
    assert_eq!(grid_step_mm(&edited), 0.05);
    assert_eq!(status_grid(&b), 1.0, "undo puts the status bar back");
}

#[test]
fn typing_a_grid_step_changes_nothing_until_enter() {
    let mut b = open(all_kinds());
    b.refused(|b| b.board_panel(PcbBoardMsg::GridStepTyped("0.5".to_string())));
    assert_eq!(status_grid(&b), 1.0);
}

#[test]
fn a_grid_step_that_is_no_length_above_zero_is_refused() {
    for bad in ["abc", "0", "-1"] {
        let mut b = open(all_kinds());
        b.refused(|b| type_grid(b, bad));
        assert_eq!(status_grid(&b), 1.0, "{bad}");
    }
}

#[test]
fn entering_the_step_the_board_already_has_is_no_edit() {
    let mut b = open(all_kinds());
    b.refused(|b| type_grid(b, "1"));
    assert!(!b.dirty());
}

#[test]
fn a_status_bar_grid_pick_writes_the_boards_step_in_one_undo_step() {
    let mut b = open(all_kinds());
    let edited = b.one_step(|b| pick_from_status_popover(b, 0.5));
    assert_eq!(grid_step_mm(&edited), 0.5);
}

#[test]
fn a_fast_grid_chip_writes_the_boards_step_in_one_undo_step() {
    let mut b = open(all_kinds());
    b.app.ui_state.fast_grids_mm = [0.5, 0.25];
    let edited = b.one_step(|b| b.board_panel(PcbBoardMsg::FastGridApplied(1)));
    assert_eq!(grid_step_mm(&edited), 0.25);
}

/// Adds an extra grid for parts and everything else; its id.
fn add_extra_grid(b: &mut Bench) -> Uuid {
    b.board_panel(PcbBoardMsg::GridAdded);
    b.board().options.grids[0].uuid
}

#[test]
fn adding_an_extra_grid_is_one_undo_step() {
    let mut b = open(all_kinds());
    let edited = b.one_step(|b| b.board_panel(PcbBoardMsg::GridAdded));
    assert_eq!(edited.options.grids.len(), 1);
    assert_eq!(edited.options.grids[0].step_mm, 1.0, "on the board's step");
}

#[test]
fn with_an_extra_grid_in_effect_the_status_bar_reads_and_edits_that_grid() {
    let mut b = open(all_kinds());
    let id = add_extra_grid(&mut b);
    field(&mut b, BoardField::GridStep(id), "0.25");
    let grid = b.app.status_grid(b.main);
    assert!(grid.extra, "the read-out names the extra grid");
    assert_eq!(grid.size_mm, 0.25);

    pick_from_status_popover(&mut b, 0.5);
    let board = b.board();
    assert_eq!(
        board.options.grids[0].step_mm, 0.5,
        "the extra grid took it"
    );
    assert_eq!(grid_step_mm(&board), 1.0, "the board's own step stays");
}

#[test]
fn an_extra_grid_step_of_zero_or_text_is_refused() {
    let mut b = open(all_kinds());
    let id = add_extra_grid(&mut b);
    for bad in ["0", "-2", "abc"] {
        b.refused(|b| field(b, BoardField::GridStep(id), bad));
    }
}

#[test]
fn deleting_an_extra_grid_is_undone_in_one_step() {
    let mut b = open(all_kinds());
    let id = add_extra_grid(&mut b);
    let with_grid = b.json();
    b.board_panel(PcbBoardMsg::GridDeleted(id));
    assert!(b.board().options.grids.is_empty());
    b.undo();
    assert_eq!(b.json(), with_grid);
}

#[test]
fn a_guide_is_added_moved_and_deleted_each_in_one_undo_step() {
    let mut b = open(all_kinds());
    let edited = b.one_step(|b| b.board_panel(PcbBoardMsg::GuideAdded(GuideKind::Vertical)));
    assert_eq!(edited.options.guides.len(), 1);

    b.board_panel(PcbBoardMsg::GuideAdded(GuideKind::Vertical));
    let id = b.board().options.guides[0].uuid;
    let placed = b.json();
    field(&mut b, BoardField::GuideX(id), "12.5");
    assert_eq!(b.board().options.guides[0].at.x, 12.5);
    b.undo();
    assert_eq!(b.json(), placed, "undo puts the guide back");

    b.board_panel(PcbBoardMsg::GuideDeleted(id));
    assert!(b.board().options.guides.is_empty());
    b.undo();
    assert_eq!(b.json(), placed, "undo brings the guide back");
}

#[test]
fn a_guide_position_that_is_not_a_length_is_refused() {
    let mut b = open(all_kinds());
    b.board_panel(PcbBoardMsg::GuideAdded(GuideKind::Horizontal));
    let id = b.board().options.guides[0].uuid;
    b.refused(|b| field(b, BoardField::GuideY(id), "abc"));
}

#[test]
fn the_sheet_size_edits_in_one_step_and_refuses_zero() {
    let mut b = open(all_kinds());
    let edited = b.one_step(|b| field(b, BoardField::SheetWidth, "210"));
    assert_eq!(edited.options.sheet.width, 210.0);
    b.refused(|b| field(b, BoardField::SheetWidth, "0"));
    b.refused(|b| field(b, BoardField::SheetHeight, "-5"));
}

#[test]
fn the_designator_and_pour_naming_options_are_one_undo_step_each() {
    let mut b = open(all_kinds());
    let edited = b.one_step(|b| {
        b.board_panel(PcbBoardMsg::DesignatorDisplayPicked(
            DesignatorDisplay::Logical,
        ));
    });
    assert_eq!(
        edited.options.designator_display,
        DesignatorDisplay::Logical
    );
    let edited = b.one_step(|b| {
        b.board_panel(PcbBoardMsg::PolygonNamingPicked(
            PolygonNaming::LayerNetPour,
        ));
    });
    assert_eq!(edited.options.polygon_naming, PolygonNaming::LayerNetPour);
}

#[test]
fn picking_the_naming_a_board_already_has_is_no_edit() {
    let mut b = open(all_kinds());
    b.refused(|b| {
        b.board_panel(PcbBoardMsg::DesignatorDisplayPicked(
            DesignatorDisplay::Physical,
        ));
    });
}

#[test]
fn a_kind_turned_off_in_the_selection_filter_is_not_picked_and_the_board_stays_clean() {
    let mut b = open(all_kinds());
    b.view_only(|b| b.board_panel(PcbBoardMsg::FilterKindToggled(PickKind::Tracks)));
    b.click(5.0, 0.0);
    let selection = b.app.interaction_state.pcb_selection.selection();
    assert!(selection.is_empty(), "the track is filtered out");

    b.board_panel(PcbBoardMsg::FilterAllSet(true));
    b.click(5.0, 0.0);
    let selection = b.app.interaction_state.pcb_selection.selection();
    assert_eq!(selection.len(), 1, "All - On picks it again");
}

#[test]
fn a_components_count_selects_the_components_of_that_side() {
    let mut b = open(all_kinds());
    b.view_only(|b| b.board_panel(PcbBoardMsg::ComponentsSelected(ComponentSide::Bottom)));
    assert!(b.app.interaction_state.pcb_selection.selection().is_empty());
    b.board_panel(PcbBoardMsg::ComponentsSelected(ComponentSide::Top));
    assert_eq!(b.app.interaction_state.pcb_selection.selection().len(), 1);
}

#[test]
fn edit_board_shape_opens_board_sketch_and_leaves_the_board_unchanged() {
    let mut b = open(all_kinds());
    b.view_only(|b| b.board_panel(PcbBoardMsg::BoardShapeEditPressed));
    assert!(b.app.interaction_state.board_sketch.is_some());
}

#[test]
fn the_board_materials_link_opens_the_layer_stack() {
    let mut b = open(all_kinds());
    b.view_only(|b| b.board_panel(PcbBoardMsg::LayerStackOpened));
    // Layer Stack is a document tab (DECISIONS 2026-11-18).
    assert!(b.app.document_state.tabs.iter().any(|t| t.kind.is_layer_stack()));
}

#[test]
fn the_health_check_tab_opens_and_the_parameters_tab_stays_closed() {
    let mut b = open(all_kinds());
    b.view_only(|b| b.board_panel(PcbBoardMsg::TabPicked(BoardTab::HealthCheck)));
    assert_eq!(b.app.ui_state.pcb_board_panel.tab, BoardTab::HealthCheck);
    b.board_panel(PcbBoardMsg::TabPicked(BoardTab::Parameters));
    assert_eq!(
        b.app.ui_state.pcb_board_panel.tab,
        BoardTab::HealthCheck,
        "boards hold no parameters yet"
    );
}

#[test]
fn a_board_without_a_panel_shows_no_panel_section() {
    let b = open(all_kinds());
    let summary = b.app.document_state.panel_ctx.pcb_board.as_ref();
    assert!(summary.expect("the board summary").panel.is_none());
}

// ---- Snap Options on a placing path (decision of 2026-11-21) ----

/// A via off the 1 mm grid at (20.03, 5.03), so landing on it can only be
/// its snap.
fn off_grid_via() -> PcbBoard {
    let mut board = all_kinds();
    board.vias[0].position = Point::new(20.03, 5.03);
    board
}

/// Where the Line tool's first click near that via lands. The tool draws on
/// a non-copper layer, so the overlay is made active first.
fn line_start(set_up: impl FnOnce(&mut Bench)) -> Point {
    let mut b = open(off_grid_via());
    set_up(&mut b);
    b.send(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
    ))));
    b.command("place_board_line");
    b.click(20.2, 5.2);
    b.app
        .interaction_state
        .pcb_tool
        .as_ref()
        .and_then(|tool| tool.points.first().copied())
        .expect("the first point is placed")
}

#[test]
fn with_every_snap_object_off_a_tool_point_takes_only_the_grid() {
    assert_at(line_start(|_| {}), 20.0, 5.0);
}

#[test]
fn one_snap_object_on_pulls_a_tool_point_exactly_onto_it() {
    let at = line_start(|b| {
        b.board_panel(PcbBoardMsg::SnapObjectToggled(SnapObject::ViaCenters));
    });
    assert_at(at, 20.03, 5.03);
}

#[test]
fn another_snap_object_does_not_pull_a_tool_point_to_the_via() {
    let at = line_start(|b| {
        b.board_panel(PcbBoardMsg::SnapObjectToggled(SnapObject::PadCenters));
    });
    assert_at(at, 20.0, 5.0);
}

#[test]
fn snapping_off_stops_object_snap_and_leaves_the_grid() {
    let at = line_start(|b| {
        b.board_panel(PcbBoardMsg::SnapObjectToggled(SnapObject::ViaCenters));
        b.board_panel(PcbBoardMsg::SnapScopePicked(SnapScope::Off));
    });
    assert_at(at, 20.0, 5.0);
}

#[test]
fn the_status_bar_snap_switch_off_stops_every_snap() {
    let at = line_start(|b| {
        b.board_panel(PcbBoardMsg::SnapObjectToggled(SnapObject::ViaCenters));
        b.send(Message::Ui(UiMsg::StatusBar(StatusBarRequest::ToggleSnap)));
    });
    assert_at(at, 20.2, 5.2);
}

#[test]
fn snap_options_are_session_settings_and_leave_the_board_clean() {
    let mut b = open(all_kinds());
    b.view_only(|b| {
        b.board_panel(PcbBoardMsg::SnapToGridsToggled);
        b.board_panel(PcbBoardMsg::SnapToGuidesToggled);
        b.board_panel(PcbBoardMsg::SnapToAxesToggled);
        b.board_panel(PcbBoardMsg::SnapObjectToggled(SnapObject::Texts));
        b.board_panel(PcbBoardMsg::SnapScopePicked(SnapScope::CurrentLayer));
    });
    let snap = &b.app.ui_state.pcb_snap;
    assert!(!snap.to_grids && snap.objects.contains(&SnapObject::Texts));
    assert_eq!(snap.scope, SnapScope::CurrentLayer);
}

#[test]
fn an_axis_range_of_zero_or_text_is_refused() {
    let mut b = open(all_kinds());
    let range = b.app.ui_state.pcb_snap.axis_range_mm;
    for bad in ["0", "abc"] {
        b.refused(|b| field(b, BoardField::AxisRange, bad));
        assert_eq!(b.app.ui_state.pcb_snap.axis_range_mm, range);
    }
    field(&mut b, BoardField::AxisRange, "4");
    assert_eq!(b.app.ui_state.pcb_snap.axis_range_mm, 4.0);
    assert!(!b.dirty(), "the range is a session setting");
}
