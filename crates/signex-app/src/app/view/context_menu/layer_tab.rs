//! Right-click menu of a layer tab under the board canvas (#2379, #2394):
//! hide or highlight the layer, its colour (saved with the board,
//! 2026-11-18), show it alone or every layer, hide or show every layer of a
//! kind or one layer of it, the board editors, the tab names and the flip
//! of the board view.

use super::*;

use super::items::{dd_disabled, dd_msg, text};
use crate::app::handlers::pcb_layer_strip::LAYER_STACK_COMMAND;
use crate::app::{LayerStripMsg, LayerTabMenuMsg, LayerTabSubmenu, PcbMsg, PcbViewMsg};
use crate::layer_strip_prefs::{LayerNameStyle, LayerStripPrefs};
use crate::pcb_canvas::layer_style::layer_name;
use signex_i18n::{Arg, I18n, MessageId, ids};
use signex_renderer::pcb::{BoardPaint, LayerKind, PcbLayerView, kind_layers};
use signex_types::pcb::LayerDef;
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

/// What the menu of the tab of `layer` shows: the board view it acts on,
/// the board's layer colours, the strip's settings and its open flyout.
pub(super) struct TabMenuState<'a> {
    pub layer: PcbLayer,
    pub view: &'a PcbLayerView,
    pub prefs: &'a LayerStripPrefs,
    pub submenu: Option<LayerTabSubmenu>,
    /// `board_layer_stack` (#2401) is enabled in the menu's window: the
    /// command's own rule, the one the Design menu row greys by.
    pub layer_stack_live: bool,
    pub colours: TabColours,
}

/// Which colours the board gives its layers, for the reset rows.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct TabColours {
    /// The tab's layer has a colour of its own.
    pub own: bool,
    /// Some layer has one.
    pub any: bool,
}

fn menu_msg(event: LayerTabMenuMsg) -> Message {
    Message::Pcb(PcbMsg::LayerTabMenu(event))
}

fn pick(event: PcbViewMsg) -> Message {
    menu_msg(LayerTabMenuMsg::Picked(event))
}

/// A row with a check mark while `on`.
fn checked_row(label: String, message: Message, on: bool) -> DropdownEntry<Message> {
    DropdownEntry::Item(DropdownItem::new(label, message).checked(on))
}

/// A greyed row with the reason as its hover text.
fn unavailable(i18n: &I18n, label: MessageId, reason: MessageId) -> DropdownEntry<Message> {
    match dd_disabled(None, &text(i18n, label), None) {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.tooltip(text(i18n, reason))),
        other => other,
    }
}

/// A row that sends `message` while `live`, greyed otherwise.
fn live_row(label: &str, message: Message, live: bool) -> DropdownEntry<Message> {
    if live {
        dd_msg(None, label, "", message)
    } else {
        dd_disabled(None, label, None)
    }
}

/// A flyout launcher: a click or the pointer opens `submenu`.
fn launcher(label: String, submenu: LayerTabSubmenu, open: bool) -> DropdownEntry<Message> {
    let opens = menu_msg(LayerTabMenuMsg::SubmenuOpened(submenu));
    let item = DropdownItem::new(label, opens.clone())
        .submenu()
        .hover(opens, Message::noop())
        .active(open);
    DropdownEntry::Item(item)
}

/// Whether Layer colour... can open a picker for `layer`: the board view
/// recolours it, and the Layers panel lists a row for it (a mechanical
/// layer has none yet).
fn colour_editable(layer: PcbLayer) -> bool {
    BoardPaint::can_recolour(layer) && !matches!(layer, PcbLayer::Mechanical(_))
}

/// The colour rows (2026-11-18): Layer colour..., Reset layer colour and
/// Reset all layer colours.
fn colour_rows(i18n: &I18n, layer: PcbLayer, colours: TabColours) -> Vec<DropdownEntry<Message>> {
    let edit = if colour_editable(layer) {
        dd_msg(
            None,
            &text(i18n, ids::PCB_LAYER_TAB_COLOUR),
            "",
            menu_msg(LayerTabMenuMsg::ColourEditRequested(Some(layer))),
        )
    } else {
        unavailable(
            i18n,
            ids::PCB_LAYER_TAB_COLOUR,
            ids::PCB_LAYER_TAB_COLOUR_UNAVAILABLE,
        )
    };
    let reset = PcbViewMsg::LayerColourSet {
        layer,
        colour: None,
    };
    vec![
        edit,
        live_row(
            &text(i18n, ids::PCB_LAYER_TAB_COLOUR_RESET),
            pick(reset),
            colours.own,
        ),
        live_row(
            &text(i18n, ids::PCB_LAYER_TAB_COLOUR_RESET_ALL),
            pick(PcbViewMsg::LayerColoursReset),
            colours.any,
        ),
    ]
}

/// Pure row data for the menu, so the labels and messages are tested
/// without a renderer.
pub(super) fn layer_tab_menu_entries(
    i18n: &I18n,
    state: &TabMenuState<'_>,
) -> Vec<DropdownEntry<Message>> {
    let TabMenuState {
        layer,
        view,
        prefs,
        submenu,
        layer_stack_live,
        colours,
    } = *state;
    // Layer stack... opens the layer stack manager (#2401) through its
    // catalog command; greyed when that command is.
    let layer_stack = text(i18n, ids::PCB_LAYER_TAB_LAYER_STACK);
    let layer_stack = live_row(
        &layer_stack,
        menu_msg(LayerTabMenuMsg::LayerStackPicked),
        layer_stack_live,
    );
    let name = layer_name(i18n, layer).into_string();
    let with_layer = |id| i18n.message_plain(id, &[("layer", Arg::from(name.clone()))]);
    let setting = |s| menu_msg(LayerTabMenuMsg::SettingPicked(s));
    let style_row = |label, style: LayerNameStyle| {
        let on = prefs.name_style == style;
        checked_row(
            text(i18n, label),
            setting(LayerStripMsg::NameStyleSet(style)),
            on,
        )
    };
    let mut rows = vec![
        dd_msg(
            None,
            &with_layer(ids::PCB_LAYER_TAB_HIDE),
            "",
            pick(PcbViewMsg::LayerHidden(layer)),
        ),
        checked_row(
            with_layer(ids::PCB_LAYER_TAB_HIGHLIGHT),
            pick(PcbViewMsg::LayerHighlightToggled(layer)),
            view.highlighted() == Some(layer),
        ),
    ];
    rows.extend(colour_rows(i18n, layer, colours));
    rows.extend([
        DropdownEntry::Separator,
        dd_msg(
            None,
            &text(i18n, ids::PCB_LAYER_TAB_SHOW_ONLY),
            "",
            pick(PcbViewMsg::OnlyLayerShown(layer)),
        ),
        dd_msg(
            None,
            &text(i18n, ids::PCB_LAYERS_SHOW_ALL),
            "",
            pick(PcbViewMsg::AllLayersShown),
        ),
        DropdownEntry::Separator,
        launcher(
            text(i18n, ids::PCB_LAYER_TAB_HIDE_LAYERS),
            LayerTabSubmenu::Hide,
            submenu == Some(LayerTabSubmenu::Hide),
        ),
        launcher(
            text(i18n, ids::PCB_LAYER_TAB_SHOW_LAYERS),
            LayerTabSubmenu::Show,
            submenu == Some(LayerTabSubmenu::Show),
        ),
        DropdownEntry::Separator,
        layer_stack,
        unavailable(
            i18n,
            ids::PCB_LAYER_TAB_DRILL_SYMBOLS,
            ids::PCB_LAYER_TAB_DRILL_SYMBOLS_UNAVAILABLE,
        ),
        unavailable(
            i18n,
            ids::PCB_LAYER_TAB_DRAWING_ORDER,
            ids::PCB_LAYER_TAB_DRAWING_ORDER_UNAVAILABLE,
        ),
        DropdownEntry::Separator,
        style_row(ids::PCB_LAYER_TAB_NAMES_SHORT, LayerNameStyle::Short),
        style_row(ids::PCB_LAYER_TAB_NAMES_MEDIUM, LayerNameStyle::Medium),
        style_row(ids::PCB_LAYER_TAB_NAMES_LONG, LayerNameStyle::Long),
        checked_row(
            text(i18n, ids::PCB_LAYER_TAB_SHOW_NUMBERS),
            setting(LayerStripMsg::LayerNumbersToggled),
            prefs.show_numbers,
        ),
        DropdownEntry::Separator,
        checked_row(
            text(i18n, ids::PCB_LAYER_TAB_FLIP),
            pick(PcbViewMsg::BoardFlipToggled),
            view.flipped(),
        ),
    ]);
    rows
}

/// The rows of Hide layers ▸ or Show layers ▸: per kind of layer a row for
/// every layer of it, then the layers of that kind the row would change
/// one by one (the shown ones under Hide, the hidden ones under Show);
/// last, All layers.
pub(super) fn layer_kind_entries(
    i18n: &I18n,
    submenu: LayerTabSubmenu,
    layers: &[LayerDef],
    view: &PcbLayerView,
) -> Vec<DropdownEntry<Message>> {
    let visible = submenu == LayerTabSubmenu::Show;
    let mut rows = Vec::new();
    for kind in LayerKind::ALL {
        if !rows.is_empty() {
            rows.push(DropdownEntry::Separator);
        }
        let label = i18n.message_plain(ids::PCB_LAYER_KIND, &[("kind", Arg::from(kind.token()))]);
        let group = PcbViewMsg::LayerKindVisibilitySet { kind, visible };
        rows.push(dd_msg(None, &label, "", pick(group)));
        if kind == LayerKind::All {
            continue;
        }
        let changed = kind_layers(layers, kind)
            .into_iter()
            .filter(|&layer| view.is_hidden(layer) == visible);
        for layer in changed {
            let one = PcbViewMsg::LayerVisibilitySet { layer, visible };
            let name = layer_name(i18n, layer).into_string();
            rows.push(dd_msg(None, &name, "", pick(one)));
        }
    }
    rows
}

impl Signex {
    /// The open layer tab menu with its click-outside backdrop and its open
    /// flyout, in the window it was opened in; the open layer set menu
    /// (#2394) in its place; empty otherwise.
    pub(in crate::app::view) fn layer_tab_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        if self.interaction_state.pcb_layer_set_menu.is_some() {
            return self.layer_set_menu_overlay(window_id);
        }
        let Some(menu) = self.interaction_state.pcb_layer_tab_menu else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(menu.window, window_id) {
            return Vec::new();
        }
        let view = self.pcb_layer_view_of_window(menu.window);
        let layer_stack_live =
            crate::keymap::AppCommandId::new(LAYER_STACK_COMMAND).is_ok_and(|id| {
                crate::app::command::is_enabled(&id, &self.command_context(menu.window))
            });
        let board = self.pcb_view_target(menu.window).map(|(_, board)| board);
        let colours = board.map_or_else(TabColours::default, |board| TabColours {
            own: board.options.layer_color(menu.layer).is_some(),
            any: !board.options.layer_colors.is_empty(),
        });
        let layers: &[LayerDef] = board.map(|b| b.layers.as_slice()).unwrap_or_default();
        let state = TabMenuState {
            layer: menu.layer,
            view: &view,
            prefs: &self.ui_state.layer_strip,
            submenu: menu.submenu,
            layer_stack_live,
            colours,
        };
        let i18n = &self.ui_state.i18n;
        let panel_ctx = &self.document_state.panel_ctx;
        let (tokens, font) = (&panel_ctx.tokens, &panel_ctx.ui_font_name);
        let card = super::placement::menu_card(layer_tab_menu_entries(i18n, &state), tokens, font);
        let mut anchored = super::anchored::AnchoredMenu::new(card.element, (menu.x, menu.y));
        if let (Some(submenu), Some(offset)) = (menu.submenu, card.flyout_offset) {
            let rows = layer_kind_entries(i18n, submenu, layers, &view);
            let flyout = super::placement::menu_card(rows, tokens, font);
            anchored = anchored.flyout(flyout.element, offset);
        }
        Self::anchored_layers(menu_msg(LayerTabMenuMsg::Dismissed), anchored)
    }
}

#[cfg(test)]
#[path = "layer_tab_tests.rs"]
mod tests;
