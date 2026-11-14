//! #2357: the layer tabs under the board canvas and the layer keys set the
//! active layer, which the status pill names. #2359: one Layers panel on a
//! board tab lists the board's layers and drives the same view.

use std::path::PathBuf;

use iced::Size;
use signex_types::pcb::{LayerDef, PcbBoard, Point, Segment};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use crate::app::{FileMsg, Message, Signex, UiMsg};
use crate::dock::DockMessage;
use crate::keymap::ShortcutTrigger;
use crate::layout_probe::{lay_out_texts, messages_on_click};
use crate::panels::{PanelKind, PanelMsg, view_panel};

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const WINDOW: Size = Size::new(1440.0, 900.0);

fn segment(n: u128, y: f64, layer: &str) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.5,
        layer: layer.to_owned(),
        net: 1,
        unknown_columns: Default::default(),
    }
}

/// Top Layer, Top Overlay, Bottom Layer, in that order, a track on each
/// copper layer.
fn board() -> PcbBoard {
    let layer = |id: u8, name: &str, kind: &str| LayerDef {
        id,
        name: name.to_owned(),
        layer_type: kind.to_owned(),
        display_name: String::new(),
    };
    PcbBoard {
        sketch: None,
        unknown: Default::default(),
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![
            layer(0, "TopCopper", "signal"),
            layer(1, "TopSilkscreen", ""),
            layer(31, "BottomCopper", "signal"),
        ],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![
            segment(1, 0.0, "TopCopper"),
            segment(2, 5.0, "BottomCopper"),
        ],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

fn open() -> (Signex, PathBuf, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = PathBuf::from("/w/tabs.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "tabs".to_string(),
        result: Ok(Box::new(board())),
    }));
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    assert!(app.has_active_pcb());
    (app, path, main)
}

fn press(app: &mut Signex, window: iced::window::Id, keys: &str) {
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("{keys} is a key");
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: Some(window),
        stroke: strokes.remove(0),
    }));
}

fn pill(app: &Signex, window: iced::window::Id) -> Option<String> {
    app.status_active_layer(window)
        .map(|(_, name)| name.into_string())
}

#[test]
fn a_tab_click_sets_the_active_layer_and_the_status_readout() {
    let (mut app, path, main) = open();
    assert_eq!(pill(&app, main).as_deref(), Some("Top Layer"));
    let sent = messages_on_click(app.view(main), WINDOW, "Bottom Layer");
    assert_eq!(sent.len(), 1, "the tab sends one message: {sent:?}");
    for msg in sent {
        let _ = app.update(msg);
    }
    assert_eq!(app.pcb_layer_view(&path).active(), BOTTOM);
    assert_eq!(pill(&app, main).as_deref(), Some("Bottom Layer"));
    // The bottom track now draws last, on top of the rest.
    let snapshot = app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot");
    assert_eq!(snapshot.traces.last().map(|t| t.p0[1]), Some(5.0));

    // A non-copper tab is a layer like any other.
    let sent = messages_on_click(app.view(main), WINDOW, "Top Overlay");
    for msg in sent {
        let _ = app.update(msg);
    }
    assert_eq!(app.pcb_layer_view(&path).active(), OVERLAY);
    assert_eq!(pill(&app, main).as_deref(), Some("Top Overlay"));
}

#[test]
fn plus_steps_to_the_next_layer_and_star_to_the_next_signal_layer() {
    let (mut app, path, main) = open();
    press(&mut app, main, "+");
    assert_eq!(app.pcb_layer_view(&path).active(), OVERLAY, "board order");
    assert_eq!(pill(&app, main).as_deref(), Some("Top Overlay"));
    press(&mut app, main, "+");
    assert_eq!(app.pcb_layer_view(&path).active(), BOTTOM);
    press(&mut app, main, "-");
    assert_eq!(app.pcb_layer_view(&path).active(), OVERLAY);
    press(&mut app, main, "-");
    assert_eq!(app.pcb_layer_view(&path).active(), TOP);
    press(&mut app, main, "*");
    assert_eq!(
        app.pcb_layer_view(&path).active(),
        BOTTOM,
        "the overlay is not a signal layer"
    );
}

#[test]
fn shift_s_toggles_single_layer_mode() {
    let (mut app, path, main) = open();
    press(&mut app, main, "Shift+S");
    assert!(app.pcb_layer_view(&path).single_layer());
    let traces = app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
        .traces
        .len();
    assert_eq!(traces, 1, "only the Top track is drawn");
    assert!(app.document_state.panel_ctx.pcb_layers.single_layer);
    press(&mut app, main, "Shift+S");
    assert!(!app.pcb_layer_view(&path).single_layer());
}

fn panel_send(app: &mut Signex, sent: Vec<PanelMsg>) {
    for msg in sent {
        let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
    }
}

/// #2359: on a board tab the one Layers panel lists the board's layers in
/// groups; a row click sets the active layer, an eye hides the layer from
/// the canvas draw.
#[test]
fn the_layers_panel_on_a_board_tab_lists_and_drives_the_board_layers() {
    let (mut app, path, main) = open();
    let panel = Size::new(300.0, 1800.0);
    let ctx = app.document_state.panel_ctx.clone();
    let texts = lay_out_texts(view_panel(PanelKind::Layers, &ctx), panel);
    for label in [
        "SIGNAL",
        "Top Layer",
        "Bottom Layer",
        "OVERLAY",
        "Top Overlay",
    ] {
        assert!(texts.iter().any(|t| t.text == label), "{label} listed");
    }

    let sent = messages_on_click(view_panel(PanelKind::Layers, &ctx), panel, "Bottom Layer");
    panel_send(&mut app, sent);
    assert_eq!(app.pcb_layer_view(&path).active(), BOTTOM);
    assert_eq!(pill(&app, main).as_deref(), Some("Bottom Layer"));

    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbLayers(
        crate::app::PcbViewMsg::LayerVisibilityToggled(TOP),
    ))));
    let snapshot = app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot");
    assert!(
        snapshot.traces.iter().all(|t| t.p0[1] != 0.0),
        "the hidden Top track is not drawn"
    );
    assert!(app.document_state.panel_ctx.pcb_layers.is_hidden(TOP));
}
