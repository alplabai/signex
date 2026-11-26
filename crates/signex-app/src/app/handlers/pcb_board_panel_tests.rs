//! #2353 — the board Properties panel through `Signex::update`: a Selection
//! Filter chip changes what a click picks, an Objects-for-snapping checkbox
//! changes where a Move lands, Board Information counts the fixture board,
//! and a Grid Manager edit sets the board's snap grid as one undo step.

use std::path::{Path, PathBuf};

use pcb_geom::{ClickMode, ItemRef, PickKind};
use signex_engine::pcb::snap::SnapObject;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::{ComponentSide, PanelMsg, PcbBoardMsg};
use crate::pcb_canvas::PcbCanvasMsg;

const U1: Uuid = Uuid::from_u128(0x10);
const U2: Uuid = Uuid::from_u128(0x20);
const U1_PAD: Uuid = Uuid::from_u128(0x11);

/// A 50 x 40 mm board on a 1.27 mm grid: U1 on top at (10, 10) with SMD pads
/// at (12, 10) and (8, 10); U2 on the bottom at (30, 30) with one drilled
/// pad; a via at (20.3, 10.4); one track, one zone and one text.
fn board() -> PcbBoard {
    let pad = |id: Uuid, x: f64, drill: Option<f64>, layer: &str| {
        serde_json::json!({
            "uuid": id, "number": "1",
            "pad_type": if drill.is_some() { "thru" } else { "smd" },
            "shape": "rect", "position": {"x": x, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": drill.map(|d| serde_json::json!({"diameter": d})),
            "layers": [layer]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.27},
        "thickness": 1.6,
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0},
            {"x": 50.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "layers": [
            {"id": 0, "name": "TopCopper", "layer_type": "signal"},
            {"id": 31, "name": "BottomCopper", "layer_type": "signal"}
        ],
        "footprints": [
            {"uuid": U1, "reference": "U1", "position": {"x": 10.0, "y": 10.0},
             "layer": "TopCopper",
             "pads": [pad(U1_PAD, 2.0, None, "TopCopper"),
                      pad(Uuid::from_u128(0x12), -2.0, None, "TopCopper")]},
            {"uuid": U2, "reference": "U2", "position": {"x": 30.0, "y": 30.0},
             "layer": "BottomCopper",
             "pads": [pad(Uuid::from_u128(0x21), 0.0, Some(1.0), "BottomCopper")]}
        ],
        "vias": [{"uuid": Uuid::from_u128(0x30), "position": {"x": 20.3, "y": 10.4},
                  "diameter": 0.6, "drill": 0.3}],
        "segments": [{"uuid": Uuid::from_u128(0x40), "start": {"x": 40.0, "y": 5.0},
                      "end": {"x": 45.0, "y": 5.0}, "width": 0.25, "layer": "TopCopper"}],
        "zones": [{"uuid": Uuid::from_u128(0x50), "layer": "TopCopper",
                   "outline": [{"x": 35.0, "y": 20.0}, {"x": 45.0, "y": 20.0},
                               {"x": 45.0, "y": 30.0}]}],
        "texts": [{"uuid": Uuid::from_u128(0x60), "text": "REV A",
                   "position": {"x": 5.0, "y": 35.0}, "layer": "TopSilkscreen"}]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    // The grid step is typed in the panel's unit.
    app.ui_state.unit = signex_types::coord::Unit::Mm;
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn panel(app: &mut Signex, msg: PcbBoardMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg))));
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
}

fn selected(app: &Signex) -> Vec<ItemRef> {
    app.interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

fn position(app: &Signex, path: &Path, id: Uuid) -> Point {
    let board = app.document_state.pcb_board(path).expect("board");
    board
        .footprints
        .iter()
        .find(|f| f.uuid == id)
        .unwrap()
        .position
}

/// Edit ▸ Move U1 from its origin to the pointer at `(x, y)`.
fn move_u1_to(app: &mut Signex, x: f64, y: f64) {
    let command = AppCommandId::new("move_object").expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(command)));
    click(app, 10.0, 10.0);
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::PointerMoved {
        at: Point::new(x, y),
        zoom_pct: 100.0,
        tolerance_mm: 0.05,
    }));
    click(app, x, y);
}

#[test]
fn a_filter_chip_changes_what_a_click_picks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    click(&mut app, 10.0, 10.0);
    assert_eq!(selected(&app), vec![ItemRef::Footprint(U1)]);

    panel(
        &mut app,
        PcbBoardMsg::FilterKindToggled(PickKind::Components),
    );
    click(&mut app, 10.0, 10.0);
    assert!(
        selected(&app).is_empty(),
        "a body is not picked with Components off"
    );
    click(&mut app, 12.0, 10.0);
    let pad = ItemRef::Pad {
        footprint: U1,
        pad: U1_PAD,
    };
    assert_eq!(selected(&app), vec![pad], "pads still pick");

    panel(&mut app, PcbBoardMsg::FilterAllToggled);
    assert!(app.ui_state.pcb_pick_filter.is_all_on());
    click(&mut app, 10.0, 10.0);
    assert_eq!(selected(&app), vec![ItemRef::Footprint(U1)]);
}

#[test]
fn an_objects_for_snapping_checkbox_changes_where_a_move_lands() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    // Via Centers is on: the pointer near the via takes U1's origin onto it.
    move_u1_to(&mut app, 20.5, 10.5);
    let moved = position(&app, &path, U1);
    assert!(
        (moved.x - 20.3).abs() < 1e-9 && (moved.y - 10.4).abs() < 1e-9,
        "{moved:?}"
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));

    panel(
        &mut app,
        PcbBoardMsg::SnapObjectToggled(SnapObject::ViaCenters),
    );
    assert!(
        !app.ui_state
            .pcb_snap
            .objects
            .contains(&SnapObject::ViaCenters)
    );
    move_u1_to(&mut app, 20.5, 10.5);
    let moved = position(&app, &path, U1);
    // On the 1.27 mm grid: (20.5, 10.5) lands on (20.32, 10.16).
    assert!(
        (moved.x - 20.32).abs() < 1e-9 && (moved.y - 10.16).abs() < 1e-9,
        "{moved:?}"
    );
}

#[test]
fn board_information_counts_the_fixture_board_and_selects_by_side() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    app.refresh_panel_ctx();
    let summary = app
        .document_state
        .panel_ctx
        .pcb_board
        .clone()
        .expect("a board summary");
    let stats = &summary.stats;
    assert_eq!(stats.area_mm2, Some(2000.0));
    assert_eq!((stats.components_top, stats.components_bottom), (1, 1));
    assert_eq!(stats.pads, 3);
    assert_eq!((stats.pad_holes, stats.via_holes), (1, 1));
    assert_eq!(
        (stats.tracks, stats.vias, stats.polygons, stats.strings),
        (1, 1, 1, 1)
    );
    assert_eq!((summary.layers, summary.copper_layers), (2, 2));
    assert_eq!(summary.grid_mm, Some(1.27));

    panel(
        &mut app,
        PcbBoardMsg::ComponentsSelected(ComponentSide::Bottom),
    );
    assert_eq!(selected(&app), vec![ItemRef::Footprint(U2)]);
    panel(
        &mut app,
        PcbBoardMsg::ComponentsSelected(ComponentSide::All),
    );
    assert_eq!(selected(&app).len(), 2);
}

#[test]
fn a_grid_manager_edit_sets_the_snap_grid_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let grid = |app: &Signex| {
        let board = app.document_state.pcb_board(&path).expect("board");
        board.setup.as_ref().expect("a setup").grid_size
    };
    panel(&mut app, PcbBoardMsg::GridStepTyped("0.5".into()));
    panel(&mut app, PcbBoardMsg::GridStepSubmitted);
    assert_eq!(grid(&app), 0.5);
    assert_eq!(app.ui_state.pcb_board_panel.grid_draft, None);

    // A step that is not above zero is refused and changes nothing.
    panel(&mut app, PcbBoardMsg::GridStepTyped("0".into()));
    panel(&mut app, PcbBoardMsg::GridStepSubmitted);
    assert_eq!(grid(&app), 0.5);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(grid(&app), 1.27);
}

/// Caner, 2026-11-24 (#2879): on a board Shift+E is its own command,
/// `cycle_board_snapping`, in every built-in profile; it replaces the
/// reroute of `toggle_electrical_grid` (#2353), which now toggles hotspots
/// wherever it runs.
#[test]
fn shift_e_on_a_board_steps_the_snapping_mode() {
    use std::str::FromStr;

    use crate::keymap::{CompiledKeymap, KeyStroke, Platform, ShortcutContext, ShortcutProfileSet};
    use crate::pcb_canvas::SnapScope;
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    let run = |app: &mut Signex, id: &str| {
        let command = AppCommandId::new(id).expect("a catalog id");
        let _ = app.update(Message::Menu(MenuMessage::Command(command)));
    };
    let hotspots = app.ui_state.snap_hotspots;
    run(&mut app, "cycle_board_snapping");
    assert_eq!(app.ui_state.pcb_snap.scope, SnapScope::CurrentLayer);
    run(&mut app, "cycle_board_snapping");
    assert_eq!(app.ui_state.pcb_snap.scope, SnapScope::Off);
    run(&mut app, "cycle_board_snapping");
    assert_eq!(app.ui_state.pcb_snap.scope, SnapScope::AllLayers);
    assert_eq!(
        app.ui_state.snap_hotspots, hotspots,
        "the schematic setting stays"
    );
    run(&mut app, "toggle_electrical_grid");
    assert_eq!(app.ui_state.pcb_snap.scope, SnapScope::AllLayers);
    assert_eq!(app.ui_state.snap_hotspots, !hotspots);

    let on_a_board = [ShortcutContext::Global, ShortcutContext::Pcb];
    let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    for profile in set.profiles() {
        let keymap = CompiledKeymap::compile_for(profile, Platform::Windows);
        let shift_e = KeyStroke::from_str("Shift+E").expect("a key");
        let got = keymap.lookup(&[shift_e], &on_a_board).command;
        assert_eq!(
            got.as_ref().map(AppCommandId::as_str),
            Some("cycle_board_snapping"),
            "{}",
            profile.id
        );
    }
}

/// #2523: a hovered field row lights its callout through the panel's own
/// state; a late leave of another field does not clear a hover that moved on.
#[test]
fn a_hovered_field_is_panel_state_the_summary_carries() {
    use crate::panels::pcb_board_properties::{BOARD_HEIGHT, BOARD_WIDTH};
    use signex_widgets::property_illustration::IllustrationMsg as M;
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    let hover = |app: &Signex| app.ui_state.pcb_board_panel.illustration_hover.clone();

    panel(
        &mut app,
        PcbBoardMsg::IllustrationHovered(M::FieldHovered(BOARD_WIDTH)),
    );
    assert!(hover(&app).is_field_highlighted(&BOARD_WIDTH));
    app.refresh_panel_ctx();
    let summary = app
        .document_state
        .panel_ctx
        .pcb_board
        .clone()
        .expect("a board summary");
    assert!(
        summary
            .view
            .illustration_hover
            .is_field_highlighted(&BOARD_WIDTH),
        "the view draws from the summary"
    );

    panel(
        &mut app,
        PcbBoardMsg::IllustrationHovered(M::FieldLeft(BOARD_HEIGHT)),
    );
    assert!(hover(&app).is_field_highlighted(&BOARD_WIDTH));
    panel(
        &mut app,
        PcbBoardMsg::IllustrationHovered(M::FieldLeft(BOARD_WIDTH)),
    );
    assert_eq!(hover(&app).hovered(), None);
}
