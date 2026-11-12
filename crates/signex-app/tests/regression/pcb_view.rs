//! #2223 - the board view: ratsnest on and off, layer visibility and the
//! active layer, driven through the menu-row message and the keyboard on a
//! board opened from disk. None of it reaches the file.

use signex_app::app::{FileMsg, Message, PcbMsg, PcbViewMsg, Signex, UiMsg};
use signex_app::keymap::ShortcutTrigger;
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    Footprint, LayerDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment,
};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use crate::drive::drive;
use crate::menu_support::menu_row;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn layer(id: u8, name: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    }
}

fn pad(n: u128, x: f64) -> Pad {
    Pad {
        uuid: Uuid::from_u128(n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: pt(x, 20.0),
        size: pt(1.0, 1.0),
        drill: None,
        // A real SMD pad: copper, mask and paste. It goes with its copper.
        layers: vec![
            "TopCopper".into(),
            "TopSolderMask".into(),
            "TopPaste".into(),
        ],
        net: Some(PadNet {
            number: 1,
            name: "N1".into(),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn segment(n: u128, y: f64, layer: &str) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: pt(0.0, y),
        end: pt(10.0, y),
        width: 0.5,
        layer: layer.to_owned(),
        net: 2,
        unknown_columns: Default::default(),
    }
}

/// A top track at y 0, an inner track at y 5, a bottom track at y 10, and two
/// pads of one net far from them (one ratsnest line).
fn board() -> PcbBoard {
    let footprint = Footprint {
        uuid: Uuid::from_u128(100),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: pt(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        pads: vec![pad(1, 0.0), pad(2, 8.0)],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![
            layer(0, "TopCopper"),
            layer(1, "Inner1Copper"),
            layer(31, "BottomCopper"),
        ],
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint],
        segments: vec![
            segment(10, 0.0, "TopCopper"),
            segment(11, 5.0, "Inner1Copper"),
            segment(12, 10.0, "BottomCopper"),
        ],
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    saved: Vec<u8>,
    _tmp: tempfile::TempDir,
}

fn open_board() -> Fixture {
    open(board())
}

fn open(board: PcbBoard) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board).write_string().unwrap()).unwrap();
    let saved = std::fs::read(&path).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    drive(&mut app, task);
    Fixture {
        app,
        path,
        saved,
        _tmp: tmp,
    }
}

fn run(f: &mut Fixture, command: &str) {
    let task = f.app.update(menu_row(command));
    drive(&mut f.app, task);
}

fn press(f: &mut Fixture, keys: &str) {
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("{keys} is a key, not a gesture");
    };
    let task = f.app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: strokes
            .remove(0)
            .resolved_for(signex_app::keymap::Platform::current()),
    }));
    drive(&mut f.app, task);
}

fn view(f: &Fixture) -> signex_renderer::pcb::PcbLayerView {
    f.app
        .ui_state
        .pcb_layer_views
        .get(&f.path)
        .cloned()
        .unwrap_or_default()
}

fn snapshot(f: &Fixture) -> &signex_renderer::pcb::PcbSnapshot {
    f.app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
}

fn pick(f: &mut Fixture, y: f64) -> bool {
    let msg = signex_app::pcb_canvas::PcbCanvasMsg::Clicked {
        at: pt(5.0, y),
        tolerance_mm: 0.1,
        mode: pcb_geom::ClickMode::Replace,
    };
    let task = f.app.update(Message::PcbCanvas(msg));
    drive(&mut f.app, task);
    !f.app.interaction_state.pcb_selection.selection().is_empty()
}

#[test]
fn toggle_ratsnest_hides_and_shows_the_lines() {
    let mut f = open_board();
    assert_eq!(snapshot(&f).ratsnest_lines.len(), 1);
    run(&mut f, "toggle_ratsnest");
    assert!(snapshot(&f).ratsnest_lines.is_empty());
    assert!(!view(&f).ratsnest_visible());
    run(&mut f, "toggle_ratsnest");
    assert_eq!(snapshot(&f).ratsnest_lines.len(), 1);
}

#[test]
fn next_and_prev_layer_step_through_the_boards_copper_and_wrap() {
    let mut f = open_board();
    assert_eq!(view(&f).active(), CopperLayer::Top);
    run(&mut f, "next_layer");
    assert!(matches!(view(&f).active(), CopperLayer::Inner(_)));
    run(&mut f, "next_layer");
    assert_eq!(view(&f).active(), CopperLayer::Bottom);
    run(&mut f, "next_layer");
    assert_eq!(view(&f).active(), CopperLayer::Top, "wraps forward");
    run(&mut f, "prev_layer");
    assert_eq!(view(&f).active(), CopperLayer::Bottom, "wraps back");
}

#[test]
fn the_layer_keys_step_the_active_layer_on_a_pcb_tab() {
    let mut f = open_board();
    press(&mut f, "-");
    assert_eq!(view(&f).active(), CopperLayer::Bottom);
    press(&mut f, "+");
    assert_eq!(view(&f).active(), CopperLayer::Top);
    press(&mut f, "Shift++");
    assert!(matches!(view(&f).active(), CopperLayer::Inner(_)));
    press(&mut f, "*");
    assert_eq!(view(&f).active(), CopperLayer::Bottom);
    press(&mut f, "Shift+*");
    assert_eq!(view(&f).active(), CopperLayer::Top);
}

#[test]
fn the_active_layer_is_drawn_last_and_the_rest_are_dimmed() {
    let mut f = open_board();
    let order = |f: &Fixture| -> Vec<(u32, bool)> {
        snapshot(f)
            .traces
            .iter()
            .map(|t| (t.net, t.dimmed))
            .collect()
    };
    assert_eq!(snapshot(&f).traces.len(), 3);
    assert_eq!(snapshot(&f).traces.iter().filter(|t| !t.dimmed).count(), 1);
    // The last trace drawn is on the active layer: y 0 while Top is active.
    assert!(!snapshot(&f).traces.last().unwrap().dimmed);
    assert_eq!(snapshot(&f).traces.last().unwrap().p0[1], 0.0);
    run(&mut f, "prev_layer");
    assert_eq!(snapshot(&f).traces.last().unwrap().p0[1], 10.0);
    assert_eq!(order(&f).iter().filter(|(_, dimmed)| *dimmed).count(), 2);
}

#[test]
fn a_hidden_layer_is_not_drawn_and_not_selectable() {
    let mut f = open_board();
    assert!(pick(&mut f, 0.0), "the top track can be picked");
    // Hide the active (top) layer.
    run(&mut f, "toggle_layer_visibility");
    assert_eq!(snapshot(&f).traces.len(), 2);
    assert!(
        f.app.interaction_state.pcb_selection.selection().is_empty(),
        "hiding drops the selection"
    );
    assert!(!pick(&mut f, 0.0), "the hidden top track is not hit");
    assert!(pick(&mut f, 10.0), "a shown layer's track still is");
    // Pads sit on the hidden layer too.
    assert!(snapshot(&f).pads.is_empty());

    run(&mut f, "show_all_layers");
    assert_eq!(snapshot(&f).traces.len(), 3);
    assert_eq!(snapshot(&f).pads.len(), 2);
    assert!(pick(&mut f, 0.0));
}

#[test]
fn the_view_is_kept_per_board_and_never_written_to_the_file() {
    let mut f = open_board();
    let revision = f.app.document_state.pcb_revisions.get(&f.path).copied();
    run(&mut f, "toggle_ratsnest");
    run(&mut f, "next_layer");
    run(&mut f, "toggle_layer_visibility");
    assert_eq!(std::fs::read(&f.path).unwrap(), f.saved);
    assert_eq!(
        f.app.document_state.pcb_revisions.get(&f.path).copied(),
        revision,
        "the board itself did not change"
    );
    assert_eq!(f.app.ui_state.pcb_layer_views.len(), 1);
}

#[test]
fn the_layers_panel_follows_the_view() {
    let mut f = open_board();
    let panel = &f.app.document_state.panel_ctx.pcb_layers;
    assert_eq!(panel.active, Some(CopperLayer::Top));
    assert!(panel.hidden.is_empty());
    // Hiding the active Top layer moves the active layer to Mid-Layer 1; the
    // next step skips the hidden Top and lands on Bottom.
    run(&mut f, "toggle_layer_visibility");
    let panel = &f.app.document_state.panel_ctx.pcb_layers;
    assert!(matches!(panel.active, Some(CopperLayer::Inner(_))));
    run(&mut f, "next_layer");
    let panel = &f.app.document_state.panel_ctx.pcb_layers;
    assert_eq!(panel.active, Some(CopperLayer::Bottom));
    assert_eq!(panel.hidden.len(), 1);
}

#[test]
fn the_board_view_commands_are_off_without_a_pcb() {
    let (mut app, _boot) = Signex::new();
    for command in [
        "toggle_ratsnest",
        "next_layer",
        "prev_layer",
        "show_all_layers",
    ] {
        let task = app.update(menu_row(command));
        drive(&mut app, task);
    }
    assert!(app.ui_state.pcb_layer_views.is_empty());
}

/// The Board Outline row's eye hides the drawn board edge and shows it
/// again; the `.snxpcb` keeps its `EdgeCuts` token and is never written.
#[test]
fn the_board_outline_row_hides_and_shows_the_board_edge() {
    let mut board = board();
    board.outline = vec![
        pt(-5.0, -5.0),
        pt(20.0, -5.0),
        pt(20.0, 15.0),
        pt(-5.0, 15.0),
    ];
    let mut f = open(board);
    let toggle = |f: &mut Fixture| {
        let task = f.app.update(Message::Pcb(PcbMsg::View(
            PcbViewMsg::LayerVisibilityToggled(signex_renderer::pcb::BOARD_OUTLINE),
        )));
        drive(&mut f.app, task);
    };
    assert!(!snapshot(&f).board_features.edge_hidden);
    assert_eq!(snapshot(&f).board_features.outline.len(), 4);
    toggle(&mut f);
    assert!(
        snapshot(&f).board_features.edge_hidden,
        "the edge is hidden"
    );
    assert!(
        f.app
            .document_state
            .panel_ctx
            .pcb_layers
            .hidden
            .contains(&signex_renderer::pcb::BOARD_OUTLINE)
    );
    toggle(&mut f);
    assert!(!snapshot(&f).board_features.edge_hidden, "the edge is back");
    assert_eq!(std::fs::read(&f.path).unwrap(), f.saved);
}

/// #2262: the Layers panel's Multi-Layer eye hides vias and through-hole
/// pads: gone from the render snapshot, not hit by a click, back with Show
/// all layers. It is view state: Save writes the board as it was.
#[test]
fn hiding_the_multi_layer_hides_vias_and_through_hole_pads() {
    let mut board = board();
    board.vias.push(signex_types::pcb::Via {
        uuid: Uuid::from_u128(20),
        position: pt(5.0, 30.0),
        diameter: 0.8,
        drill: 0.4,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 2,
        via_type: signex_types::pcb::ViaType::Through,
        unknown_columns: Default::default(),
    });
    let mut tht = pad(3, 5.0);
    tht.pad_type = PadType::Thru;
    tht.position = pt(5.0, 40.0);
    tht.size = pt(1.8, 1.8);
    tht.shape = PadShape::Circle;
    tht.drill = Some(signex_types::pcb::DrillDef {
        diameter: 1.0,
        shape: "circle".into(),
    });
    tht.layers = vec!["TopCopper".into(), "BottomCopper".into()];
    board.footprints[0].pads.push(tht);
    let mut f = open(board);
    let revision = f.app.document_state.pcb_revisions.get(&f.path).copied();
    let toggle = |f: &mut Fixture| {
        let task = f.app.update(Message::Pcb(PcbMsg::View(
            PcbViewMsg::MultiLayerVisibilityToggled,
        )));
        drive(&mut f.app, task);
    };
    assert_eq!(snapshot(&f).vias.len(), 1);
    assert_eq!(snapshot(&f).pads.len(), 3);
    assert!(picks_a_via(&mut f, 30.0), "the via can be picked");
    assert!(
        picks_a_pad(&mut f, 40.0),
        "the through-hole pad can be picked"
    );

    toggle(&mut f);
    assert!(snapshot(&f).vias.is_empty(), "the via is not drawn");
    assert_eq!(snapshot(&f).pads.len(), 2, "only the SMD pads are drawn");
    assert!(!picks_a_via(&mut f, 30.0), "the hidden via is not hit");
    assert!(
        !picks_a_pad(&mut f, 40.0),
        "the hidden through-hole pad is not hit"
    );
    let panel = &f.app.document_state.panel_ctx.pcb_layers;
    assert!(panel.multi_layer_hidden && panel.has_hidden());

    run(&mut f, "save_document");
    assert_eq!(std::fs::read(&f.path).unwrap(), f.saved);
    assert_eq!(
        f.app.document_state.pcb_revisions.get(&f.path).copied(),
        revision,
        "the board itself did not change"
    );

    run(&mut f, "show_all_layers");
    assert_eq!(snapshot(&f).vias.len(), 1);
    assert_eq!(snapshot(&f).pads.len(), 3);
    assert!(picks_a_via(&mut f, 30.0));
}

/// A click at x 5 mm, `y` picks a pad. The footprint body around a hidden
/// pad stays shown, so a click there may still pick the footprint.
fn picks_a_pad(f: &mut Fixture, y: f64) -> bool {
    picks(f, y, |item| matches!(item, pcb_geom::ItemRef::Pad { .. }))
}

/// A click at x 5 mm, `y` picks a via. The via in the Multi-Layer test sits
/// inside the footprint body, so a click there may still pick the footprint.
fn picks_a_via(f: &mut Fixture, y: f64) -> bool {
    picks(f, y, |item| matches!(item, pcb_geom::ItemRef::Via(_)))
}

fn picks(f: &mut Fixture, y: f64, kind: impl Fn(&pcb_geom::ItemRef) -> bool) -> bool {
    pick(f, y);
    f.app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .any(|item| kind(&item))
}
