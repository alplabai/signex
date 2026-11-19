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
/// dropdowns above the rows (the layer set dropdown also reads All Layers).
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
    assert!(
        listed(&open, "Colour"),
        "the colour row opens under the row"
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
    for label in [
        "GENERAL SETTINGS",
        "OBJECT VISIBILITY",
        "Tracks",
        "Show Grid",
    ] {
        assert!(listed(&ctx, label), "{label}");
    }
}

/// The On cell beside Single Layer turns the mode on. m103: the caption is
/// "Single Layer", the full name its tooltip.
#[test]
fn single_layer_mode_on_sends_the_toggle() {
    let ctx = context_with(&PcbLayerView::default(), options());
    let texts = texts(&ctx);
    let row_y = find(&texts, "Single Layer").bounds.center_y();
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

/// m103: Configuration is one labelled field above the sections, greyed
/// until presets exist; it sends nothing.
#[test]
fn the_configuration_field_is_greyed_and_sends_nothing() {
    let ctx = context_with(&PcbLayerView::default(), options());
    let texts = texts(&ctx);
    let caption = find(&texts, "Configuration").bounds;
    let field = find(&texts, "Default").bounds;
    let general = find(&texts, "GENERAL SETTINGS").bounds;
    assert!(caption.y < general.y, "above the sections");
    assert!(
        (field.center_y() - caption.center_y()).abs() < caption.height,
        "the field beside its caption"
    );
    assert!(messages_on_click(panel(&ctx), WINDOW, "Default").is_empty());
}

/// m103: Show Grid is a switch beside its caption; it toggles the grid.
#[test]
fn the_show_grid_switch_toggles_the_grid() {
    let ctx = context_with(&PcbLayerView::default(), options());
    let label = find(&texts(&ctx), "Show Grid").bounds;
    let sent = messages_along(&ctx, "Show Grid", label.x + label.width, WINDOW.width);
    assert_eq!(shown(&sent), shown(&[PanelMsg::ToggleGrid]));
}

/// m103: All Objects reads Mixed in the Transparency column while the
/// kinds differ, and its eye then shows every kind; with every kind shown
/// there is no Mixed.
#[test]
fn all_objects_reads_mixed_while_the_kinds_differ() {
    let ctx = context_with(&PcbLayerView::default(), options());
    assert!(!listed(&ctx, "Mixed"));

    let mut view = PcbLayerView::default();
    view.set_object_visible(BoardObject::Tracks, false);
    let ctx = context_with(&view, options());
    let texts = texts(&ctx);
    let all = find(&texts, "All Objects").bounds;
    let mixed = find(&texts, "Mixed").bounds;
    let column = find(&texts, "Transparency").bounds;
    assert!(
        (mixed.center_y() - all.center_y()).abs() < all.height,
        "on the All Objects row"
    );
    assert!(
        mixed.x >= column.x - 1.0,
        "in the Transparency column: {mixed:?} {column:?}"
    );
    assert_eq!(
        shown(&left_of(&ctx, "All Objects")),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::ObjectVisibilitySet {
            kind: None,
            visible: true,
        })])
    );
}

/// The percent laid out on the row of `label`.
fn percent_beside(ctx: &PanelContext, label: &str) -> LaidOutText {
    let row = row_text(ctx, label);
    texts(ctx)
        .into_iter()
        .find(|t| {
            t.text.ends_with('%') && (t.bounds.center_y() - row.center_y()).abs() < row.height
        })
        .unwrap_or_else(|| panic!("no percent beside {label}"))
}

/// m103: Mask and Dim show what the view draws today, greyed (nothing
/// sent), their percents lined up with the object rows'.
#[test]
fn mask_and_dim_show_todays_percentages() {
    let ctx = context_with(&PcbLayerView::default(), options());
    let dimmed = format!(
        "{} %",
        (signex_renderer::pcb::DIMMED_ALPHA * 100.0).round() as u32
    );
    for (label, value) in [
        ("Dimmed Objects", dimmed.as_str()),
        ("Highlighted Objects", "100 %"),
        ("Masked Objects", dimmed.as_str()),
    ] {
        let percent = percent_beside(&ctx, label);
        assert_eq!(percent.text, value, "{label}");
        let right = percent.bounds.x + percent.bounds.width;
        let tracks = percent_beside(&ctx, "Tracks").bounds;
        assert!(
            (right - (tracks.x + tracks.width)).abs() < 1.0,
            "{label} lines up with the object rows"
        );
        assert!(
            messages_along(&ctx, label, 0.0, WINDOW.width).is_empty(),
            "{label} is greyed"
        );
    }
}

/// The eleven Additional Options, in their order.
const ADDITIONAL_OPTIONS: [&str; 11] = [
    "Test Points",
    "Status Info",
    "Pad Nets",
    "Pad Numbers",
    "Via Nets",
    "Via Span",
    "All Connections in Single Layer Mode",
    "Net Color Override",
    "Use Layer Colors for Connection Drawing",
    "Repeated Net Names on Tracks",
    "Special Strings",
];

/// m103: Additional Options is closed by default, its header counting the
/// options that are on (none is honoured yet); opened, it lists the eleven
/// options as greyed checkboxes that send nothing.
#[test]
fn additional_options_are_closed_with_a_count_and_list_greyed_checkboxes() {
    let ctx = context_with(&PcbLayerView::default(), options());
    assert!(listed(&ctx, "0/11"), "the header count");
    for label in ADDITIONAL_OPTIONS {
        assert!(!listed(&ctx, label), "{label} closed by default");
    }

    let mut open = context_with(&PcbLayerView::default(), options());
    open.collapsed_sections
        .insert("pcb_view_additional.open".to_string());
    for label in ADDITIONAL_OPTIONS {
        assert!(listed(&open, label), "{label} listed when open");
        assert!(
            messages_on_click(panel(&open), WINDOW, label).is_empty(),
            "{label} is greyed"
        );
    }
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
