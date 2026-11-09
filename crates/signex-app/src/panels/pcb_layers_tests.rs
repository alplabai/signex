use super::*;
use crate::app::Signex;
use crate::layout_probe::{
    draw_pixels, find, lay_out_texts, messages_on_click, messages_on_click_at,
};
use iced::Size;
use signex_types::pcb_layer::MechanicalLayer;

const WINDOW: Size = Size::new(300.0, 1800.0);

fn layer(id: u8, name: &str, display: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.to_owned(),
        layer_type: String::new(),
        display_name: display.to_owned(),
    }
}

fn layers() -> Vec<LayerDef> {
    vec![
        layer(0, "TopCopper", ""),
        layer(31, "BottomCopper", ""),
        layer(40, "Mechanical3", "Assembly notes"),
    ]
}

fn context(view: &PcbLayerView) -> PanelContext {
    let (mut app, _boot) = Signex::new();
    app.document_state.panel_ctx.pcb_layers =
        PcbLayersPanel::new(&layers(), view, app.ui_state.i18n.clone());
    app.document_state.panel_ctx.clone()
}

fn pixels(view: &PcbLayerView) -> Vec<u8> {
    let ctx = context(view);
    draw_pixels(view_panel_for_test(&ctx), WINDOW)
}

fn view_panel_for_test(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    super::view(ctx)
}

#[test]
fn rows_show_the_names_of_copper_technical_and_mechanical_layers() {
    let ctx = context(&PcbLayerView::default());
    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    for label in [
        "Layers",
        "Copper",
        "Technical",
        "Mechanical",
        "Top Copper",
        "Bottom Copper",
        "Top Silkscreen",
        "Assembly notes",
    ] {
        find(&texts, label);
    }
}

#[test]
fn a_name_click_picks_the_copper_layer_and_an_eye_click_toggles_it() {
    let ctx = context(&PcbLayerView::default());
    let picked = messages_on_click(view_panel_for_test(&ctx), WINDOW, "Bottom Copper");
    assert!(
        matches!(
            picked.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(
                CopperLayer::Bottom
            ))]
        ),
        "{picked:?}"
    );

    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    let label = find(&texts, "Bottom Copper").bounds;
    // The eye sits left of the swatch: row spacing, name padding, swatch.
    let eye_x = label.x - SP.s - 12.0 - SP.s - SP.xs - (7.0 + SP.s);
    let toggled = messages_on_click_at(
        view_panel_for_test(&ctx),
        WINDOW,
        None,
        iced::Point::new(eye_x, label.center_y()),
    );
    assert!(
        matches!(
            toggled.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(
                PcbLayer::Copper(CopperLayer::Bottom)
            ))]
        ),
        "{toggled:?}"
    );
}

/// A technical layer has no active state: its name is not a button.
#[test]
fn a_technical_layer_name_is_not_clickable() {
    let ctx = context(&PcbLayerView::default());
    let clicked = messages_on_click(view_panel_for_test(&ctx), WINDOW, "Top Silkscreen");
    assert!(clicked.is_empty(), "{clicked:?}");
}

/// Graphite look: the active row, a hidden layer's eye and swatch each draw
/// differently from the plain row (selection tint, struck eye, faint swatch).
#[test]
fn the_active_row_and_a_hidden_layer_are_drawn_differently() {
    let plain = pixels(&PcbLayerView::default());

    let mut other_active = PcbLayerView::default();
    other_active.set_active(CopperLayer::Bottom);
    assert_ne!(plain, pixels(&other_active), "the active row is tinted");

    let mut hidden = PcbLayerView::default();
    hidden.toggle_visibility(PcbLayer::Mechanical(MechanicalLayer::new(3).unwrap()));
    assert_ne!(plain, pixels(&hidden), "a hidden layer is struck and faint");
}

#[test]
fn the_empty_panel_says_a_board_is_needed() {
    let (app, _boot) = Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    find(&texts, "Layers");
    find(&texts, "Open a PCB to see its layers.");
}
