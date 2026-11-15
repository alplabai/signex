use super::*;
use crate::app::Signex;
use crate::layout_probe::{
    draw_pixels, find, lay_out_texts, messages_on_click, messages_on_click_at,
};
use iced::Size;
use signex_types::pcb_layer::{CopperLayer, MechanicalLayer, TechnicalLayer};

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
fn rows_show_the_names_of_the_layers_in_their_groups() {
    let ctx = context(&PcbLayerView::default());
    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    for label in [
        "Layers",
        "SIGNAL",
        "OVERLAY",
        "MASK",
        "DRILL",
        "MECHANICAL",
        "Top Layer",
        "Bottom Layer",
        "Top Overlay",
        "Board Outline",
        "Assembly notes",
    ] {
        find(&texts, label);
    }
}

#[test]
fn a_name_click_picks_the_copper_layer_and_an_eye_click_toggles_it() {
    let ctx = context(&PcbLayerView::default());
    let picked = messages_on_click(view_panel_for_test(&ctx), WINDOW, "Bottom Layer");
    assert!(
        matches!(
            picked.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(
                PcbLayer::Copper(CopperLayer::Bottom)
            ))]
        ),
        "{picked:?}"
    );

    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    let label = find(&texts, "Bottom Layer").bounds;
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

/// The Board Outline row's eye toggles the layer that gates the drawn edge.
#[test]
fn the_board_outline_eye_toggles_the_board_edge() {
    let ctx = context(&PcbLayerView::default());
    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    let label = find(&texts, "Board Outline").bounds;
    // Same row geometry as a copper row.
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
                signex_renderer::pcb::BOARD_OUTLINE
            ))]
        ),
        "{toggled:?}"
    );
}

/// #2357/#2359: any layer can be the active one, so a technical row's name
/// picks its layer, as its tab under the canvas does.
#[test]
fn a_technical_layer_name_picks_it_as_the_active_layer() {
    let ctx = context(&PcbLayerView::default());
    let clicked = messages_on_click(view_panel_for_test(&ctx), WINDOW, "Top Overlay");
    assert!(
        matches!(
            clicked.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(
                PcbLayer::Technical(TechnicalLayer::SilkscreenTop)
            ))]
        ),
        "{clicked:?}"
    );
}

/// #2359: a group's button hides every layer of the group while all are
/// shown, and shows them all again once one is hidden.
#[test]
fn a_group_button_hides_or_shows_the_whole_group() {
    let ctx = context(&PcbLayerView::default());
    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    let signal = find(&texts, "SIGNAL").bounds;
    let hide = texts
        .iter()
        .filter(|t| t.text == "Hide all" && t.bounds.y > signal.y)
        .min_by(|a, b| a.bounds.y.total_cmp(&b.bounds.y))
        .expect("the Signal group's button");
    let sent = messages_on_click_at(
        view_panel_for_test(&ctx),
        WINDOW,
        None,
        iced::Point::new(hide.bounds.center_x(), hide.bounds.center_y()),
    );
    assert!(
        matches!(
            sent.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::LayerGroupVisibilitySet {
                group: LayerGroup::Signal,
                visible: false,
            })]
        ),
        "{sent:?}"
    );

    let mut view = PcbLayerView::default();
    view.toggle_visibility(PcbLayer::Copper(CopperLayer::Bottom));
    let ctx = context(&view);
    assert!(!ctx.pcb_layers.group_all_shown(LayerGroup::Signal));
    assert!(ctx.pcb_layers.group_all_shown(LayerGroup::Overlay));
}

/// #2357/#2359: the Single layer button sends the same toggle as Shift+S
/// and is drawn pressed while the mode is on.
#[test]
fn the_single_layer_button_toggles_the_mode() {
    let ctx = context(&PcbLayerView::default());
    let sent = messages_on_click(view_panel_for_test(&ctx), WINDOW, "Single layer");
    assert!(
        matches!(
            sent.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::SingleLayerModeToggled)]
        ),
        "{sent:?}"
    );
    let mut on = PcbLayerView::default();
    on.toggle_single_layer();
    assert!(context(&on).pcb_layers.single_layer);
    assert_ne!(
        pixels(&PcbLayerView::default()),
        pixels(&on),
        "the button is filled while on"
    );
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
    find(&texts, "Open a board or a footprint to see its layers.");
}

/// The Multi-Layer row lists with the drill layers; its eye shows or hides
/// vias and through-hole pads, and a click on its name picks no layer.
#[test]
fn the_multi_layer_row_toggles_vias_and_through_hole_pads() {
    let ctx = context(&PcbLayerView::default());
    let texts = lay_out_texts(view_panel_for_test(&ctx), WINDOW);
    let label = find(&texts, "Multi-Layer").bounds;
    assert!(label.y > find(&texts, "Bottom Layer").bounds.y);
    assert!(label.y > find(&texts, "Drill Drawing").bounds.y);
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
            [PanelMsg::PcbLayers(PcbViewMsg::MultiLayerVisibilityToggled)]
        ),
        "{toggled:?}"
    );
    assert!(messages_on_click(view_panel_for_test(&ctx), WINDOW, "Multi-Layer").is_empty());

    let mut view = PcbLayerView::default();
    view.toggle_multi_layer();
    let ctx = context(&view);
    assert!(ctx.pcb_layers.multi_layer_hidden && ctx.pcb_layers.has_hidden());
    let shown_all = messages_on_click(view_panel_for_test(&ctx), WINDOW, "Show all layers");
    assert!(
        matches!(
            shown_all.as_slice(),
            [PanelMsg::PcbLayers(PcbViewMsg::AllLayersShown)]
        ),
        "{shown_all:?}"
    );
}
