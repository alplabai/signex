//! mockup row 104: the strip's look: its 28 px height and scrolling, a hidden
//! layer's tab, and the layer set button while its menu is open.

use super::*;
use crate::layout_probe::{draw_pixels, lay_out_texts, root_size};
use iced::Size;
use signex_types::pcb::LayerDef;
use signex_types::pcb_layer::CopperLayer;
use signex_types::theme::{ThemeId, theme_tokens};

const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);

/// A board that declares the layer tokens `names`, nothing on them.
fn board(names: &[&str]) -> PcbBoard {
    let layers = names
        .iter()
        .zip(0u8..)
        .map(|(name, id)| LayerDef {
            id,
            name: (*name).to_owned(),
            layer_type: String::new(),
            display_name: String::new(),
        })
        .collect();
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        unknown: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers,
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
        free: Default::default(),
    }
}

fn strip<'a>(
    board: &PcbBoard,
    view: &PcbLayerView,
    set_menu_open: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let prefs = LayerStripPrefs::default();
    let strip = StripBoard {
        board,
        view,
        window: None,
        prefs: &prefs,
        set_menu_open,
    };
    layer_strip(strip, &I18n::en_us(), tokens).expect("the board declares layers")
}

fn names(element: Element<'_, Message>, window: Size) -> Vec<String> {
    lay_out_texts(element, window)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

/// mockup row 104: more tabs than the window is wide scroll inside the strip; every
/// visible layer still has its tab, on one line, and the strip keeps to
/// the window and its 28 px.
#[test]
fn the_strip_lays_out_every_visible_tab_inside_its_scrollable() {
    let board = board(&[
        "TopCopper",
        "BottomCopper",
        "TopSilkscreen",
        "BottomSilkscreen",
        "TopSolderMask",
        "BottomSolderMask",
        "TopPaste",
        "BottomPaste",
        "EdgeCuts",
        "TopCourtyard",
        "BottomCourtyard",
        "Dimensions",
        "Notes",
    ]);
    let view = PcbLayerView::default();
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let window = Size::new(400.0, 600.0);
    let laid_out = lay_out_texts(strip(&board, &view, false, &tokens), window);
    for name in [
        "Top Layer",
        "Bottom Layer",
        "Top Silkscreen",
        "Bottom Silkscreen",
        "Top Solder",
        "Bottom Solder",
        "Top Paste",
        "Bottom Paste",
        "Board Outline",
        "Top Courtyard",
        "Bottom Courtyard",
        "Dimensions",
        "Notes",
    ] {
        let tab = laid_out
            .iter()
            .find(|t| t.text == name)
            .unwrap_or_else(|| panic!("no tab {name:?}: {laid_out:#?}"));
        assert!(tab.bounds.height < 2.0 * TS.body, "{name} on one line");
    }
    let size = root_size(strip(&board, &view, false, &tokens), window);
    assert!(size.width <= window.width, "the strip keeps to the window");
    assert_eq!(size.height, STRIP_HEIGHT);
}

/// A strip too long for its window keeps its height and moves the tabs up,
/// so the scrollbar has its own space under them.
#[test]
fn an_overflowing_strip_keeps_the_scrollbar_clear_of_the_tabs() {
    let board = board(&["TopCopper", "BottomCopper", "TopSilkscreen", "BottomSilkscreen"]);
    let view = PcbLayerView::default();
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let centre = |width: f32| {
        let window = Size::new(width, STRIP_HEIGHT);
        let texts = lay_out_texts(strip(&board, &view, false, &tokens), window);
        texts
            .iter()
            .find(|t| t.text == "Top Layer")
            .expect("the first tab")
            .bounds
            .center_y()
    };
    let (fits, overflows) = (centre(1200.0), centre(120.0));
    assert!(
        fits - overflows >= SCROLLBAR_GAP / 2.0,
        "tabs at {overflows} with a scrollbar, {fits} without"
    );
}

/// A line divides two neighbouring tabs, in both Graphite themes.
#[test]
fn a_visible_line_divides_neighbouring_tabs() {
    let board = board(&["TopCopper", "BottomCopper", "TopPaste"]);
    let view = PcbLayerView::default();
    let window = Size::new(900.0, STRIP_HEIGHT.ceil());
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(theme);
        let texts = lay_out_texts(strip(&board, &view, false, &tokens), window);
        let at = |name: &str| texts.iter().find(|t| t.text == name).expect(name).bounds;
        let (left, right) = (at("Bottom Layer"), at("Top Paste"));
        let pixels = draw_pixels(strip(&board, &view, false, &tokens), window);
        let row = 3 * window.width as usize * 4;
        let pixel = |x: usize| &pixels[row + x * 4..row + x * 4 + 3];
        let from = (left.x + left.width).ceil() as usize + 1;
        let ground = pixel(from).to_vec();
        assert!(
            (from..right.x.floor() as usize).any(|x| pixel(x) != ground.as_slice()),
            "{theme:?}: no divider between the tabs"
        );
    }
}

/// mockup row 104: a hidden layer has no tab; showing every layer brings it back.
/// A layer token this build does not know has no tab: the strip lists what
/// the Layers panel shows (Caner, 2026-11-20).
#[test]
fn a_hidden_layer_has_no_tab_until_it_is_shown() {
    let board = board(&["TopCopper", "BottomCopper", "Nowhere"]);
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let window = Size::new(1200.0, 600.0);
    let mut view = PcbLayerView::default();
    view.hide(BOTTOM);
    let shown = names(strip(&board, &view, false, &tokens), window);
    assert!(!shown.iter().any(|t| t == "Bottom Layer"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "Top Layer"), "{shown:?}");
    assert!(!shown.iter().any(|t| t == "Nowhere"), "{shown:?}");
    view.show_all();
    let shown = names(strip(&board, &view, false, &tokens), window);
    assert!(shown.iter().any(|t| t == "Bottom Layer"), "{shown:?}");
    assert!(!shown.iter().any(|t| t == "Nowhere"), "{shown:?}");
}

/// mockup row 104: the layer set button is filled accent-soft while its menu is
/// open, plain with muted ink otherwise, in both Graphite themes.
#[test]
fn the_set_button_draws_pressed_while_its_menu_is_open() {
    let board = board(&["TopCopper", "BottomCopper"]);
    let view = PcbLayerView::default();
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(theme);
        let r = resolve(&tokens);
        assert_eq!(set_button_look(&tokens, false), (None, r.ink3), "{theme:?}");
        assert_eq!(
            set_button_look(&tokens, true),
            (Some(r.accent_soft), r.accent_soft_ink),
            "{theme:?}"
        );
        let window = Size::new(320.0, STRIP_HEIGHT);
        let closed = draw_pixels(strip(&board, &view, false, &tokens), window);
        let open = draw_pixels(strip(&board, &view, true, &tokens), window);
        assert_ne!(closed, open, "{theme:?}: the open button looks pressed");
    }
}
