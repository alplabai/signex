//! #2523 (mockup row 93) - the board Properties panel with nothing selected: Edit
//! board shape runs Board sketch, the board materials link opens the Layer
//! Stack, the size row reads 'Bounding box' for an outline that is not a
//! rectangle, and a hovered tile and its callout light each other.

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::pcb_board_properties::{BOARD_WIDTH, size_readout};
use signex_app::panels::{PanelKind, PanelMsg, PcbBoardMsg};
use signex_types::coord::Unit;
use signex_types::format::SnxPcb;
use signex_types::pcb::Point;
use signex_widgets::property_illustration::IllustrationMsg;

use crate::board_sketch::{Fixture, board, in_sketch, open_board, send};
use crate::drive::drive;

fn properties(f: &mut Fixture, msg: PcbBoardMsg) {
    send(
        f,
        Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg))),
    );
}

/// The fixture board with an L-shaped outline 50 x 40 mm across.
fn open_shaped_board() -> (Signex, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("shaped.snxpcb");
    let mut shaped = board();
    shaped.outline = [
        (0.0, 0.0),
        (50.0, 0.0),
        (50.0, 20.0),
        (25.0, 20.0),
        (25.0, 40.0),
        (0.0, 40.0),
    ]
    .into_iter()
    .map(|(x, y)| Point::new(x, y))
    .collect();
    std::fs::write(&path, SnxPcb::new(shaped).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path))));
    drive(&mut app, task);
    (app, tmp)
}

#[test]
fn edit_board_shape_runs_board_sketch() {
    let mut f = open_board();
    assert!(!in_sketch(&f));
    properties(&mut f, PcbBoardMsg::BoardShapeEditPressed);
    assert!(in_sketch(&f), "Edit board shape… enters Board sketch");
}

#[test]
fn the_board_materials_link_opens_the_layer_stack() {
    let mut f = open_board();
    f.app.document_state.dock.close_panel(PanelKind::LayerStack);
    let dock = |f: &Fixture| f.app.document_state.dock.locate(PanelKind::LayerStack);
    assert!(dock(&f).is_none());
    properties(&mut f, PcbBoardMsg::LayerStackOpened);
    assert!(dock(&f).is_some(), "the link opens the Layer Stack");
}

#[test]
fn a_shaped_outline_reads_as_its_bounding_box() {
    let (mut app, _tmp) = open_shaped_board();
    let task = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(
        PcbBoardMsg::SearchChanged(String::new()),
    ))));
    drive(&mut app, task);
    let summary = app
        .document_state
        .panel_ctx
        .pcb_board
        .clone()
        .expect("the board's Properties");
    assert!(!summary.outline_is_rectangle);
    let shown = size_readout(&app.ui_state.i18n, &summary, Unit::Mm)
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(shown, "Bounding box 50.000 × 40.000 mm");
}

#[test]
fn a_hovered_tile_lights_its_callout_and_the_callout_its_tile() {
    let mut f = open_board();
    let lit = |f: &Fixture| {
        f.app
            .ui_state
            .pcb_board_panel
            .illustration_hover
            .is_field_highlighted(&BOARD_WIDTH)
    };
    // The Width tile reports the field; the width callout is linked to it.
    properties(
        &mut f,
        PcbBoardMsg::IllustrationHovered(IllustrationMsg::FieldHovered(BOARD_WIDTH)),
    );
    assert!(lit(&f));
    properties(
        &mut f,
        PcbBoardMsg::IllustrationHovered(IllustrationMsg::FieldLeft(BOARD_WIDTH)),
    );
    assert!(!lit(&f));
    // The width callout reports the field; the Width tile reads it.
    properties(
        &mut f,
        PcbBoardMsg::IllustrationHovered(IllustrationMsg::CalloutHovered(BOARD_WIDTH)),
    );
    assert!(lit(&f));
    properties(
        &mut f,
        PcbBoardMsg::IllustrationHovered(IllustrationMsg::CalloutLeft),
    );
    assert!(!lit(&f));
}
