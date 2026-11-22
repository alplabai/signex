//! Rows m103 and m104: the Layers panel, the layer tab menu and the layer
//! set menu. What they show is the board view's, never the board's, except a
//! layer colour, which is saved with the board as one undo step.

use signex_app::app::{
    LayerSetCard, LayerSetMenuMsg, LayerStripMsg, LayerTabMenuMsg, LayerTabSubmenu, Message,
    PcbMsg, PcbViewMsg,
};
use signex_app::layer_strip_prefs::LayerNameStyle;
use signex_app::panels::{LayersPanelMsg, LayersTab, PanelKind, PanelMsg};
use signex_renderer::pcb::{BoardObject, BuiltinLayerSet, LayerKind, LayerSetId};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::support::{Bench, four_layers, open};

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const TOP_SILK: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

fn layers_panel(b: &mut Bench, msg: LayersPanelMsg) {
    b.post(Message::Dock(signex_app::dock::DockMessage::Panel(
        PanelMsg::LayersPanel(msg),
    )));
}

fn row(b: &mut Bench, event: PcbViewMsg) {
    b.panel(PanelMsg::PcbLayers(event));
}

fn drawn_tracks(b: &Bench) -> usize {
    b.app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("a drawn board")
        .traces
        .len()
}

// ---- m103 Layers panel ----

#[test]
fn a_layer_eye_hides_its_copper_and_leaves_the_board_clean() {
    let mut b = open(four_layers());
    let shown = drawn_tracks(&b);
    assert!(shown > 0, "the top copper is drawn");
    b.view_only(|b| {
        row(
            b,
            PcbViewMsg::LayerVisibilitySet {
                layer: TOP,
                visible: false,
            },
        );
    });
    assert!(b.layer_view().is_hidden(TOP));
    assert!(drawn_tracks(&b) < shown, "the view reflects the eye");
    assert!(b.app.document_state.panel_ctx.pcb_layers.is_hidden(TOP));

    row(
        &mut b,
        PcbViewMsg::LayerVisibilitySet {
            layer: TOP,
            visible: true,
        },
    );
    assert_eq!(drawn_tracks(&b), shown);
}

#[test]
fn a_layer_row_makes_its_layer_active() {
    let mut b = open(four_layers());
    b.view_only(|b| row(b, PcbViewMsg::ActiveLayerPicked(BOTTOM)));
    assert_eq!(b.layer_view().active(), BOTTOM);
    let panel = &b.app.document_state.panel_ctx.pcb_layers;
    assert_eq!(panel.active, Some(BOTTOM));
}

#[test]
fn the_panel_tab_and_search_are_panel_state_only() {
    let mut b = open(four_layers());
    b.view_only(|b| {
        layers_panel(b, LayersPanelMsg::TabPicked(LayersTab::ViewOptions));
        layers_panel(b, LayersPanelMsg::SearchChanged("top".to_string()));
    });
    let ui = &b.app.document_state.panel_ctx.pcb_layers_ui;
    assert_eq!(ui.tab, LayersTab::ViewOptions);
    assert_eq!(ui.search, "top");
}

#[test]
fn a_typed_layer_colour_is_one_undo_step_saved_with_the_board() {
    let mut b = open(four_layers());
    let was = b.app.document_state.panel_ctx.pcb_layers.hex_of(TOP);
    layers_panel(&mut b, LayersPanelMsg::ColourChipToggled(TOP));
    let edited = b.one_step(|b| {
        layers_panel(b, LayersPanelMsg::HexEdited("#112233".to_string()));
        let shown = b.app.document_state.panel_ctx.pcb_layers.hex_of(TOP);
        assert_eq!(shown.to_ascii_uppercase(), "#112233");
    });
    assert_eq!(edited.options.layer_colors.len(), 1);
    assert_eq!(b.app.document_state.panel_ctx.pcb_layers.hex_of(TOP), was);
}

#[test]
fn a_colour_that_is_not_hex_is_refused() {
    let mut b = open(four_layers());
    layers_panel(&mut b, LayersPanelMsg::ColourChipToggled(TOP));
    for bad in ["#12", "#GGGGGG", "red"] {
        b.refused(|b| layers_panel(b, LayersPanelMsg::HexEdited(bad.to_string())));
    }
}

#[test]
fn hiding_an_object_kind_hides_it_and_leaves_the_board_clean() {
    let mut b = open(four_layers());
    let shown = drawn_tracks(&b);
    b.view_only(|b| {
        row(
            b,
            PcbViewMsg::ObjectVisibilitySet {
                kind: Some(BoardObject::Tracks),
                visible: false,
            },
        );
    });
    assert!(!b.layer_view().shows_object(BoardObject::Tracks));
    assert!(drawn_tracks(&b) < shown, "the tracks are no longer drawn");
}

#[test]
fn an_object_transparency_is_view_state() {
    let mut b = open(four_layers());
    b.view_only(|b| {
        row(
            b,
            PcbViewMsg::ObjectTransparencySet {
                kind: BoardObject::Tracks,
                percent: 40,
            },
        );
    });
}

#[test]
fn single_layer_mode_toggles_and_leaves_the_board_clean() {
    let mut b = open(four_layers());
    b.view_only(|b| row(b, PcbViewMsg::SingleLayerModeToggled));
    assert!(b.layer_view().single_layer());
    row(&mut b, PcbViewMsg::SingleLayerModeToggled);
    assert!(!b.layer_view().single_layer());
}

#[test]
fn the_3d_switch_shows_the_board_in_3d_and_back() {
    let mut b = open(four_layers());
    b.view_only(|b| layers_panel(b, LayersPanelMsg::Board3dToggled));
    assert!(b.app.interaction_state.board_3d.is_some());
    layers_panel(&mut b, LayersPanelMsg::Board3dToggled);
    assert!(b.app.interaction_state.board_3d.is_none());
}

#[test]
fn the_more_menus_layer_stack_row_opens_the_layer_stack() {
    let mut b = open(four_layers());
    layers_panel(&mut b, LayersPanelMsg::MoreMenuToggled);
    b.panel(PanelMsg::LayersPanel(LayersPanelMsg::LayerStackPicked));
    // Layer Stack is a document tab (DECISIONS 2026-11-18).
    assert!(b.app.document_state.tabs.iter().any(|t| t.kind.is_layer_stack()));
    assert!(!b.app.document_state.panel_ctx.pcb_layers_ui.more_menu_open);
}

#[test]
fn cancelling_the_set_name_row_saves_nothing() {
    let mut b = open(four_layers());
    let saved = b.app.ui_state.layer_strip.saved_sets.len();
    layers_panel(&mut b, LayersPanelMsg::SetNamingStarted);
    layers_panel(
        &mut b,
        LayersPanelMsg::SetNameChanged("Routing".to_string()),
    );
    layers_panel(&mut b, LayersPanelMsg::SetNamingCancelled);
    let ui = &b.app.document_state.panel_ctx.pcb_layers_ui;
    assert!(!ui.naming && ui.set_name.is_empty());
    assert_eq!(b.app.ui_state.layer_strip.saved_sets.len(), saved);
}

// ---- m104 layer tab menu ----

fn tab_menu(b: &mut Bench, msg: LayerTabMenuMsg) {
    b.post(Message::Pcb(PcbMsg::LayerTabMenu(msg)));
}

fn open_tab_menu(b: &mut Bench, layer: PcbLayer) {
    tab_menu(
        b,
        LayerTabMenuMsg::Opened {
            window: None,
            layer,
        },
    );
    assert!(b.app.interaction_state.pcb_layer_tab_menu.is_some());
}

fn pick_on_tab(b: &mut Bench, layer: PcbLayer, event: PcbViewMsg) {
    open_tab_menu(b, layer);
    tab_menu(b, LayerTabMenuMsg::Picked(event));
    assert!(
        b.app.interaction_state.pcb_layer_tab_menu.is_none(),
        "a picked row closes the menu"
    );
}

#[test]
fn dismissing_the_tab_menu_changes_nothing() {
    let mut b = open(four_layers());
    let hidden = b.layer_view().has_hidden();
    b.view_only(|b| {
        open_tab_menu(b, TOP);
        tab_menu(b, LayerTabMenuMsg::Dismissed);
    });
    assert!(b.app.interaction_state.pcb_layer_tab_menu.is_none());
    assert_eq!(b.layer_view().has_hidden(), hidden);
}

#[test]
fn hide_this_layer_hides_it_and_show_all_brings_it_back() {
    let mut b = open(four_layers());
    b.view_only(|b| pick_on_tab(b, TOP, PcbViewMsg::LayerHidden(TOP)));
    assert!(b.layer_view().is_hidden(TOP));
    pick_on_tab(&mut b, TOP, PcbViewMsg::AllLayersShown);
    assert!(!b.layer_view().has_hidden());
}

#[test]
fn show_only_this_layer_makes_it_active_and_hides_the_others() {
    let mut b = open(four_layers());
    b.view_only(|b| pick_on_tab(b, BOTTOM, PcbViewMsg::OnlyLayerShown(BOTTOM)));
    let view = b.layer_view();
    assert_eq!(view.active(), BOTTOM);
    assert!(view.is_hidden(TOP));
    assert!(!view.is_hidden(BOTTOM));
}

#[test]
fn highlight_this_layer_toggles() {
    let mut b = open(four_layers());
    b.view_only(|b| pick_on_tab(b, TOP, PcbViewMsg::LayerHighlightToggled(TOP)));
    assert_eq!(b.layer_view().highlighted(), Some(TOP));
    pick_on_tab(&mut b, TOP, PcbViewMsg::LayerHighlightToggled(TOP));
    assert_eq!(b.layer_view().highlighted(), None);
}

#[test]
fn the_hide_layers_flyout_hides_a_kind_and_show_layers_brings_it_back() {
    let mut b = open(four_layers());
    open_tab_menu(&mut b, TOP);
    tab_menu(
        &mut b,
        LayerTabMenuMsg::SubmenuOpened(LayerTabSubmenu::Hide),
    );
    let menu = b.app.interaction_state.pcb_layer_tab_menu.expect("open");
    assert_eq!(menu.submenu, Some(LayerTabSubmenu::Hide));
    tab_menu(
        &mut b,
        LayerTabMenuMsg::Picked(PcbViewMsg::LayerKindVisibilitySet {
            kind: LayerKind::Overlay,
            visible: false,
        }),
    );
    assert!(b.layer_view().is_hidden(TOP_SILK));
    pick_on_tab(
        &mut b,
        TOP,
        PcbViewMsg::LayerKindVisibilitySet {
            kind: LayerKind::Overlay,
            visible: true,
        },
    );
    assert!(!b.layer_view().is_hidden(TOP_SILK));
    assert!(!b.dirty());
}

#[test]
fn flip_board_view_turns_the_view_and_not_the_board() {
    let mut b = open(four_layers());
    b.view_only(|b| pick_on_tab(b, TOP, PcbViewMsg::BoardFlipToggled));
    assert!(b.layer_view().flipped());
}

#[test]
fn the_tab_names_row_sets_the_style_and_closes_the_menu() {
    let mut b = open(four_layers());
    open_tab_menu(&mut b, TOP);
    tab_menu(
        &mut b,
        LayerTabMenuMsg::SettingPicked(LayerStripMsg::NameStyleSet(LayerNameStyle::Short)),
    );
    assert_eq!(b.app.ui_state.layer_strip.name_style, LayerNameStyle::Short);
    assert!(b.app.interaction_state.pcb_layer_tab_menu.is_none());
    assert!(!b.dirty());
}

#[test]
fn show_layer_numbers_toggles() {
    let mut b = open(four_layers());
    let was = b.app.ui_state.layer_strip.show_numbers;
    open_tab_menu(&mut b, TOP);
    tab_menu(
        &mut b,
        LayerTabMenuMsg::SettingPicked(LayerStripMsg::LayerNumbersToggled),
    );
    assert_eq!(b.app.ui_state.layer_strip.show_numbers, !was);
}

// ---- m104 layer set menu ----

fn set_menu(b: &mut Bench, msg: LayerSetMenuMsg) {
    b.post(Message::Pcb(PcbMsg::LayerSetMenu(msg)));
}

fn open_set_menu(b: &mut Bench) {
    set_menu(b, LayerSetMenuMsg::Opened { window: None });
    assert!(b.app.interaction_state.pcb_layer_set_menu.is_some());
}

#[test]
fn a_built_in_set_shows_its_layers_and_leaves_the_board_clean() {
    let mut b = open(four_layers());
    b.view_only(|b| {
        open_set_menu(b);
        set_menu(b, LayerSetMenuMsg::BuiltinPicked(BuiltinLayerSet::Signal));
    });
    assert!(b.app.interaction_state.pcb_layer_set_menu.is_none());
    let view = b.layer_view();
    let shown = view.layer_set().expect("a set is shown");
    assert_eq!(shown.id, LayerSetId::Builtin(BuiltinLayerSet::Signal));
    assert!(view.is_hidden(TOP_SILK), "silkscreen is not a signal layer");
    assert!(!view.is_hidden(TOP));
}

#[test]
fn dismissing_the_set_menu_changes_nothing() {
    let mut b = open(four_layers());
    b.view_only(|b| {
        open_set_menu(b);
        set_menu(b, LayerSetMenuMsg::Dismissed);
    });
    assert!(b.app.interaction_state.pcb_layer_set_menu.is_none());
    assert!(b.layer_view().layer_set().is_none());
}

#[test]
fn a_saved_set_is_shown_listed_and_deleted() {
    let mut b = open(four_layers());
    b.app.ui_state.layer_strip.saved_sets.clear();
    row(&mut b, PcbViewMsg::LayerHidden(TOP_SILK));
    open_set_menu(&mut b);
    set_menu(&mut b, LayerSetMenuMsg::SaveStarted);
    set_menu(&mut b, LayerSetMenuMsg::NameEdited("Routing".to_string()));
    set_menu(&mut b, LayerSetMenuMsg::NameSubmitted);
    assert!(b.app.interaction_state.pcb_layer_set_menu.is_none());
    assert!(b.app.ui_state.layer_strip.saved_set("Routing").is_some());
    let saved = LayerSetId::Saved("Routing".to_string());
    assert_eq!(
        b.layer_view().layer_set().map(|s| s.id.clone()),
        Some(saved)
    );

    open_set_menu(&mut b);
    set_menu(&mut b, LayerSetMenuMsg::ManageStarted);
    set_menu(&mut b, LayerSetMenuMsg::DeletePicked("Routing".to_string()));
    assert!(b.app.ui_state.layer_strip.saved_set("Routing").is_none());
    assert!(b.layer_view().layer_set().is_none(), "the view lets it go");
    assert!(!b.dirty());
}

#[test]
fn a_blank_set_name_saves_nothing_and_keeps_the_field_open() {
    let mut b = open(four_layers());
    b.app.ui_state.layer_strip.saved_sets.clear();
    open_set_menu(&mut b);
    set_menu(&mut b, LayerSetMenuMsg::SaveStarted);
    set_menu(&mut b, LayerSetMenuMsg::NameEdited("   ".to_string()));
    set_menu(&mut b, LayerSetMenuMsg::NameSubmitted);
    assert!(b.app.ui_state.layer_strip.saved_sets.is_empty());
    let menu = b.app.interaction_state.pcb_layer_set_menu.as_ref();
    assert!(matches!(
        menu.map(|m| &m.card),
        Some(LayerSetCard::Naming(_))
    ));
}
