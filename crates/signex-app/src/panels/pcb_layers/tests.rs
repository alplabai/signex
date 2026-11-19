use super::*;
use crate::app::{PcbViewMsg, Signex};
use crate::layout_probe::{
    LaidOutText, draw_pixels, find, lay_out_texts, messages_on_click, messages_on_click_at,
};
use iced::{Point, Size};
use signex_renderer::pcb::BOARD_OUTLINE;
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

/// Where the row named `label` is: its last text of that name, below the
/// Active row above the rows (its dropdown reads the active layer's name).
fn row_text(ctx: &PanelContext, label: &str) -> iced::Rectangle {
    texts(ctx)
        .into_iter()
        .rev()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("no row {label}"))
        .bounds
}

/// Every message a click publishes along the line of `label`, from `from`
/// to `to` in x: the eyes and chips beside a name.
fn messages_along(ctx: &PanelContext, label: &str, from: f32, to: f32) -> Vec<PanelMsg> {
    let y = row_text(ctx, label).center_y();
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
    let bounds = row_text(ctx, label);
    messages_along(ctx, label, 0.0, bounds.x)
}

/// `text` without the Unicode isolation marks a message argument is wrapped
/// in.
fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .collect()
}

/// The context with System Colors opened: it starts closed (#103).
fn with_system_colors(mut ctx: PanelContext) -> PanelContext {
    ctx.collapsed_sections
        .insert("pcb_layers_system.open".to_string());
    ctx
}

/// #103: no title in the body (the dock's title bar names the panel), the
/// search row with Used On, the Active | Set row and the sections. View
/// From Bottom Side is in the more menu now.
#[test]
fn the_tabs_the_search_and_the_sections_are_listed() {
    let ctx = with_system_colors(context(&PcbLayerView::default()));
    for label in [
        "Layers & Colors",
        "View Options",
        "ACTIVE",
        "Set: All",
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
        "Silkscreen",
        "Courtyard",
        "Board Outline",
        "Drill Drawing",
        "Multi-Layer",
        "Connection Lines",
    ] {
        assert!(listed(&ctx, label), "{label} listed");
    }
    for label in ["Layers", "Overlay", "View From Bottom Side", "Layer Sets"] {
        assert!(!listed(&ctx, label), "{label} is not in the body");
    }
    // #2362: no numbered mechanical layers in the panel.
    assert!(!texts(&ctx).iter().any(|t| t.text.starts_with("Mechanical")));
}

/// #103: System Colors starts closed; its header opens it.
#[test]
fn system_colors_start_closed_and_open_from_the_header() {
    let ctx = context(&PcbLayerView::default());
    assert!(listed(&ctx, "SYSTEM COLORS"));
    assert!(!listed(&ctx, "Connection Lines"), "closed by default");
    let sent = messages_on_click(panel(&ctx), WINDOW, "SYSTEM COLORS");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::ToggleSection(
            "pcb_layers_system.open".to_string()
        )])
    );
    assert!(listed(&with_system_colors(ctx), "Connection Lines"));
}

/// #103: each section header counts the rows it lists: two copper layers,
/// the eleven pairs with layers, six technical layers, Drill Drawing and
/// Multi-Layer, and the fourteen system colours.
#[test]
fn a_section_header_counts_its_rows() {
    let ctx = context(&PcbLayerView::default());
    let texts = texts(&ctx);
    for (title, count) in [
        ("SIGNAL AND PLANE LAYERS", "2"),
        ("COMPONENT LAYER PAIRS", "11"),
        ("OTHER TECHNICAL LAYERS", "6"),
        ("OTHER LAYERS", "2"),
        ("SYSTEM COLORS", "14"),
    ] {
        let y = find(&texts, title).bounds.center_y();
        assert!(
            texts.iter().any(|t| plain(&t.text) == count
                && t.bounds.y <= y
                && y <= t.bounds.y + t.bounds.height),
            "{title} counts {count}"
        );
    }
}

/// #103: a section's eye hides every layer of the section, or shows them
/// all while one is hidden.
#[test]
fn a_section_eye_hides_or_shows_the_whole_section() {
    let ctx = context(&PcbLayerView::default());
    let title = row_text(&ctx, "SIGNAL AND PLANE LAYERS");
    let sent = messages_along(
        &ctx,
        "SIGNAL AND PLANE LAYERS",
        title.x + title.width,
        WINDOW.width,
    );
    let hide = PanelMsg::LayersPanel(LayersPanelMsg::SectionVisibilitySet {
        section: LayerSection::Copper,
        visible: false,
    });
    assert!(shown(&sent).contains(&format!("{hide:?}")), "{sent:?}");

    let mut one_hidden = PcbLayerView::default();
    one_hidden.toggle_visibility(BOTTOM);
    let ctx = context(&one_hidden);
    let sent = messages_along(
        &ctx,
        "SIGNAL AND PLANE LAYERS",
        title.x + title.width,
        WINDOW.width,
    );
    let show = PanelMsg::LayersPanel(LayersPanelMsg::SectionVisibilitySet {
        section: LayerSection::Copper,
        visible: true,
    });
    assert!(shown(&sent).contains(&format!("{show:?}")), "{sent:?}");
}

/// The layers a section's eye acts on, and when it reads shut.
#[test]
fn a_section_lists_its_layers_and_is_shut_while_all_are_hidden() {
    let ctx = context(&PcbLayerView::default());
    let panel = &ctx.pcb_layers;
    assert_eq!(panel.section_layers(LayerSection::Copper), [TOP, BOTTOM]);
    let pairs = panel.section_layers(LayerSection::Pairs);
    assert_eq!(pairs.len(), 22, "both sides of the eleven pairs");
    assert!(pairs.contains(&OVERLAY_TOP) && pairs.contains(&OVERLAY_BOTTOM));
    assert_eq!(panel.section_layers(LayerSection::Technical).len(), 6);
    assert!(!panel.section_hidden(LayerSection::Copper));

    let mut hidden = PcbLayerView::default();
    hidden.toggle_visibility(TOP);
    hidden.toggle_visibility(BOTTOM);
    let ctx = context(&hidden);
    assert!(ctx.pcb_layers.section_hidden(LayerSection::Copper));
    assert!(ctx.pcb_layers.section_any_hidden(LayerSection::Copper));
    assert!(
        ctx.pcb_layers
            .toggles_multi_layer(LayerSection::Other, false),
        "hiding Other Layers hides the Multi-Layer row too"
    );
    assert!(
        !ctx.pcb_layers
            .toggles_multi_layer(LayerSection::Copper, false)
    );
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

/// #2363: a pair row has an eye per side; each hides only its side. #103:
/// the overlay pair reads Silkscreen.
#[test]
fn a_pair_row_eye_toggles_only_its_side() {
    let ctx = context(&PcbLayerView::default());
    let label = find(&texts(&ctx), "Silkscreen").bounds;
    let left = messages_along(&ctx, "Silkscreen", 0.0, label.x);
    let right = messages_along(&ctx, "Silkscreen", label.x + label.width, WINDOW.width);
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

/// #103: a row Signex has no layer for is not listed (it was a greyed row
/// that sent nothing); it shows once the board has a layer for it.
#[test]
fn a_row_without_a_layer_is_not_listed() {
    let ctx = context(&PcbLayerView::default());
    for label in [
        "Coating",
        "Component Center",
        "Tenting",
        "Value",
        "Drill Guide",
    ] {
        assert!(!listed(&ctx, label), "{label} is not listed");
    }
}

/// #2363: an open dot shows the colour card; a set colour is the layer's
/// colour in this view, and its dot shows it. #103: the card lists the
/// palette and a hex field holding the typed text.
#[test]
fn an_open_chip_shows_the_colours_and_a_set_colour_is_drawn() {
    let ui = LayersPanelUi {
        picker: Some(BOTTOM),
        hex: "#386CD0".to_string(),
        ..LayersPanelUi::default()
    };
    let mut view = PcbLayerView::default();
    let closed = draw_pixels(panel(&context(&view)), WINDOW);
    let open = context_with(&view, ui);
    for label in ["Palette", "Colour"] {
        assert!(listed(&open, label), "the card shows {label}");
    }
    assert!(
        !listed(&open, "Palette colour"),
        "no palette colour again without a colour of its own"
    );
    view.set_layer_colour(BOTTOM, Some([255, 0, 255]));
    let ctx = context(&view);
    assert_eq!(
        ctx.pcb_layers.colour(BOTTOM),
        iced::Color::from_rgb8(255, 0, 255)
    );
    assert_ne!(
        closed,
        draw_pixels(panel(&ctx), WINDOW),
        "the chip is recoloured"
    );
}

/// #2363: Used On lists only the layers the board has objects on.
#[test]
fn used_on_hides_the_empty_layers() {
    let mut view = PcbLayerView::default();
    let all = context(&view);
    assert!(listed(&all, "Bottom Layer") && listed(&all, "Silkscreen"));
    view.set_used_only(true);
    let used = context(&view);
    assert!(!listed(&used, "Bottom Layer"), "nothing is on Bottom");
    assert!(!listed(&used, "Silkscreen"), "nothing is on a silkscreen");
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
    assert!(!listed(&ctx, "Silkscreen") && !listed(&ctx, "Bottom Layer"));
    assert!(
        !texts(&ctx)
            .iter()
            .any(|t| t.text.contains("No layers match"))
    );
}

/// #103: a search that lists no row says so, and Clear empties it.
#[test]
fn a_search_with_no_row_says_so_and_clears() {
    let ui = LayersPanelUi {
        search: "zzz".to_string(),
        ..LayersPanelUi::default()
    };
    let ctx = context_with(&PcbLayerView::default(), ui);
    let line = texts(&ctx)
        .into_iter()
        .find(|t| t.text.starts_with("No layers match"))
        .expect("the no-match line");
    assert!(plain(&line.text).contains("zzz"), "{}", line.text);
    let sent = messages_on_click(panel(&ctx), WINDOW, "Clear");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::SearchChanged(
            String::new()
        ))])
    );
}

/// The context with the more menu open.
fn menu_open(view: &PcbLayerView) -> PanelContext {
    let ui = LayersPanelUi {
        more_menu_open: true,
        ..LayersPanelUi::default()
    };
    context_with(view, ui)
}

#[path = "menu_tests.rs"]
mod menu;

/// View From Bottom Side, now in the more menu (#103), flips the view.
#[test]
fn view_from_bottom_side_flips_the_view() {
    let ctx = menu_open(&PcbLayerView::default());
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
    let ctx = with_system_colors(context(&PcbLayerView::default()));
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
    for label in [
        "GENERAL SETTINGS",
        "OBJECT VISIBILITY",
        "Tracks",
        "Show Grid",
    ] {
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
    assert_eq!(ui.picker, Some(TOP));
    ui.apply(&LayersPanelMsg::ColourChipToggled(BOTTOM));
    assert_eq!(ui.picker, Some(BOTTOM), "another dot moves the card");
    ui.apply(&LayersPanelMsg::ColourChipToggled(BOTTOM));
    assert_eq!(ui.picker, None);
    assert!(
        !ui.apply(&LayersPanelMsg::HexEdited("#FF00".to_string())),
        "a typed colour is the app's to apply"
    );
    assert_eq!(ui.hex, "#FF00", "the field keeps the text as typed");
    assert_eq!(ui.hex_colour(), None, "not a colour yet");
    ui.apply(&LayersPanelMsg::HexEdited("#ff00FF".to_string()));
    assert_eq!(ui.hex_colour(), Some([255, 0, 255]));
    assert!(
        !ui.apply(&LayersPanelMsg::Board3dToggled),
        "the app's to run"
    );
}

/// The Board Outline row's eye toggles the layer that gates the drawn edge.
#[test]
fn the_board_outline_eye_toggles_the_board_edge() {
    let ctx = context(&PcbLayerView::default());
    let beside = shown(&left_of(&ctx, "Board Outline"));
    let eye = PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(BOARD_OUTLINE));
    assert!(beside.contains(&format!("{eye:?}")), "{beside:?}");
}

/// #2357/#2359: any layer can be the active one, so a technical row's name
/// picks its layer, as its tab under the canvas does.
#[test]
fn a_technical_layer_name_picks_it_as_the_active_layer() {
    let ctx = context(&PcbLayerView::default());
    let drill = PcbLayer::Technical(TechnicalLayer::DrillDrawing);
    for (label, layer) in [("Board Outline", BOARD_OUTLINE), ("Drill Drawing", drill)] {
        let at = row_text(&ctx, label).center();
        let clicked = messages_on_click_at(panel(&ctx), WINDOW, None, at);
        assert_eq!(
            shown(&clicked),
            shown(&[PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(layer))]),
            "{label}"
        );
    }
}

/// The keep-out chips are greyed: the board's keepout regions cannot take a
/// view colour yet. The eyes still work.
#[test]
fn the_keep_out_chips_open_no_colours() {
    let ctx = context(&PcbLayerView::default());
    let sent = messages_along(&ctx, "Keep-Out", 0.0, WINDOW.width);
    assert!(
        !sent.iter().any(|m| matches!(
            m,
            PanelMsg::LayersPanel(LayersPanelMsg::ColourChipToggled(_))
        )),
        "{sent:?}"
    );
    assert!(
        sent.iter().any(|m| matches!(
            m,
            PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(_))
        )),
        "{sent:?}"
    );
}

/// The pixels of the panel in the band of the row named `label`.
fn row_band(view: &PcbLayerView, label: &str) -> Vec<u8> {
    let ctx = context(view);
    let bounds = row_text(&ctx, label);
    let pixels = draw_pixels(panel(&ctx), WINDOW);
    let stride = WINDOW.width as usize * 4;
    let y0 = bounds.y.floor() as usize;
    let y1 = (bounds.y + bounds.height).ceil() as usize;
    pixels[y0 * stride..y1 * stride].to_vec()
}

/// Graphite look: the active row, and a hidden layer's eye, chip and name,
/// each draw differently from the plain row. Only the row's own band is
/// compared, so the Active Layer dropdown and the All Layers eye do not
/// count.
#[test]
fn the_active_row_and_a_hidden_layer_are_drawn_differently() {
    let plain = row_band(&PcbLayerView::default(), "Bottom Layer");

    let mut active = PcbLayerView::default();
    active.set_active(CopperLayer::Bottom);
    assert_ne!(
        plain,
        row_band(&active, "Bottom Layer"),
        "the active row is tinted"
    );

    let mut hidden = PcbLayerView::default();
    hidden.toggle_visibility(BOTTOM);
    assert_ne!(
        plain,
        row_band(&hidden, "Bottom Layer"),
        "a hidden layer is struck and faint"
    );
}
