//! Right-click menu of a layer tab under the board canvas (#2379, #2394,
//! #104): hide or highlight the layer, show it alone or every layer, hide or
//! show every layer of a kind, the board editors, the tab names and the flip
//! of the board view. Every row has the 16 px icon column, so the labels
//! line up whether or not a row has an icon.

use super::*;

use super::items::{dd_disabled, dd_msg, text};
use crate::app::handlers::pcb_layer_strip::LAYER_STACK_COMMAND;
use crate::app::{LayerStripMsg, LayerTabMenuMsg, LayerTabSubmenu, PcbMsg, PcbViewMsg};
use crate::icons;
use crate::layer_strip_prefs::{LayerNameStyle, LayerStripPrefs};
use crate::styles::{ti, tokens::resolve};
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{container, row};
use iced::{Background, Border, Length, Padding, Theme, Widget as _};
use signex_i18n::{Arg, I18n, Localized, MessageId, ids, t};
use signex_renderer::pcb::{LayerKind, PcbLayerView};
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, MenuMetrics, RowPaint};
use signex_widgets::{Label, SegmentedStyle, theme_ext};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the menu of the tab of `layer` shows: the board view it acts on,
/// the strip's settings and its open flyout, in the look of `theme`.
pub(super) struct TabMenuState<'a> {
    pub layer: PcbLayer,
    pub view: &'a PcbLayerView,
    pub prefs: &'a LayerStripPrefs,
    pub submenu: Option<LayerTabSubmenu>,
    /// `board_layer_stack` (#2401) is enabled in the menu's window: the
    /// command's own rule, the one the Design menu row greys by.
    pub layer_stack_live: bool,
    /// The theme the row icons are tinted for.
    pub theme: ThemeId,
    /// Colours of the Tab names switch and the Show layer numbers row.
    pub tokens: &'a ThemeTokens,
}

fn menu_msg(event: LayerTabMenuMsg) -> Message {
    Message::Pcb(PcbMsg::LayerTabMenu(event))
}

fn pick(event: PcbViewMsg) -> Message {
    menu_msg(LayerTabMenuMsg::Picked(event))
}

fn setting(event: LayerStripMsg) -> Message {
    menu_msg(LayerTabMenuMsg::SettingPicked(event))
}

/// A row with `icon` in the icon column and a check mark while `on`.
fn checked_row(
    icon: iced::widget::svg::Handle,
    label: String,
    message: Message,
    on: bool,
) -> DropdownEntry<Message> {
    DropdownEntry::Item(DropdownItem::new(label, message).icon(icon).checked(on))
}

/// A greyed row for an editor that is not built, with the reason as its
/// hover text.
fn unavailable(
    i18n: &I18n,
    icon: iced::widget::svg::Handle,
    label: MessageId,
    reason: MessageId,
) -> DropdownEntry<Message> {
    match dd_disabled(Some(icon), &text(i18n, label), None) {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.tooltip(text(i18n, reason))),
        other => other,
    }
}

/// A flyout launcher: a click or the pointer opens `submenu`; while it is
/// open the row keeps the current-row look.
fn launcher(label: String, submenu: LayerTabSubmenu, open: bool) -> DropdownEntry<Message> {
    let opens = menu_msg(LayerTabMenuMsg::SubmenuOpened(submenu));
    let item = DropdownItem::new(label, opens.clone())
        .submenu()
        .hover(opens, Message::noop())
        .active(open)
        .current(open);
    DropdownEntry::Item(item)
}

/// The catalog name of a tab name style: Short, Medium or Long.
fn style_name(i18n: &I18n, style: LayerNameStyle) -> Localized {
    t!(i18n, ids::PCB_LAYER_TAB_NAME_STYLE, "style" => style.token())
}

/// The cells of the Tab names switch: each style's name, the style, and
/// whether it is the one the tabs use.
pub(super) fn tab_name_cells(
    i18n: &I18n,
    prefs: &LayerStripPrefs,
) -> Vec<(String, LayerNameStyle, bool)> {
    LayerNameStyle::ALL
        .into_iter()
        .map(|style| {
            let name = i18n.message_plain(
                ids::PCB_LAYER_TAB_NAME_STYLE,
                &[("style", Arg::from(style.token()))],
            );
            (name, style, prefs.name_style == style)
        })
        .collect()
}

/// Left edge of a row label: the row padding, the icon column and the gap
/// after it.
fn label_inset(m: &MenuMetrics) -> f32 {
    m.row_pad_x + m.icon_size + m.row_spacing
}

/// The Tab names switch, Short | Medium | Long, lined up with the labels. A
/// cell closes the menu and sets the style.
fn tab_names_row(
    i18n: &I18n,
    prefs: &LayerStripPrefs,
    tokens: &ThemeTokens,
) -> DropdownEntry<Message> {
    let m = MenuMetrics::GRAPHITE;
    let r = resolve(tokens);
    let style = SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(tokens.text),
        TS.caption,
    )
    .fill_width(true);
    let cells: Vec<(LayerNameStyle, Label<'static>)> = tab_name_cells(i18n, prefs)
        .into_iter()
        .map(|(_, style, _)| (style, style_name(i18n, style).into()))
        .collect();
    let switch = signex_widgets::segmented(cells, prefs.name_style, style, |style| {
        setting(LayerStripMsg::NameStyleSet(style))
    });
    let inset = Padding {
        top: SP.xs,
        right: m.row_pad_x,
        bottom: SP.s,
        left: label_inset(&m),
    };
    DropdownEntry::Custom(container(switch).padding(inset).width(Length::Fill).boxed())
}

/// Show layer numbers: a checkbox in the icon column and the label, the
/// whole row one button with the menu row's hover.
fn layer_numbers_row(i18n: &I18n, on: bool, tokens: &ThemeTokens) -> DropdownEntry<Message> {
    let m = MenuMetrics::GRAPHITE;
    let paint = RowPaint::of(tokens);
    let ink = theme_ext::text_primary(tokens);
    // No toggle of its own: the row's button takes the click.
    let tick = signex_widgets::checkbox(on)
        .size(TS.body)
        .style(crate::panels::form_look::checkbox_style);
    let cells = row![
        container(tick)
            .width(m.icon_size)
            .align_x(iced::alignment::Horizontal::Center)
            .boxed(),
        signex_widgets::text(t!(i18n, ids::PCB_LAYER_TAB_SHOW_NUMBERS))
            .size(m.label_size)
            .color(ink)
            .wrapping(signex_widgets::text::Wrapping::None)
            .boxed(),
    ]
    .spacing(m.row_spacing)
    .align_y(iced::Alignment::Center);
    let line = container(cells)
        .padding([0.0, m.row_pad_x])
        .height(m.item_height(true))
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill);
    let row_button = signex_widgets::button(line)
        .padding(0)
        .width(Length::Fill)
        .style(move |_: &Theme, status| ButtonStyle {
            background: matches!(status, Status::Hovered | Status::Pressed)
                .then_some(Background::Color(paint.hover)),
            border: Border {
                radius: paint.radius.into(),
                ..Border::default()
            },
            text_color: ink,
            ..ButtonStyle::default()
        })
        .on_press(setting(LayerStripMsg::LayerNumbersToggled));
    DropdownEntry::Custom(row_button.boxed())
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
        theme,
        tokens,
    } = *state;
    // Layer stack... opens the layer stack manager (#2401) through its
    // catalog command; greyed when that command is.
    let layer_stack = text(i18n, ids::PCB_LAYER_TAB_LAYER_STACK);
    let stack_icon = Some(icons::icon_dd_layers(theme));
    let layer_stack = if layer_stack_live {
        dd_msg(
            stack_icon,
            &layer_stack,
            "",
            menu_msg(LayerTabMenuMsg::LayerStackPicked),
        )
    } else {
        dd_disabled(stack_icon, &layer_stack, None)
    };
    let name = crate::pcb_canvas::layer_style::layer_name(i18n, layer).into_string();
    let with_layer = |id| i18n.message_plain(id, &[("layer", Arg::from(name.clone()))]);
    vec![
        dd_msg(
            Some(icons::icon_dd_eye_off(theme)),
            &with_layer(ids::PCB_LAYER_TAB_HIDE),
            "",
            pick(PcbViewMsg::LayerHidden(layer)),
        ),
        checked_row(
            icons::icon_dd_highlight(theme),
            with_layer(ids::PCB_LAYER_TAB_HIGHLIGHT),
            pick(PcbViewMsg::LayerHighlightToggled(layer)),
            view.highlighted() == Some(layer),
        ),
        dd_msg(
            Some(icons::icon_dd_eye(theme)),
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
            icons::icon_dd_drill(theme),
            ids::PCB_LAYER_TAB_DRILL_SYMBOLS,
            ids::PCB_LAYER_TAB_DRILL_SYMBOLS_UNAVAILABLE,
        ),
        unavailable(
            i18n,
            icons::icon_dd_draw_order(theme),
            ids::PCB_LAYER_TAB_DRAWING_ORDER,
            ids::PCB_LAYER_TAB_DRAWING_ORDER_UNAVAILABLE,
        ),
        DropdownEntry::Separator,
        DropdownEntry::Header(i18n.upper_for_display(&text(i18n, ids::PCB_LAYER_TAB_NAMES))),
        tab_names_row(i18n, prefs, tokens),
        layer_numbers_row(i18n, prefs.show_numbers, tokens),
        checked_row(
            icons::icon_dd_flip_x(theme),
            text(i18n, ids::PCB_LAYER_TAB_FLIP),
            pick(PcbViewMsg::BoardFlipToggled),
            view.flipped(),
        ),
    ]
}

/// The rows of Hide layers ▸ or Show layers ▸: a HIDE or SHOW header, All
/// layers, then one row per kind of layer.
pub(super) fn layer_kind_entries(
    i18n: &I18n,
    submenu: LayerTabSubmenu,
) -> Vec<DropdownEntry<Message>> {
    let visible = submenu == LayerTabSubmenu::Show;
    let mode = if visible { "show" } else { "hide" };
    let header = i18n.message_plain(ids::PCB_LAYER_TAB_FLYOUT, &[("mode", Arg::from(mode))]);
    let kind_row = |kind: LayerKind| {
        let label = i18n.message_plain(ids::PCB_LAYER_KIND, &[("kind", Arg::from(kind.token()))]);
        dd_msg(
            None,
            &label,
            "",
            pick(PcbViewMsg::LayerKindVisibilitySet { kind, visible }),
        )
    };
    let mut entries = vec![
        DropdownEntry::Header(i18n.upper_for_display(&header)),
        kind_row(LayerKind::All),
        DropdownEntry::Separator,
    ];
    entries.extend(
        LayerKind::ALL
            .into_iter()
            .filter(|kind| *kind != LayerKind::All)
            .map(kind_row),
    );
    entries
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
        let i18n = &self.ui_state.i18n;
        let panel_ctx = &self.document_state.panel_ctx;
        let (tokens, font) = (&panel_ctx.tokens, &panel_ctx.ui_font_name);
        let state = TabMenuState {
            layer: menu.layer,
            view: &view,
            prefs: &self.ui_state.layer_strip,
            submenu: menu.submenu,
            layer_stack_live,
            theme: self.ui_state.effective_theme_id(),
            tokens,
        };
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
        tokens: &'a ThemeTokens,
    ) -> TabMenuState<'a> {
        TabMenuState {
            layer,
            view,
            prefs,
            submenu: None,
            layer_stack_live: true,
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
        let entries = layer_tab_menu_entries(&i18n, &state(layer, &view, &prefs, &tokens));
        // #104: Tab names is a header over one switch, Show layer numbers a
        // checkbox row, and Flip follows it with no separator.
        assert_eq!(shape(&entries), "IIIISIISIIISHCCI");
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

    /// The open flyout's launcher keeps the current-row look; the other
    /// launcher does not.
    #[test]
    fn the_open_launcher_is_the_current_row() {
        let view = PcbLayerView::default();
        let prefs = LayerStripPrefs::default();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let mut state = state(PcbLayer::Copper(CopperLayer::Top), &view, &prefs, &tokens);
        state.submenu = Some(LayerTabSubmenu::Show);
        let looks: Vec<(String, bool, bool)> = layer_tab_menu_entries(&I18n::en_us(), &state)
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) if item.submenu => {
                    Some((item.label, item.active, item.current))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            looks,
            [
                ("Hide layers".to_owned(), false, false),
                ("Show layers".to_owned(), true, true),
            ]
        );
    }

    /// #2401: Layer stack... greys exactly when `board_layer_stack` is
    /// disabled in the menu's window.
    #[test]
    fn the_layer_stack_row_follows_its_command() {
        let view = PcbLayerView::default();
        let prefs = LayerStripPrefs::default();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let mut state = state(PcbLayer::Copper(CopperLayer::Top), &view, &prefs, &tokens);
        state.layer_stack_live = false;
        let rows = rows(layer_tab_menu_entries(&I18n::en_us(), &state));
        let stack = rows.iter().find(|(l, _, _)| l == "Layer stack...");
        assert!(stack.is_some_and(|(_, message, _)| message.is_none()));
    }

    #[test]
    fn the_flyouts_hide_or_show_every_kind() {
        let i18n = I18n::en_us();
        let entries = layer_kind_entries(&i18n, LayerTabSubmenu::Hide);
        // #104: a header, All layers first, a separator, then the kinds.
        assert!(matches!(&entries[0], DropdownEntry::Header(h) if h == "HIDE"));
        assert!(matches!(entries[2], DropdownEntry::Separator));
        let rows = rows(entries);
        assert_eq!(rows.len(), LayerKind::ALL.len());
        let labels: Vec<&str> = rows.iter().map(|(label, _, _)| label.as_str()).collect();
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
            view_event(&rows[0].1),
            Some(PcbViewMsg::LayerKindVisibilitySet {
                kind: LayerKind::All,
                visible: false
            })
        );
        assert_eq!(
            view_event(&rows[5].1),
            Some(PcbViewMsg::LayerKindVisibilitySet {
                kind: LayerKind::Paste,
                visible: false
            })
        );

        let shown = layer_kind_entries(&i18n, LayerTabSubmenu::Show);
        assert!(matches!(&shown[0], DropdownEntry::Header(h) if h == "SHOW"));
        assert_eq!(
            view_event(&rows_of(shown)[3]),
            Some(PcbViewMsg::LayerKindVisibilitySet {
                kind: LayerKind::Overlay,
                visible: true
            })
        );
    }

    /// The messages of the item rows of `entries`, in order.
    fn rows_of(entries: Vec<DropdownEntry<Message>>) -> Vec<Option<Message>> {
        rows(entries).into_iter().map(|(_, m, _)| m).collect()
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
