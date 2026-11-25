//! 2026-11-20: the Layers panel's colour card and hex field write the
//! board's layer colours (the 2026-11-18 board-saved path): each pick is
//! one undo step that marks the board changed, nothing lands in the view
//! state, and the colour survives a write and a read of the board.

use std::path::{Path, PathBuf};

use iced::Size;
use signex_renderer::pcb::PcbLayerView;
use signex_types::format::{SNXPCB_FORMAT_LAYER_COLORS, SnxPcb};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::theme::Color;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, PcbViewMsg, Signex};
use crate::dock::DockMessage;
use crate::layout_probe::messages_on_click;
use crate::panels::{LayersPanelMsg, PanelKind, PanelMsg, PcbLayersPanel, view_panel};

const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const PANEL: Size = Size::new(320.0, 2600.0);
const MAGENTA: Color = Color::new(255, 0, 255, 0xFF);

/// Top and Bottom copper and the top silkscreen.
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "4b0c3e57-21a8-4f0e-9d0b-6a2f1c7e9b11", "version": 1,
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

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    let path = dir.join("colours.snxpcb");
    app.open_pcb_tab(path.clone(), "colours".into(), board());
    assert!(app.has_active_pcb());
    (app, path)
}

fn send(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn layers(app: &mut Signex, msg: LayersPanelMsg) {
    send(app, PanelMsg::LayersPanel(msg));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

/// The colour the board itself holds for `layer`.
fn board_colour(app: &Signex, path: &Path, layer: PcbLayer) -> Option<Color> {
    app.document_state
        .pcb_board(path)
        .expect("board")
        .options
        .layer_color(layer)
}

/// A colour typed in the hex field is the board's: one undo step, the board
/// marked changed, the view state left alone, and the card reads it back.
#[test]
fn a_typed_hex_colour_is_a_board_colour_and_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    assert!(!app.document_state.is_document_dirty(&path));

    layers(&mut app, LayersPanelMsg::ColourChipToggled(BOTTOM));
    // A half-typed colour neither sets a colour nor marks the board.
    layers(&mut app, LayersPanelMsg::HexEdited("#FF00".to_string()));
    assert_eq!(board_colour(&app, &path, BOTTOM), None);
    assert!(!app.document_state.is_document_dirty(&path));

    layers(&mut app, LayersPanelMsg::HexEdited("#FF00FF".to_string()));
    assert_eq!(board_colour(&app, &path, BOTTOM), Some(MAGENTA));
    assert!(
        app.document_state.is_document_dirty(&path),
        "a colour marks the board changed"
    );
    assert_eq!(
        app.pcb_layer_view(&path).paint().colour(BOTTOM),
        None,
        "the view state holds no colour"
    );
    let panel = &app.document_state.panel_ctx.pcb_layers;
    assert_eq!(panel.colour(BOTTOM), iced::Color::from_rgb8(255, 0, 255));
    assert_eq!(panel.hex_of(BOTTOM), "#FF00FF");

    undo(&mut app);
    assert_eq!(
        board_colour(&app, &path, BOTTOM),
        None,
        "one undo takes the typed colour back"
    );
}

/// A palette swatch sets the board's colour and Palette colour, clicked in
/// the card, drops it again; each is one undo step on the board.
#[test]
fn a_swatch_and_palette_colour_are_board_undo_steps() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());

    layers(&mut app, LayersPanelMsg::ColourChipToggled(BOTTOM));
    send(
        &mut app,
        PanelMsg::PcbLayers(PcbViewMsg::LayerColourSet {
            layer: BOTTOM,
            colour: Some([255, 0, 255]),
        }),
    );
    assert_eq!(board_colour(&app, &path, BOTTOM), Some(MAGENTA));
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(app.pcb_layer_view(&path).paint().colour(BOTTOM), None);

    // The swatch closed the card; open it again for Palette colour.
    layers(&mut app, LayersPanelMsg::ColourChipToggled(BOTTOM));
    let ctx = app.document_state.panel_ctx.clone();
    let sent = messages_on_click(view_panel(PanelKind::Layers, &ctx), PANEL, "Palette color");
    assert!(
        matches!(
            sent.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::LayerColourSet {
                layer,
                colour: None
            })] if *layer == BOTTOM
        ),
        "{sent:?}"
    );
    for msg in sent {
        send(&mut app, msg);
    }
    assert_eq!(board_colour(&app, &path, BOTTOM), None);

    undo(&mut app);
    assert_eq!(
        board_colour(&app, &path, BOTTOM),
        Some(MAGENTA),
        "one undo brings the swatch colour back"
    );
    undo(&mut app);
    assert_eq!(board_colour(&app, &path, BOTTOM), None);
}

/// A colour typed in the card survives a write and a read of the board:
/// the file carries it (and the colours token), and the panel of the board
/// read back shows it.
#[test]
fn a_typed_colour_survives_a_write_and_a_read_of_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    layers(&mut app, LayersPanelMsg::ColourChipToggled(BOTTOM));
    layers(&mut app, LayersPanelMsg::HexEdited("#FF00FF".to_string()));

    let board = app.document_state.pcb_board(&path).unwrap().clone();
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert!(
        text.starts_with(&format!("format = \"{SNXPCB_FORMAT_LAYER_COLORS}\"")),
        "{text}"
    );
    std::fs::write(&path, &text).expect("written");
    let read = std::fs::read_to_string(&path).expect("read");
    let back = SnxPcb::parse(&read).expect("reads back").board;
    assert_eq!(back.options.layer_color(BOTTOM), Some(MAGENTA));

    let panel = PcbLayersPanel::new(&back, &PcbLayerView::default(), app.ui_state.i18n.clone());
    assert_eq!(panel.colour(BOTTOM), iced::Color::from_rgb8(255, 0, 255));
    assert_eq!(panel.hex_of(BOTTOM), "#FF00FF");
}

fn redo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Redo));
}

/// The text of the open card's hex field.
fn hex_field(app: &Signex) -> String {
    app.document_state.panel_ctx.pcb_layers_ui.hex.clone()
}

/// An undo or a redo while the card is open puts the layer's colour back in
/// the hex field, so the next keystroke never applies the colour the field
/// held before the undo as a new undo step.
#[test]
fn an_undo_or_redo_with_the_card_open_rereads_the_hex_field() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    layers(&mut app, LayersPanelMsg::ColourChipToggled(BOTTOM));
    let palette = hex_field(&app);
    layers(&mut app, LayersPanelMsg::HexEdited("#FF00FF".to_string()));
    assert_eq!(board_colour(&app, &path, BOTTOM), Some(MAGENTA));

    undo(&mut app);
    assert_eq!(board_colour(&app, &path, BOTTOM), None);
    let ui = &app.document_state.panel_ctx.pcb_layers_ui;
    assert_eq!(ui.picker, Some(BOTTOM), "the card stays open");
    assert_eq!(
        hex_field(&app),
        palette,
        "the field reads the palette again"
    );

    redo(&mut app);
    assert_eq!(board_colour(&app, &path, BOTTOM), Some(MAGENTA));
    assert_eq!(hex_field(&app), "#FF00FF");

    // A half-typed colour is the user's and stays as typed.
    layers(&mut app, LayersPanelMsg::HexEdited("#12".to_string()));
    undo(&mut app);
    assert_eq!(hex_field(&app), "#12");
}

/// The card closes when another board becomes the active one, so it never
/// recolours a board it was not opened on.
#[test]
fn a_switch_of_board_closes_the_colour_card() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, first) = app_with_board(dir.path());
    layers(&mut app, LayersPanelMsg::ColourChipToggled(BOTTOM));
    assert_eq!(
        app.document_state.panel_ctx.pcb_layers_ui.picker,
        Some(BOTTOM)
    );

    let second = dir.path().join("second.snxpcb");
    app.open_pcb_tab(second.clone(), "second".into(), board());
    assert_eq!(app.active_tab_path(), Some(second.clone()));
    assert_eq!(
        app.document_state.panel_ctx.pcb_layers_ui.picker, None,
        "the card closed with the switch"
    );
    layers(&mut app, LayersPanelMsg::HexEdited("#FF00FF".to_string()));
    assert_eq!(board_colour(&app, &first, BOTTOM), None);
    assert_eq!(board_colour(&app, &second, BOTTOM), None);
}
