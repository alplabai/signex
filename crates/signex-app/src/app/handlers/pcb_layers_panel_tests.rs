//! mockup row 103: what the Layers panel's own controls do to the board view: a
//! section's eye, a typed hex colour, and the more menu's Layer Stack....

use std::path::PathBuf;

use iced::{Point, Size};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use crate::app::{FileMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::layout_probe::{lay_out_texts, messages_on_click_at};
use crate::panels::{LayersPanelMsg, PanelKind, PanelMsg, view_panel};

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const PANEL: Size = Size::new(320.0, 2600.0);

/// Top and Bottom copper and the top silkscreen.
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 1, "name": "TopSilkscreen", "layer_type": "" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" }
          ],
          "setup": null, "nets": [], "footprints": [], "segments": [],
          "vias": [], "zones": [], "graphics": [], "texts": []
        }"#,
    )
    .expect("valid board")
}

fn open() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = PathBuf::from("/w/layers.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "layers".to_string(),
        result: Ok(Box::new(board())),
    }));
    assert!(app.has_active_pcb());
    (app, path)
}

fn send(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn layers(app: &mut Signex, msg: LayersPanelMsg) {
    send(app, PanelMsg::LayersPanel(msg));
}

/// What a click on the eye of the section titled `title` sends.
fn section_eye(app: &Signex, title: &str) -> Vec<PanelMsg> {
    let ctx = app.document_state.panel_ctx.clone();
    let texts = lay_out_texts(view_panel(PanelKind::Layers, &ctx), PANEL);
    let head = texts
        .iter()
        .find(|t| t.text == title)
        .unwrap_or_else(|| panic!("no section {title}"))
        .bounds;
    let mut x = head.x + head.width;
    let mut sent = Vec::new();
    while x < PANEL.width {
        let at = Point::new(x, head.center_y());
        sent.extend(
            messages_on_click_at(view_panel(PanelKind::Layers, &ctx), PANEL, None, at)
                .into_iter()
                .filter(|m| {
                    matches!(
                        m,
                        PanelMsg::LayersPanel(LayersPanelMsg::SectionVisibilitySet { .. })
                    )
                }),
        );
        if !sent.is_empty() {
            break;
        }
        x += 1.0;
    }
    sent
}

/// mockup row 103: the eye of Signal and Plane Layers hides every copper layer, and
/// a second click shows them again.
#[test]
fn a_section_eye_hides_and_shows_its_layers() {
    let (mut app, path) = open();
    let hide = section_eye(&app, "SIGNAL AND PLANE LAYERS");
    assert!(
        matches!(
            hide.as_slice(),
            [PanelMsg::LayersPanel(
                LayersPanelMsg::SectionVisibilitySet { visible: false, .. }
            )]
        ),
        "{hide:?}"
    );
    for msg in hide {
        send(&mut app, msg);
    }
    let view = app.pcb_layer_view(&path);
    assert!(view.is_hidden(TOP) && view.is_hidden(BOTTOM));
    assert!(
        !view.is_hidden(PcbLayer::Technical(TechnicalLayer::SilkscreenTop)),
        "only the section's layers"
    );

    let show = section_eye(&app, "SIGNAL AND PLANE LAYERS");
    for msg in show {
        send(&mut app, msg);
    }
    let view = app.pcb_layer_view(&path);
    assert!(!view.is_hidden(TOP) && !view.is_hidden(BOTTOM));
}

/// The eye of Other Layers hides Drill Drawing; the Multi-Layer row (vias
/// and through-hole pads) is no layer and keeps its own eye.
#[test]
fn the_other_layers_eye_hides_drill_drawing_only() {
    let (mut app, path) = open();
    for msg in section_eye(&app, "OTHER LAYERS") {
        send(&mut app, msg);
    }
    let view = app.pcb_layer_view(&path);
    assert!(view.is_hidden(PcbLayer::Technical(TechnicalLayer::DrillDrawing)));
    assert!(view.multi_layer_visible());
}

/// A section's eye acts on the rows the panel lists: with a search that
/// leaves only Bottom Layer, it hides Bottom and leaves Top alone.
#[test]
fn a_section_eye_leaves_the_rows_the_search_hides_alone() {
    let (mut app, path) = open();
    layers(
        &mut app,
        LayersPanelMsg::SearchChanged("Bottom".to_string()),
    );
    let hide = section_eye(&app, "SIGNAL AND PLANE LAYERS");
    assert_eq!(hide.len(), 1, "{hide:?}");
    for msg in hide {
        send(&mut app, msg);
    }
    let view = app.pcb_layer_view(&path);
    assert!(view.is_hidden(BOTTOM));
    assert!(
        !view.is_hidden(TOP),
        "Top is not listed: its eye stays open"
    );
}

/// mockup row 103: a dot opens the colour card with the layer's colour in the hex
/// field; a typed colour recolours the layer at once and the card stays
/// open; a half-typed one changes nothing.
#[test]
fn a_typed_hex_colour_recolours_the_layer_and_keeps_the_card_open() {
    let (mut app, path) = open();
    layers(&mut app, LayersPanelMsg::ColourChipToggled(BOTTOM));
    let ui = &app.document_state.panel_ctx.pcb_layers_ui;
    assert_eq!(ui.picker, Some(BOTTOM));
    let start = app.document_state.panel_ctx.pcb_layers.hex_of(BOTTOM);
    assert_eq!(ui.hex, start, "the field starts at the layer's colour");
    assert!(start.starts_with('#') && start.len() == 7, "{start}");

    // Layer colours are saved with the board (2026-11-18).
    let board_colour = |app: &Signex| {
        app.document_state
            .pcb_board(&path)
            .and_then(|board| board.options.layer_color(BOTTOM))
            .map(|c| [c.r, c.g, c.b])
    };
    layers(&mut app, LayersPanelMsg::HexEdited("#FF00".to_string()));
    assert_eq!(board_colour(&app), None);

    layers(&mut app, LayersPanelMsg::HexEdited("#FF00FF".to_string()));
    assert_eq!(board_colour(&app), Some([255, 0, 255]));
    let ui = &app.document_state.panel_ctx.pcb_layers_ui;
    assert_eq!(ui.picker, Some(BOTTOM), "the card stays open");
    assert_eq!(ui.hex, "#FF00FF");

    // A swatch closes the card, as a picked colour did before.
    send(
        &mut app,
        PanelMsg::PcbLayers(crate::app::PcbViewMsg::LayerColourSet {
            layer: BOTTOM,
            colour: Some([1, 2, 3]),
        }),
    );
    assert_eq!(app.document_state.panel_ctx.pcb_layers_ui.picker, None);
}

/// mockup row 103: Layer Stack... in the more menu runs the Layer Stack command,
/// which is live on a board.
#[test]
fn layer_stack_in_the_more_menu_opens_the_layer_stack() {
    let (mut app, _path) = open();
    assert!(app.document_state.panel_ctx.pcb_layers.layer_stack_live);
    layers(&mut app, LayersPanelMsg::MoreMenuToggled);
    layers(&mut app, LayersPanelMsg::LayerStackPicked);
    assert!(app.is_panel_open(PanelKind::LayerStack));
    assert!(!app.document_state.panel_ctx.pcb_layers_ui.more_menu_open);
}

/// View From Bottom Side in the more menu flips the view and closes the
/// menu, as the menu's other live rows do.
#[test]
fn view_from_bottom_side_flips_the_view_and_closes_the_menu() {
    let (mut app, path) = open();
    assert!(!app.pcb_layer_view(&path).flipped());
    layers(&mut app, LayersPanelMsg::MoreMenuToggled);
    layers(&mut app, LayersPanelMsg::BoardFlipPicked);
    assert!(app.pcb_layer_view(&path).flipped());
    assert!(!app.document_state.panel_ctx.pcb_layers_ui.more_menu_open);
    assert!(
        app.document_state.panel_ctx.pcb_layers.flipped,
        "the tick follows"
    );
}
