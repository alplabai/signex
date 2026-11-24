//! mockup row 104: the layer tab menu's look: the icon column, the Tab names switch
//! and the Show layer numbers row, the open launcher, and the HIDE / SHOW
//! flyouts with All layers first.

use super::*;
use signex_types::pcb_layer::CopperLayer;
use signex_types::theme::theme_tokens;

fn rows(entries: Vec<DropdownEntry<Message>>) -> Vec<(String, Option<Message>, bool)> {
    entries
        .into_iter()
        .filter_map(|e| match e {
            DropdownEntry::Item(item) => Some((item.label, item.on_press, item.checked)),
            _ => None,
        })
        .collect()
}

/// The kind of each entry in order: Item, Separator, Header, Custom.
fn shape(entries: &[DropdownEntry<Message>]) -> String {
    entries
        .iter()
        .map(|e| match e {
            DropdownEntry::Item(_) => 'I',
            DropdownEntry::Separator => 'S',
            DropdownEntry::Header(_) => 'H',
            DropdownEntry::Custom(_) => 'C',
        })
        .collect()
}

fn state<'a>(
    layer: PcbLayer,
    view: &'a PcbLayerView,
    prefs: &'a LayerStripPrefs,
) -> TabMenuState<'a> {
    TabMenuState {
        layer,
        view,
        prefs,
        submenu: None,
        layer_stack_live: true,
        colours: TabColours::default(),
    }
}

fn look(tokens: &ThemeTokens) -> TabMenuLook<'_> {
    TabMenuLook {
        theme: ThemeId::GraphiteDark,
        tokens,
    }
}

#[test]
fn the_menu_lists_every_row_of_the_tab_menu() {
    let layer = PcbLayer::Copper(CopperLayer::Bottom);
    let view = PcbLayerView::default();
    let prefs = LayerStripPrefs::default();
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let entries = layer_tab_menu_rows(&i18n, &state(layer, &view, &prefs), look(&tokens));
    // mockup row 104: Tab names is a header over one switch, Show layer numbers a
    // checkbox row, and Flip follows it with no separator; the colour rows
    // (2026-11-18) follow Highlight.
    assert_eq!(shape(&entries), "IIIIISIISIISIIISHCCI");
    assert!(
        entries
            .iter()
            .any(|e| matches!(e, DropdownEntry::Header(h) if h == "TAB NAMES"))
    );
    let icons: Vec<(String, bool)> = entries
        .iter()
        .filter_map(|e| match e {
            DropdownEntry::Item(item) => Some((item.label.clone(), item.icon.is_some())),
            _ => None,
        })
        .collect();
    let rows = rows(entries);
    let labels: Vec<&str> = rows.iter().map(|(label, _, _)| label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Hide Bottom Layer",
            "Highlight Bottom Layer",
            "Layer color...",
            "Reset layer color",
            "Reset all layer colors",
            "Show only this layer",
            "Show all layers",
            "Hide layers",
            "Show layers",
            "Layer stack...",
            "Drill symbols...",
            "Layer drawing order...",
            "Flip board view",
        ]
    );
    // One icon column: the rows the mockup gives a glyph carry one.
    for (label, has_icon) in icons {
        let bare = matches!(
            label.as_str(),
            "Show all layers" | "Hide layers" | "Show layers"
        );
        assert_eq!(has_icon, !bare, "{label}");
    }
    let row = |label: &str| {
        rows.iter()
            .find(|(l, _, _)| l == label)
            .unwrap_or_else(|| panic!("no row {label:?}"))
    };
    let sent = |label: &str| view_event(&row(label).1);
    assert_eq!(
        sent("Hide Bottom Layer"),
        Some(PcbViewMsg::LayerHidden(layer))
    );
    assert_eq!(
        sent("Highlight Bottom Layer"),
        Some(PcbViewMsg::LayerHighlightToggled(layer))
    );
    assert_eq!(
        sent("Show only this layer"),
        Some(PcbViewMsg::OnlyLayerShown(layer))
    );
    assert_eq!(sent("Show all layers"), Some(PcbViewMsg::AllLayersShown));
    assert_eq!(sent("Flip board view"), Some(PcbViewMsg::BoardFlipToggled));
    // #2401: Layer stack... runs `board_layer_stack` from the menu.
    assert!(matches!(
        row("Layer stack...").1,
        Some(Message::Pcb(PcbMsg::LayerTabMenu(
            LayerTabMenuMsg::LayerStackPicked
        )))
    ));
    // The editors that are not built are greyed, never a silent row.
    for label in ["Drill symbols...", "Layer drawing order..."] {
        assert!(row(label).1.is_none(), "{label} is greyed");
    }
    // The switch's cells, Long on by default.
    assert_eq!(
        tab_name_cells(&i18n, &prefs),
        vec![
            ("Short".to_owned(), LayerNameStyle::Short, false),
            ("Medium".to_owned(), LayerNameStyle::Medium, false),
            ("Long".to_owned(), LayerNameStyle::Long, true),
        ]
    );
}

/// The switch follows the style the tabs use.
#[test]
fn the_tab_names_switch_marks_the_style_in_use() {
    let prefs = LayerStripPrefs {
        name_style: LayerNameStyle::Short,
        ..LayerStripPrefs::default()
    };
    let on: Vec<LayerNameStyle> = tab_name_cells(&I18n::en_us(), &prefs)
        .into_iter()
        .filter(|(_, _, on)| *on)
        .map(|(_, style, _)| style)
        .collect();
    assert_eq!(on, [LayerNameStyle::Short]);
}

/// The open flyout's launcher is the selected row, not the active
/// highlight; the other launcher has neither.
#[test]
fn the_open_launcher_is_the_current_row() {
    let view = PcbLayerView::default();
    let prefs = LayerStripPrefs::default();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let mut state = state(PcbLayer::Copper(CopperLayer::Top), &view, &prefs);
    state.submenu = Some(LayerTabSubmenu::Show);
    let looks: Vec<(String, bool, bool)> =
        layer_tab_menu_rows(&I18n::en_us(), &state, look(&tokens))
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) if item.submenu => {
                    Some((item.label, item.active, item.selected))
                }
                _ => None,
            })
            .collect();
    assert_eq!(
        looks,
        [
            ("Hide layers".to_owned(), false, false),
            ("Show layers".to_owned(), false, true),
        ]
    );
}

/// The kind each kind row of `entries` acts on, in order, with its label.
fn kind_rows(entries: Vec<DropdownEntry<Message>>) -> Vec<(String, LayerKind, bool)> {
    rows(entries)
        .into_iter()
        .filter_map(|(label, message, _)| match view_event(&message) {
            Some(PcbViewMsg::LayerKindVisibilitySet { kind, visible }) => {
                Some((label, kind, visible))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_flyouts_hide_or_show_every_kind() {
    let i18n = I18n::en_us();
    let view = PcbLayerView::default();
    let entries = layer_kind_entries(&i18n, LayerTabSubmenu::Hide, &[], &view);
    // mockup row 104: a header, All layers first, a separator, then the kinds.
    assert!(matches!(&entries[0], DropdownEntry::Header(h) if h == "HIDE"));
    assert!(matches!(entries[2], DropdownEntry::Separator));
    let kinds = kind_rows(entries);
    assert_eq!(kinds.len(), LayerKind::ALL.len());
    let labels: Vec<&str> = kinds.iter().map(|(label, _, _)| label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "All layers",
            "Signal layers",
            "Plane layers",
            "Silkscreen layers",
            "Mask layers",
            "Paste layers",
            "Mechanical layers",
        ]
    );
    assert_eq!(
        (kinds[0].1, kinds[0].2),
        (LayerKind::All, false),
        "All layers hides every layer"
    );
    assert_eq!((kinds[5].1, kinds[5].2), (LayerKind::Paste, false));

    let shown = layer_kind_entries(&i18n, LayerTabSubmenu::Show, &[], &view);
    assert!(matches!(&shown[0], DropdownEntry::Header(h) if h == "SHOW"));
    let kinds = kind_rows(shown);
    assert_eq!((kinds[3].1, kinds[3].2), (LayerKind::Overlay, true));
}

/// The board-view event a row sends, if it sends one.
fn view_event(message: &Option<Message>) -> Option<PcbViewMsg> {
    match message {
        Some(Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Picked(event)))) => Some(*event),
        _ => None,
    }
}
