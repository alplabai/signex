//! Right-click menu of a layer tab under the board canvas (#2379, #2394):
//! hide or highlight the layer, show it alone or every layer, hide or show
//! every layer of a kind, the board editors, the tab names and the flip of
//! the board view.

use super::*;

use super::items::{dd_disabled, dd_msg, text};
use crate::app::handlers::pcb_layer_strip::LAYER_STACK_COMMAND;
use crate::app::{LayerStripMsg, LayerTabMenuMsg, LayerTabSubmenu, PcbMsg, PcbViewMsg};
use crate::layer_strip_prefs::{LayerNameStyle, LayerStripPrefs};
use signex_i18n::{Arg, I18n, MessageId, ids};
use signex_renderer::pcb::{LayerKind, PcbLayerView};
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

/// What the menu of the tab of `layer` shows: the board view it acts on,
/// the strip's settings and its open flyout.
pub(super) struct TabMenuState<'a> {
    pub layer: PcbLayer,
    pub view: &'a PcbLayerView,
    pub prefs: &'a LayerStripPrefs,
    pub submenu: Option<LayerTabSubmenu>,
    /// `board_layer_stack` (#2401) is enabled in the menu's window: the
    /// command's own rule, the one the Design menu row greys by.
    pub layer_stack_live: bool,
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

/// A greyed row for an editor that is not built, with the reason as its
/// hover text.
fn unavailable(i18n: &I18n, label: MessageId, reason: MessageId) -> DropdownEntry<Message> {
    match dd_disabled(None, &text(i18n, label), None) {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.tooltip(text(i18n, reason))),
        other => other,
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
    } = *state;
    // Layer stack... opens the layer stack manager (#2401) through its
    // catalog command; greyed when that command is.
    let layer_stack = text(i18n, ids::PCB_LAYER_TAB_LAYER_STACK);
    let layer_stack = if layer_stack_live {
        dd_msg(
            None,
            &layer_stack,
            "",
            menu_msg(LayerTabMenuMsg::LayerStackPicked),
        )
    } else {
        dd_disabled(None, &layer_stack, None)
    };
    let name = crate::pcb_canvas::layer_style::layer_name(i18n, layer).into_string();
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
    vec![
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
    ]
}

/// The rows of Hide layers ▸ or Show layers ▸: one per kind of layer.
pub(super) fn layer_kind_entries(
    i18n: &I18n,
    submenu: LayerTabSubmenu,
) -> Vec<DropdownEntry<Message>> {
    let visible = submenu == LayerTabSubmenu::Show;
    LayerKind::ALL
        .into_iter()
        .map(|kind| {
            let label =
                i18n.message_plain(ids::PCB_LAYER_KIND, &[("kind", Arg::from(kind.token()))]);
            dd_msg(
                None,
                &label,
                "",
                pick(PcbViewMsg::LayerKindVisibilitySet { kind, visible }),
            )
        })
        .collect()
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
        let state = TabMenuState {
            layer: menu.layer,
            view: &view,
            prefs: &self.ui_state.layer_strip,
            submenu: menu.submenu,
            layer_stack_live,
        };
        let i18n = &self.ui_state.i18n;
        let panel_ctx = &self.document_state.panel_ctx;
        let (tokens, font) = (&panel_ctx.tokens, &panel_ctx.ui_font_name);
        let card = super::placement::menu_card(layer_tab_menu_entries(i18n, &state), tokens, font);
        let mut anchored = super::anchored::AnchoredMenu::new(card.element, (menu.x, menu.y));
        if let (Some(submenu), Some(offset)) = (menu.submenu, card.flyout_offset) {
            let flyout =
                super::placement::menu_card(layer_kind_entries(i18n, submenu), tokens, font);
            anchored = anchored.flyout(flyout.element, offset);
        }
        Self::anchored_layers(menu_msg(LayerTabMenuMsg::Dismissed), anchored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::CopperLayer;

    fn rows(entries: Vec<DropdownEntry<Message>>) -> Vec<(String, Option<Message>, bool)> {
        entries
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some((item.label, item.on_press, item.checked)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_menu_lists_every_row_of_the_tab_menu() {
        let layer = PcbLayer::Copper(CopperLayer::Bottom);
        let view = PcbLayerView::default();
        let prefs = LayerStripPrefs::default();
        let state = TabMenuState {
            layer,
            view: &view,
            prefs: &prefs,
            submenu: None,
            layer_stack_live: true,
        };
        let rows = rows(layer_tab_menu_entries(&I18n::en_us(), &state));
        let labels: Vec<&str> = rows.iter().map(|(label, _, _)| label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Hide Bottom Layer",
                "Highlight Bottom Layer",
                "Show only this layer",
                "Show all layers",
                "Hide layers",
                "Show layers",
                "Layer stack...",
                "Drill symbols...",
                "Layer drawing order...",
                "Short tab names",
                "Medium tab names",
                "Long tab names",
                "Show layer numbers",
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
        // The editors that are not built are greyed, never a silent row.
        for label in ["Drill symbols...", "Layer drawing order..."] {
            assert!(row(label).1.is_none(), "{label} is greyed");
        }
        let long = rows.iter().find(|(l, _, _)| l == "Long tab names");
        assert!(
            long.is_some_and(|(_, _, on)| *on),
            "the default style is checked"
        );
    }

    /// #2401: Layer stack... greys exactly when `board_layer_stack` is
    /// disabled in the menu's window.
    #[test]
    fn the_layer_stack_row_follows_its_command() {
        let view = PcbLayerView::default();
        let prefs = LayerStripPrefs::default();
        let state = TabMenuState {
            layer: PcbLayer::Copper(CopperLayer::Top),
            view: &view,
            prefs: &prefs,
            submenu: None,
            layer_stack_live: false,
        };
        let rows = rows(layer_tab_menu_entries(&I18n::en_us(), &state));
        let stack = rows.iter().find(|(l, _, _)| l == "Layer stack...");
        assert!(stack.is_some_and(|(_, message, _)| message.is_none()));
    }

    #[test]
    fn the_flyouts_hide_or_show_every_kind() {
        let rows = rows(layer_kind_entries(&I18n::en_us(), LayerTabSubmenu::Hide));
        assert_eq!(rows.len(), LayerKind::ALL.len());
        assert_eq!(rows[0].0, "Signal layers");
        assert_eq!(
            view_event(&rows[4].1),
            Some(PcbViewMsg::LayerKindVisibilitySet {
                kind: LayerKind::Paste,
                visible: false
            })
        );
    }

    /// The board-view event a row sends, if it sends one.
    fn view_event(message: &Option<Message>) -> Option<PcbViewMsg> {
        match message {
            Some(Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Picked(event)))) => {
                Some(*event)
            }
            _ => None,
        }
    }
}
