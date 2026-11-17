use super::*;
use crate::app::{PcbViewMsg, Signex};
use crate::layout_probe::{
    LaidOutText, draw_pixels, find, lay_out_texts, messages_on_click, messages_on_click_at,
};
use iced::{Point, Size};
use signex_types::pcb_layer::{CopperLayer, TechnicalLayer};

const WINDOW: Size = Size::new(320.0, 2600.0);
const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const OVERLAY_TOP: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const OVERLAY_BOTTOM: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenBottom);

/// Top and Bottom copper and the top overlay; one track, on Top.
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 1, "name": "TopSilkscreen", "layer_type": "" },
            { "id": 31, "name": "BottomCopper", "layer_type": "plane" }
          ],
          "setup": null, "nets": [], "footprints": [],
          "segments": [
            { "uuid": "f5ea9948-4c2f-4501-b647-4047f6eefca9", "start": {"x":0.0,"y":0.0}, "end": {"x":5.0,"y":0.0}, "width": 0.25, "layer": "TopCopper", "net": 1 }
          ],
          "vias": [], "zones": [], "graphics": [], "texts": []
        }"#,
    )
    .expect("valid board")
}

fn context_with(view: &PcbLayerView, ui: LayersPanelUi) -> PanelContext {
    let (mut app, _boot) = Signex::new();
    app.document_state.panel_ctx.pcb_layers =
        PcbLayersPanel::new(&board(), view, app.ui_state.i18n.clone());
    app.document_state.panel_ctx.pcb_layers_ui = ui;
    app.document_state.panel_ctx.clone()
}

fn context(view: &PcbLayerView) -> PanelContext {
    context_with(view, LayersPanelUi::default())
}

fn panel(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    super::view(ctx)
}

fn texts(ctx: &PanelContext) -> Vec<LaidOutText> {
    lay_out_texts(panel(ctx), WINDOW)
}

fn listed(ctx: &PanelContext, label: &str) -> bool {
    texts(ctx).iter().any(|t| t.text == label)
}

/// Messages as text: `PanelMsg` has no equality.
fn shown(msgs: &[PanelMsg]) -> Vec<String> {
    msgs.iter().map(|m| format!("{m:?}")).collect()
}

/// Every message a click publishes along the line of `label`, from `from`
/// to `to` in x: the eyes and chips beside a name.
fn messages_along(ctx: &PanelContext, label: &str, from: f32, to: f32) -> Vec<PanelMsg> {
    let y = find(&texts(ctx), label).bounds.center_y();
    let mut out: Vec<PanelMsg> = Vec::new();
    let mut x = from;
    while x < to {
        for msg in messages_on_click_at(panel(ctx), WINDOW, None, Point::new(x, y)) {
            if !shown(&out).contains(&format!("{msg:?}")) {
                out.push(msg);
            }
        }
        x += 1.0;
    }
    out
}

/// What the eyes and chips left of `label` send.
fn left_of(ctx: &PanelContext, label: &str) -> Vec<PanelMsg> {
    let bounds = find(&texts(ctx), label).bounds;
    messages_along(ctx, label, 0.0, bounds.x)
}

#[test]
fn the_tabs_the_search_and_the_sections_are_listed() {
    let ctx = context(&PcbLayerView::default());
    for label in [
        "Layers",
        "Layers & Colors",
        "View Options",
        "All Layers",
        "Used On",
        "SIGNAL AND PLANE LAYERS",
        "COMPONENT LAYER PAIRS",
        "OTHER TECHNICAL LAYERS",
        "OTHER LAYERS",
        "SYSTEM COLORS",
        "Bottom Layer",
        "Signal",
        "Plane",
        "Overlay",
        "Courtyard",
        "Coating",
        "Board Outline",
        "Drill Drawing",
        "Drill Guide",
        "Multi-Layer",
        "Connection Lines",
        "View From Bottom Side",
    ] {
        assert!(listed(&ctx, label), "{label} listed");
    }
    // #2362: no numbered mechanical layers in the panel.
    assert!(!texts(&ctx).iter().any(|t| t.text.starts_with("Mechanical")));
}

#[test]
fn a_name_click_picks_the_layer_and_its_eye_toggles_it() {
    let ctx = context(&PcbLayerView::default());
    let picked = messages_on_click(panel(&ctx), WINDOW, "Bottom Layer");
    assert_eq!(
        shown(&picked),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(BOTTOM))])
    );
    let beside = shown(&left_of(&ctx, "Bottom Layer"));
    let eye = PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(BOTTOM));
    assert!(beside.contains(&format!("{eye:?}")), "{beside:?}");
    let chip = PanelMsg::LayersPanel(LayersPanelMsg::ColourChipToggled(BOTTOM));
    assert!(
        beside.contains(&format!("{chip:?}")),
        "the chip opens its colours: {beside:?}"
    );
}

/// #2363: a pair row has an eye per side; each hides only its side.
#[test]
fn a_pair_row_eye_toggles_only_its_side() {
    let ctx = context(&PcbLayerView::default());
    let label = find(&texts(&ctx), "Overlay").bounds;
    let left = messages_along(&ctx, "Overlay", 0.0, label.x);
    let right = messages_along(&ctx, "Overlay", label.x + label.width, WINDOW.width);
    let toggles = |sent: &[PanelMsg]| -> Vec<PcbLayer> {
        sent.iter()
            .filter_map(|m| match m {
                PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(l)) => Some(*l),
                _ => None,
            })
            .collect()
    };
    assert_eq!(toggles(&left), [OVERLAY_TOP]);
    assert_eq!(toggles(&right), [OVERLAY_BOTTOM]);
}

/// A row Signex has no layer for is listed greyed and sends nothing.
#[test]
fn a_row_without_a_layer_sends_nothing() {
    let ctx = context(&PcbLayerView::default());
    let bounds = find(&texts(&ctx), "Coating").bounds;
    assert!(messages_along(&ctx, "Coating", 0.0, bounds.x + bounds.width).is_empty());
}

/// #2363: an open chip lists the colours; a set colour is the layer's
/// colour in this view, and its chip shows it.
#[test]
fn an_open_chip_shows_the_colours_and_a_set_colour_is_drawn() {
    let ui = LayersPanelUi {
        picker: Some((BOTTOM, false)),
        ..LayersPanelUi::default()
    };
    let mut view = PcbLayerView::default();
    let closed = draw_pixels(panel(&context(&view)), WINDOW);
    let open = context_with(&view, ui);
    assert!(listed(&open, "Colour"), "the colour row opens under the row");
    view.set_layer_colour(BOTTOM, Some([255, 0, 255]));
    let ctx = context(&view);
    assert_eq!(
        ctx.pcb_layers.colour(BOTTOM),
        iced::Color::from_rgb8(255, 0, 255)
    );
    assert_ne!(closed, draw_pixels(panel(&ctx), WINDOW), "the chip is recoloured");
}

/// #2363: Used On lists only the layers the board has objects on.
#[test]
fn used_on_hides_the_empty_layers() {
    let mut view = PcbLayerView::default();
    let all = context(&view);
    assert!(listed(&all, "Bottom Layer") && listed(&all, "Overlay"));
    view.set_used_only(true);
    let used = context(&view);
    assert!(!listed(&used, "Bottom Layer"), "nothing is on Bottom");
    assert!(!listed(&used, "Overlay"), "nothing is on an overlay");
    assert!(!listed(&used, "Coating"), "a row with no layer goes too");
    let sent = messages_on_click(panel(&used), WINDOW, "Used On");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::UsedLayersOnlySet(false))])
    );
}

#[test]
fn the_search_keeps_the_rows_whose_name_holds_it() {
    let ui = LayersPanelUi {
        search: "COURT".to_string(),
        ..LayersPanelUi::default()
    };
    let ctx = context_with(&PcbLayerView::default(), ui);
    assert!(listed(&ctx, "Courtyard"));
    assert!(!listed(&ctx, "Overlay") && !listed(&ctx, "Bottom Layer"));
}

#[test]
fn view_from_bottom_side_flips_the_view() {
    let ctx = context(&PcbLayerView::default());
    let sent = messages_on_click(panel(&ctx), WINDOW, "View From Bottom Side");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::BoardFlipToggled)])
    );
}

#[test]
fn the_all_layers_eye_hides_every_layer_then_shows_them() {
    let ctx = context(&PcbLayerView::default());
    assert_eq!(
        shown(&left_of(&ctx, "All Layers")),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::LayerKindVisibilitySet {
            kind: signex_renderer::pcb::LayerKind::All,
            visible: false,
        })])
    );
    let mut hidden = PcbLayerView::default();
    hidden.toggle_visibility(BOTTOM);
    assert_eq!(
        shown(&left_of(&context(&hidden), "All Layers")),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::AllLayersShown)])
    );
}

#[test]
fn the_multi_layer_and_connection_line_eyes_toggle_vias_and_the_ratsnest() {
    let ctx = context(&PcbLayerView::default());
    assert_eq!(
        shown(&left_of(&ctx, "Multi-Layer")),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::MultiLayerVisibilityToggled)])
    );
    assert_eq!(
        shown(&left_of(&ctx, "Connection Lines")),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::RatsnestVisibilityToggled)])
    );
    assert!(left_of(&ctx, "Pad Holes").is_empty(), "not yet: greyed");
}

fn options() -> LayersPanelUi {
    LayersPanelUi {
        tab: LayersTab::ViewOptions,
        ..LayersPanelUi::default()
    }
}

#[test]
fn a_tab_click_switches_to_view_options() {
    let ctx = context(&PcbLayerView::default());
    let sent = messages_on_click(panel(&ctx), WINDOW, "View Options");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::TabPicked(
            LayersTab::ViewOptions
        ))])
    );
    let ctx = context_with(&PcbLayerView::default(), options());
    for label in ["GENERAL SETTINGS", "OBJECT VISIBILITY", "Tracks", "Show Grid"] {
        assert!(listed(&ctx, label), "{label}");
    }
}

/// The On cell beside Single Layer Mode turns the mode on.
#[test]
fn single_layer_mode_on_sends_the_toggle() {
    let ctx = context_with(&PcbLayerView::default(), options());
    let texts = texts(&ctx);
    let row_y = find(&texts, "Single Layer Mode").bounds.center_y();
    let on = texts
        .iter()
        .filter(|t| t.text == "On")
        .min_by(|a, b| {
            (a.bounds.center_y() - row_y)
                .abs()
                .total_cmp(&(b.bounds.center_y() - row_y).abs())
        })
        .expect("an On cell");
    let sent = messages_on_click_at(panel(&ctx), WINDOW, None, on.bounds.center());
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::SingleLayerModeToggled)])
    );
}

/// A kind's eye hides it; its transparency slider sends the percent.
#[test]
fn an_object_row_hides_and_fades_its_kind() {
    let ctx = context_with(&PcbLayerView::default(), options());
    assert_eq!(
        shown(&left_of(&ctx, "Tracks")),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::ObjectVisibilitySet {
            kind: Some(BoardObject::Tracks),
            visible: false,
        })])
    );
    let label = find(&texts(&ctx), "Tracks").bounds;
    let right = messages_along(&ctx, "Tracks", label.x + label.width, WINDOW.width);
    assert!(
        right.iter().any(|m| matches!(
            m,
            PanelMsg::PcbLayers(PcbViewMsg::ObjectTransparencySet {
                kind: BoardObject::Tracks,
                ..
            })
        )),
        "{right:?}"
    );
    // Rooms cannot be hidden yet: greyed, nothing sent.
    assert!(left_of(&ctx, "Rooms").is_empty());
}

#[test]
fn the_empty_panel_says_a_board_is_needed() {
    let (app, _boot) = Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let texts = texts(&ctx);
    find(&texts, "Layers");
    find(&texts, "Open a board or a footprint to see its layers.");
}

#[test]
fn the_panel_state_opens_and_closes_a_chip() {
    let mut ui = LayersPanelUi::default();
    assert!(ui.apply(&LayersPanelMsg::ColourChipToggled(TOP)));
    assert_eq!(ui.picker, Some((TOP, false)));
    ui.apply(&LayersPanelMsg::CustomColourOpened);
    assert_eq!(ui.picker, Some((TOP, true)));
    ui.apply(&LayersPanelMsg::ColourChipToggled(TOP));
    assert_eq!(ui.picker, None);
    assert!(!ui.apply(&LayersPanelMsg::Board3dToggled), "the app's to run");
}
