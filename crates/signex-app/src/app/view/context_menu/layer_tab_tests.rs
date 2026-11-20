use super::*;
use signex_types::pcb_layer::{CopperLayer, TechnicalLayer};

fn rows(entries: Vec<DropdownEntry<Message>>) -> Vec<(String, Option<Message>, bool)> {
    entries
        .into_iter()
        .filter_map(|e| match e {
            DropdownEntry::Item(item) => Some((item.label, item.on_press, item.checked)),
            _ => None,
        })
        .collect()
}

fn state<'a>(
    layer: PcbLayer,
    view: &'a PcbLayerView,
    prefs: &'a LayerStripPrefs,
    colours: TabColours,
) -> TabMenuState<'a> {
    TabMenuState {
        layer,
        view,
        prefs,
        submenu: None,
        layer_stack_live: true,
        colours,
    }
}

#[test]
fn the_menu_lists_every_row_of_the_tab_menu() {
    let layer = PcbLayer::Copper(CopperLayer::Bottom);
    let view = PcbLayerView::default();
    let prefs = LayerStripPrefs::default();
    let state = state(layer, &view, &prefs, TabColours::default());
    let i18n = I18n::en_us();
    let entries = layer_tab_menu_entries(&i18n, &state);
    // mockup row 104 (Caner, 2026-11-20): the four tab name rows are a TAB NAMES
    // header over the Short | Medium | Long switch and the Show layer
    // numbers checkbox row, both custom entries, then Flip board view.
    let tail = &entries[entries.len() - 4..];
    assert!(matches!(&tail[0], DropdownEntry::Header(h) if h == "TAB NAMES"));
    assert!(matches!(tail[1], DropdownEntry::Custom(_)), "the switch");
    assert!(
        matches!(tail[2], DropdownEntry::Custom(_)),
        "the Show layer numbers row"
    );
    assert!(
        matches!(&tail[3], DropdownEntry::Item(item) if item.label == "Flip board view"),
        "Flip board view follows the numbers row"
    );
    let rows = rows(entries);
    let labels: Vec<&str> = rows.iter().map(|(label, _, _)| label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Hide Bottom Layer",
            "Highlight Bottom Layer",
            "Layer colour...",
            "Reset layer colour",
            "Reset all layer colours",
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
    // 2026-11-18: Layer colour... opens the picker of the tab's layer.
    assert!(matches!(
        row("Layer colour...").1,
        Some(Message::Pcb(PcbMsg::LayerTabMenu(
            LayerTabMenuMsg::ColourEditRequested(Some(l))
        ))) if l == layer
    ));
    // The editors that are not built are greyed, never a silent row; the
    // resets are greyed while the board has no colour to reset.
    for label in [
        "Drill symbols...",
        "Layer drawing order...",
        "Reset layer colour",
        "Reset all layer colours",
    ] {
        assert!(row(label).1.is_none(), "{label} is greyed");
    }
    let cells = tab_name_cells(&i18n, &prefs);
    let on: Vec<&str> = cells
        .iter()
        .filter(|(_, _, on)| *on)
        .map(|(name, _, _)| name.as_str())
        .collect();
    assert_eq!(on, ["Long"], "the default style is the selected cell");
    assert_eq!(
        cells,
        vec![
            ("Short".to_owned(), LayerNameStyle::Short, false),
            ("Medium".to_owned(), LayerNameStyle::Medium, false),
            ("Long".to_owned(), LayerNameStyle::Long, true),
        ]
    );
}

/// 2026-11-18: with colours on the board the reset rows are live and send
/// the palette colour for the layer, or for every layer.
#[test]
fn the_reset_rows_follow_the_board_colours() {
    let layer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let view = PcbLayerView::default();
    let prefs = LayerStripPrefs::default();
    let colours = TabColours {
        own: true,
        any: true,
        read_only: false,
    };
    let rows = rows(layer_tab_menu_entries(
        &I18n::en_us(),
        &state(layer, &view, &prefs, colours),
    ));
    let sent = |label: &str| {
        let row = rows.iter().find(|(l, _, _)| l == label);
        view_event(&row.and_then(|(_, m, _)| m.clone()))
    };
    assert_eq!(
        sent("Reset layer colour"),
        Some(PcbViewMsg::LayerColourSet {
            layer,
            colour: None
        })
    );
    assert_eq!(
        sent("Reset all layer colours"),
        Some(PcbViewMsg::LayerColoursReset)
    );
}

/// A read-only board keeps its colours: the colour rows are greyed, as its
/// colour commands are.
#[test]
fn a_read_only_board_greys_the_colour_rows() {
    let layer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let view = PcbLayerView::default();
    let prefs = LayerStripPrefs::default();
    let colours = TabColours {
        own: true,
        any: true,
        read_only: true,
    };
    let rows = rows(layer_tab_menu_entries(
        &I18n::en_us(),
        &state(layer, &view, &prefs, colours),
    ));
    for label in [
        "Layer colour...",
        "Reset layer colour",
        "Reset all layer colours",
    ] {
        let row = rows.iter().find(|(l, _, _)| l == label);
        assert!(
            row.is_some_and(|(_, message, _)| message.is_none()),
            "{label} is greyed"
        );
    }
}

/// A layer the Layers panel has no colour row for greys Layer colour...
#[test]
fn a_keepout_or_mechanical_layer_greys_layer_colour() {
    let view = PcbLayerView::default();
    let prefs = LayerStripPrefs::default();
    let keepout = PcbLayer::Technical(TechnicalLayer::KeepoutTop);
    let rows = rows(layer_tab_menu_entries(
        &I18n::en_us(),
        &state(keepout, &view, &prefs, TabColours::default()),
    ));
    let colour = rows.iter().find(|(l, _, _)| l == "Layer colour...");
    assert!(colour.is_some_and(|(_, message, _)| message.is_none()));
}

/// #2401: Layer stack... greys exactly when `board_layer_stack` is
/// disabled in the menu's window.
#[test]
fn the_layer_stack_row_follows_its_command() {
    let view = PcbLayerView::default();
    let prefs = LayerStripPrefs::default();
    let state = TabMenuState {
        layer_stack_live: false,
        ..state(
            PcbLayer::Copper(CopperLayer::Top),
            &view,
            &prefs,
            TabColours::default(),
        )
    };
    let rows = rows(layer_tab_menu_entries(&I18n::en_us(), &state));
    let stack = rows.iter().find(|(l, _, _)| l == "Layer stack...");
    assert!(stack.is_some_and(|(_, message, _)| message.is_none()));
}

fn def(id: u8, name: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    }
}

/// The flyouts list every kind with its layers under it: Hide the shown
/// ones, Show the hidden ones; each layer row acts on that layer alone.
#[test]
fn the_flyouts_list_each_kind_and_its_layers() {
    let layers = [def(0, "TopCopper"), def(31, "BottomCopper")];
    let mut view = PcbLayerView::default();
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);
    view.hide(bottom);
    let i18n = I18n::en_us();
    // mockup row 104 (Caner, 2026-11-20): a HIDE or SHOW header and All layers come
    // first, then each kind's row and its layers.
    let hide_entries = layer_kind_entries(&i18n, LayerTabSubmenu::Hide, &layers, &view);
    assert!(matches!(&hide_entries[0], DropdownEntry::Header(h) if h == "HIDE"));
    let hide = rows(hide_entries);
    assert_eq!(hide[0].0, "All layers", "All layers is the first row");
    assert_eq!(hide[1].0, "Signal layers");
    assert_eq!(hide[2].0, "Top Layer", "the shown signal layer");
    assert!(!hide.iter().any(|(l, _, _)| l == "Bottom Layer"));
    assert_eq!(
        hide.iter()
            .filter(|(l, _, _)| l.ends_with(" layers"))
            .count(),
        LayerKind::ALL.len()
    );
    assert!(hide.iter().any(|(l, _, _)| l == "Top Silkscreen"));
    assert!(hide.iter().any(|(l, _, _)| l == "Silkscreen layers"));

    let show_entries = layer_kind_entries(&i18n, LayerTabSubmenu::Show, &layers, &view);
    assert!(matches!(&show_entries[0], DropdownEntry::Header(h) if h == "SHOW"));
    let show = rows(show_entries);
    assert_eq!(show[0].0, "All layers", "All layers is the first row");
    assert_eq!(
        view_event(&show[0].1),
        Some(PcbViewMsg::LayerKindVisibilitySet {
            kind: LayerKind::All,
            visible: true
        })
    );
    assert_eq!(show[2].0, "Bottom Layer", "the hidden signal layer");
    assert_eq!(
        view_event(&show[2].1),
        Some(PcbViewMsg::LayerVisibilitySet {
            layer: bottom,
            visible: true
        })
    );
    assert_eq!(
        view_event(&show[1].1),
        Some(PcbViewMsg::LayerKindVisibilitySet {
            kind: LayerKind::Signal,
            visible: true
        })
    );
}

/// The board-view event a row sends, if it sends one.
fn view_event(message: &Option<Message>) -> Option<PcbViewMsg> {
    match message {
        Some(Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Picked(event)))) => Some(*event),
        _ => None,
    }
}
