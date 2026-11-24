//! #2374 — the board options through `Signex::update`: guides added, placed,
//! edited and deleted as undo steps and snapped to; axis snapping; an extra
//! grid that governs component moves; the sheet, naming options and fast
//! grids; and the options surviving a save.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_options::{BoardOptions, DesignatorDisplay, GuideKind, PolygonNaming};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, InputMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::{BoardField, PanelMsg, PcbBoardMsg};
use crate::pcb_canvas::PcbCanvasMsg;

const U1: Uuid = Uuid::from_u128(0x10);

/// A 50 x 40 mm board on a 1.27 mm grid: U1 on top at (10, 10) with one SMD
/// pad on its origin, U2 at (30, 30) with one pad on its origin.
fn board() -> PcbBoard {
    let footprint = |id: u128, x: f64, pad: u128| {
        serde_json::json!({
            "uuid": Uuid::from_u128(id), "reference": format!("U{id}"),
            "position": {"x": x, "y": x}, "layer": "TopCopper",
            "pads": [{"uuid": Uuid::from_u128(pad), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"]}]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.27},
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0},
            {"x": 50.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
        "footprints": [footprint(0x10, 10.0, 0x11), footprint(0x20, 30.0, 0x21)]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    // The tour depends on the shared test prefs; open, it takes the Esc.
    app.ui_state.first_run_tour_open = false;
    app.ui_state.unit = signex_types::coord::Unit::Mm;
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn panel(app: &mut Signex, msg: PcbBoardMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg))));
}

/// Type `text` into `field` and press Enter.
fn type_in(app: &mut Signex, field: BoardField, text: &str) {
    panel(app, PcbBoardMsg::FieldTyped(field, text.into()));
    panel(app, PcbBoardMsg::FieldSubmitted(field));
}

fn options(app: &Signex, path: &Path) -> BoardOptions {
    app.document_state
        .pcb_board(path)
        .expect("board")
        .options
        .clone()
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
}

/// Edit ▸ Move U1 from its origin to the pointer at `(x, y)`; where it
/// lands.
fn move_u1_to(app: &mut Signex, path: &Path, x: f64, y: f64) -> Point {
    let command = AppCommandId::new("move_object").expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(command)));
    click(app, 10.0, 10.0);
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::PointerMoved {
        at: Point::new(x, y),
        zoom_pct: 100.0,
        tolerance_mm: 0.05,
    }));
    click(app, x, y);
    let board = app.document_state.pcb_board(path).expect("board");
    board
        .footprints
        .iter()
        .find(|f| f.uuid == U1)
        .unwrap()
        .position
}

fn near(p: Point, x: f64, y: f64) -> bool {
    (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9
}

#[test]
fn a_guide_is_added_edited_and_deleted_as_undo_steps() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(&mut app, PcbBoardMsg::GuideAdded(GuideKind::Vertical));
    let guides = options(&app, &path).guides;
    assert_eq!(guides.len(), 1);
    let id = guides[0].uuid;
    assert_eq!(guides[0].at, Point::ZERO);

    type_in(&mut app, BoardField::GuideX(id), "15");
    assert_eq!(options(&app, &path).guides[0].at.x, 15.0);
    assert!(app.ui_state.pcb_board_panel.drafts.is_empty());
    panel(&mut app, PcbBoardMsg::GuideEnabledToggled(id));
    assert!(!options(&app, &path).guides[0].enabled);
    let color = options(&app, &path).guides[0].color;
    panel(&mut app, PcbBoardMsg::GuideColorStepped(id));
    assert_ne!(options(&app, &path).guides[0].color, color);

    // The canvas draws enabled guides only.
    assert!(
        app.interaction_state
            .pcb_canvas
            .board_marks()
            .guides
            .is_empty()
    );
    undo(&mut app);
    undo(&mut app);
    assert_eq!(
        app.interaction_state.pcb_canvas.board_marks().guides.len(),
        1
    );

    panel(&mut app, PcbBoardMsg::GuideMenuToggled(id));
    assert_eq!(app.ui_state.pcb_board_panel.guide_menu, Some(id));
    panel(&mut app, PcbBoardMsg::GuideDeleted(id));
    assert!(options(&app, &path).guides.is_empty());
    assert_eq!(app.ui_state.pcb_board_panel.guide_menu, None);
    undo(&mut app);
    assert_eq!(options(&app, &path).guides[0].at.x, 15.0);
}

#[test]
fn place_sets_a_guide_where_the_board_is_clicked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(
        &mut app,
        PcbBoardMsg::GuidePlaceArmed(GuideKind::Horizontal),
    );
    // Esc drops the armed guide.
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert!(app.interaction_state.pcb_guide_place.is_none());

    panel(
        &mut app,
        PcbBoardMsg::GuidePlaceArmed(GuideKind::Horizontal),
    );
    // 3.6 / 1.27 = 2.83, so x rounds to 3 steps (3.81); 3.1 rounds to 2.
    click(&mut app, 3.6, 7.4);
    let guides = options(&app, &path).guides;
    assert_eq!(guides.len(), 1);
    assert_eq!(guides[0].kind, GuideKind::Horizontal);
    // On the 1.27 mm grid.
    assert!(near(guides[0].at, 3.81, 7.62), "{:?}", guides[0].at);
    assert!(app.interaction_state.pcb_guide_place.is_none());
    assert!(
        app.interaction_state.pcb_selection.selection().is_empty(),
        "the click placed the guide and picked nothing"
    );
}

#[test]
fn a_guide_pulls_a_move_while_guides_are_on() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(&mut app, PcbBoardMsg::GuideAdded(GuideKind::Vertical));
    let id = options(&app, &path).guides[0].uuid;
    type_in(&mut app, BoardField::GuideX(id), "15");

    let moved = move_u1_to(&mut app, &path, 15.2, 20.5);
    // x onto the guide, y on the 1.27 mm grid.
    assert!(near(moved, 15.0, 20.32), "{moved:?}");
    undo(&mut app);

    panel(&mut app, PcbBoardMsg::SnapToGuidesToggled);
    let moved = move_u1_to(&mut app, &path, 15.2, 20.5);
    assert!(near(moved, 15.24, 20.32), "{moved:?}");
}

/// Caner, 2026-11-24 (#2879): Axes starts on, and every supported object
/// does; Pad Centers alone is kept here so the pad's centre, not one of its
/// corners, is what lines up.
#[test]
fn axes_line_a_move_up_with_a_pad_in_range() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    assert!(app.ui_state.pcb_snap.to_axes, "Axes starts on");
    for object in signex_engine::pcb::snap::SnapObject::DEFAULT_ON {
        if object != signex_engine::pcb::snap::SnapObject::PadCenters {
            panel(&mut app, PcbBoardMsg::SnapObjectToggled(object));
        }
    }
    type_in(&mut app, BoardField::AxisRange, "15");
    assert_eq!(app.ui_state.pcb_snap.axis_range_mm, 15.0);
    // U2's pad is at (30, 30): 0.4 mm off its column, 10 mm away.
    let moved = move_u1_to(&mut app, &path, 30.4, 20.0);
    assert!(near(moved, 30.0, 20.32), "{moved:?}");
    undo(&mut app);

    panel(&mut app, PcbBoardMsg::SnapToAxesToggled);
    let moved = move_u1_to(&mut app, &path, 30.4, 20.0);
    assert!(near(moved, 30.48, 20.32), "{moved:?}");
}

/// Caner, 2026-11-24 (#2879): Snap Distance is a length typed in the
/// board's unit, 1 mm by default; a value not above zero is refused.
#[test]
fn snap_distance_is_a_length_field() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    assert_eq!(app.ui_state.pcb_snap.distance_mm, 1.0);
    assert_eq!(app.ui_state.pcb_snap.axis_range_mm, 2.0);
    type_in(&mut app, BoardField::SnapDistance, "0.25");
    assert_eq!(app.ui_state.pcb_snap.distance_mm, 0.25);
    type_in(&mut app, BoardField::SnapDistance, "0");
    assert_eq!(app.ui_state.pcb_snap.distance_mm, 0.25, "refused");
    app.ui_state.unit = signex_types::coord::Unit::Mil;
    type_in(&mut app, BoardField::SnapDistance, "20");
    assert!((app.ui_state.pcb_snap.distance_mm - 0.508).abs() < 1e-9);
}

#[test]
fn an_extra_grid_governs_component_moves_until_it_stops_applying() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(&mut app, PcbBoardMsg::GridAdded);
    let grid = options(&app, &path).grids[0].clone();
    assert_eq!(grid.step_mm, 1.27, "a new grid starts on the board step");
    assert_eq!(
        app.ui_state.pcb_board_panel.grid_editing,
        Some(crate::panels::GridCard::Extra(grid.uuid)),
        "a new grid opens its fields"
    );
    type_in(&mut app, BoardField::GridStep(grid.uuid), "0.5");
    type_in(&mut app, BoardField::GridOriginX(grid.uuid), "0.1");
    type_in(&mut app, BoardField::GridName(grid.uuid), "Fine");
    let grid = options(&app, &path).grids[0].clone();
    assert_eq!((grid.step_mm, grid.origin.x), (0.5, 0.1));
    assert_eq!(grid.name, "Fine");
    assert_eq!(
        app.interaction_state.pcb_canvas.board_marks().grids.len(),
        1
    );

    let moved = move_u1_to(&mut app, &path, 20.6, 20.6);
    assert!(near(moved, 20.6, 20.5), "{moved:?}");
    undo(&mut app);

    panel(&mut app, PcbBoardMsg::GridComponentsToggled(grid.uuid));
    let moved = move_u1_to(&mut app, &path, 20.6, 20.6);
    assert!(near(moved, 20.32, 20.32), "{moved:?}");

    panel(&mut app, PcbBoardMsg::GridDeleted(grid.uuid));
    assert!(options(&app, &path).grids.is_empty());
    // A step that is not above zero is refused.
    undo(&mut app);
    type_in(&mut app, BoardField::GridStep(grid.uuid), "0");
    assert_eq!(options(&app, &path).grids[0].step_mm, 0.5);
}

#[test]
fn the_sheet_and_naming_options_are_board_data_that_survive_a_save() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(&mut app, PcbBoardMsg::SheetFromBoardToggled);
    let sheet = options(&app, &path).sheet;
    assert!(!sheet.from_board);
    assert_eq!(
        (sheet.origin, sheet.width, sheet.height),
        (Point::ZERO, 50.0, 40.0),
        "the sheet keeps the box around the board"
    );
    type_in(&mut app, BoardField::SheetWidth, "80");
    assert_eq!(options(&app, &path).sheet.width, 80.0);
    assert_eq!(
        app.interaction_state.pcb_canvas.board_marks().sheet,
        Some(([0.0, 0.0], [80.0, 40.0]))
    );
    panel(
        &mut app,
        PcbBoardMsg::PolygonNamingPicked(PolygonNaming::LayerNetPour),
    );
    panel(
        &mut app,
        PcbBoardMsg::DesignatorDisplayPicked(DesignatorDisplay::Logical),
    );
    let saved = options(&app, &path);
    let board = app.document_state.pcb_board(&path).unwrap().clone();
    let text = SnxPcb::new(board).write_string().expect("saves");
    let back = SnxPcb::parse(&text).expect("loads").board;
    assert_eq!(back.options, saved);

    undo(&mut app);
    assert_eq!(
        options(&app, &path).designator_display,
        DesignatorDisplay::Physical
    );
}

#[test]
fn a_fast_grid_button_sets_the_board_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.ui_state.fast_grids_mm = [0.25, 2.5];
    panel(&mut app, PcbBoardMsg::FastGridApplied(1));
    let board = app.document_state.pcb_board(&path).unwrap();
    assert_eq!(board.setup.as_ref().unwrap().grid_size, 2.5);
}

/// #2386: each direction of the Add Guide dropdown adds its guide through
/// the origin as one undo step, and closes the dropdown.
#[test]
fn the_add_guide_dropdown_adds_each_direction_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    for (n, kind) in GuideKind::ALL.into_iter().enumerate() {
        panel(&mut app, PcbBoardMsg::GuideAddMenuToggled);
        assert!(app.ui_state.pcb_board_panel.guide_add_open);
        panel(&mut app, PcbBoardMsg::GuideAdded(kind));
        assert!(!app.ui_state.pcb_board_panel.guide_add_open);
        let guides = options(&app, &path).guides;
        assert_eq!(guides.len(), n + 1);
        assert_eq!(guides[n].kind, kind);
    }
    undo(&mut app);
    assert_eq!(options(&app, &path).guides.len(), 3, "one step per guide");
}

/// #2386: Place on Board turns the dropdown's directions into arming one.
#[test]
fn place_on_board_arms_the_picked_direction() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(&mut app, PcbBoardMsg::GuideAddMenuToggled);
    panel(&mut app, PcbBoardMsg::GuidePlaceModeToggled);
    assert!(app.ui_state.pcb_board_panel.guide_place);
    panel(&mut app, PcbBoardMsg::GuidePlaceArmed(GuideKind::Minus45));
    assert!(options(&app, &path).guides.is_empty(), "nothing added yet");
    assert_eq!(
        app.interaction_state
            .pcb_guide_place
            .as_ref()
            .map(|(_, k)| *k),
        Some(GuideKind::Minus45)
    );
}

/// #2386: a grid is added under the board's own grid (top priority), Move
/// Up swaps it with the one above as one undo step, and its step field
/// edits it as one undo step.
#[test]
fn grid_cards_add_move_and_edit_as_undo_steps() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(&mut app, PcbBoardMsg::GridAdded);
    let first = options(&app, &path).grids[0].uuid;
    panel(&mut app, PcbBoardMsg::GridAdded);
    let second = options(&app, &path).grids[0].uuid;
    assert_ne!(
        first, second,
        "the new grid is first, under the board's own"
    );

    // The top card cannot go up; the second can.
    panel(&mut app, PcbBoardMsg::GridMoved(second, true));
    assert_eq!(options(&app, &path).grids[0].uuid, second);
    panel(&mut app, PcbBoardMsg::GridMoved(first, true));
    let order: Vec<Uuid> = options(&app, &path).grids.iter().map(|g| g.uuid).collect();
    assert_eq!(order, vec![first, second]);
    undo(&mut app);
    let order: Vec<Uuid> = options(&app, &path).grids.iter().map(|g| g.uuid).collect();
    assert_eq!(order, vec![second, first], "one undo step");

    type_in(&mut app, BoardField::GridStep(second), "0.25");
    assert_eq!(options(&app, &path).grids[0].step_mm, 0.25);
    undo(&mut app);
    assert_eq!(options(&app, &path).grids[0].step_mm, 1.27);

    panel(
        &mut app,
        PcbBoardMsg::GridMenuToggled(crate::panels::GridCard::Extra(first)),
    );
    panel(&mut app, PcbBoardMsg::GridDeleted(first));
    assert_eq!(options(&app, &path).grids.len(), 1);
    assert_eq!(app.ui_state.pcb_board_panel.grid_menu, None);
}

/// 2026-11-18: a layer colour is board data: set from the Layers panel's
/// chip, it is one undo step that marks the board changed; Reset all layer
/// colours is another; undo brings each back.
#[test]
fn a_layer_colour_is_saved_with_the_board_as_an_undo_step() {
    use crate::app::PcbViewMsg;
    use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    assert_eq!(app.document_state.dirty_document_count(), 0);
    let set = PcbViewMsg::LayerColourSet {
        layer: silk,
        colour: Some([255, 255, 0]),
    };
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbLayers(set))));
    let yellow = signex_types::theme::Color::new(255, 255, 0, 0xFF);
    assert_eq!(options(&app, &path).layer_color(silk), Some(yellow));
    assert_eq!(app.document_state.dirty_document_count(), 1);
    assert_eq!(
        app.document_state.panel_ctx.pcb_layers.colour(silk),
        iced::Color::from_rgb8(255, 255, 0),
        "the panel reads the board's colour"
    );

    let reset = PcbViewMsg::LayerColoursReset;
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbLayers(
        reset,
    ))));
    assert!(options(&app, &path).layer_colors.is_empty());
    undo(&mut app);
    assert_eq!(options(&app, &path).layer_color(silk), Some(yellow));
    undo(&mut app);
    assert!(options(&app, &path).layer_colors.is_empty());
}
