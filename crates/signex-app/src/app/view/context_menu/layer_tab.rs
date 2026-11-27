//! Right-click menu of a layer tab under the board canvas (#2379, #2394,
//! mockup row 104): hide or highlight the layer, its colour (saved with the board,
//! 2026-11-18), show it alone or every layer, hide or show every layer of a
//! kind or one layer of it, the board editors, the tab names and the flip
//! of the board view. Every row has the 16 px icon column, so the labels
//! line up whether or not a row has an icon.

use super::*;

use super::items::{dd_disabled, dd_msg, text};
use crate::app::handlers::pcb_layer_strip::LAYER_STACK_COMMAND;
use crate::app::{LayerStripMsg, LayerTabMenuMsg, LayerTabSubmenu, PcbMsg, PcbViewMsg};
use crate::icons;
use crate::layer_strip_prefs::{LayerNameStyle, LayerStripPrefs};
use crate::pcb_canvas::layer_style::layer_name;
use crate::styles::tokens::resolve;
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::svg::Handle;
use iced::widget::{container, row};
use iced::{Background, Border, Length, Padding, Theme, Widget as _};
use signex_i18n::{Arg, I18n, Localized, MessageId, ids, t};
use signex_renderer::pcb::{BoardPaint, LayerKind, PcbLayerView, kind_layers};
use signex_types::pcb::LayerDef;
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, MenuMetrics, RowPaint};
use signex_widgets::{Label, theme_ext};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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
    /// The board takes no edit (its colour commands are greyed): the
    /// colours are saved with it.
    pub read_only: bool,
}

/// How the menu is drawn (mockup row 104): the theme the row icons are tinted for
/// and the colours of the Tab names switch and the Show layer numbers row.
#[derive(Clone, Copy)]
pub(super) struct TabMenuLook<'a> {
    pub theme: ThemeId,
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
fn checked_row(icon: Handle, label: String, message: Message, on: bool) -> DropdownEntry<Message> {
    DropdownEntry::Item(DropdownItem::new(label, message).icon(icon).checked(on))
}

/// A greyed row with the reason as its hover text.
fn unavailable(
    i18n: &I18n,
    icon: Handle,
    label: MessageId,
    reason: MessageId,
) -> DropdownEntry<Message> {
    match dd_disabled(Some(icon), &text(i18n, label), None) {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.tooltip(text(i18n, reason))),
        other => other,
    }
}

/// A row that sends `message` while `live`, greyed otherwise.
fn live_row(
    icon: Option<Handle>,
    label: &str,
    message: Message,
    live: bool,
) -> DropdownEntry<Message> {
    if live {
        dd_msg(icon, label, "", message)
    } else {
        dd_disabled(icon, label, None)
    }
}

/// A flyout launcher: a click or the pointer opens `submenu`; while it is
/// open the row is the selected row (accent-soft fill and ink), not
/// the active highlight.
fn launcher(label: String, submenu: LayerTabSubmenu, open: bool) -> DropdownEntry<Message> {
    let opens = menu_msg(LayerTabMenuMsg::SubmenuOpened(submenu));
    let item = DropdownItem::new(label, opens.clone())
        .submenu()
        .hover(opens, Message::noop())
        .selected(open);
    DropdownEntry::Item(item)
}

/// Whether Layer colour... can open a picker for `layer`: the board view
/// recolours it, and the Layers panel lists a row for it (a mechanical
/// layer has none yet).
fn colour_editable(layer: PcbLayer) -> bool {
    BoardPaint::can_recolour(layer) && !matches!(layer, PcbLayer::Mechanical(_))
}

/// The colour rows (2026-11-18): Layer colour..., Reset layer colour and
/// Reset all layer colours, with the net colour menu's glyphs.
fn colour_rows(
    i18n: &I18n,
    layer: PcbLayer,
    colours: TabColours,
    theme: ThemeId,
) -> Vec<DropdownEntry<Message>> {
    let edit = if colour_editable(layer) && !colours.read_only {
        dd_msg(
            Some(icons::icon_dd_net_color_custom(theme)),
            &text(i18n, ids::PCB_LAYER_TAB_COLOUR),
            "",
            menu_msg(LayerTabMenuMsg::ColourEditRequested(Some(layer))),
        )
    } else {
        unavailable(
            i18n,
            icons::icon_dd_net_color_custom(theme),
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
            Some(icons::icon_dd_net_color_clear(theme)),
            &text(i18n, ids::PCB_LAYER_TAB_COLOUR_RESET),
            pick(reset),
            colours.own && !colours.read_only,
        ),
        live_row(
            Some(icons::icon_dd_net_color_clear_all(theme)),
            &text(i18n, ids::PCB_LAYER_TAB_COLOUR_RESET_ALL),
            pick(PcbViewMsg::LayerColoursReset),
            colours.any && !colours.read_only,
        ),
    ]
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
    let style = r.segmented(TS.caption).fill_width(true);
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

/// The menu's rows drawn in `look`, so the labels and messages are tested
/// without a renderer.
pub(super) fn layer_tab_menu_rows(
    i18n: &I18n,
    state: &TabMenuState<'_>,
    look: TabMenuLook<'_>,
) -> Vec<DropdownEntry<Message>> {
    let TabMenuState {
        layer,
        view,
        prefs,
        submenu,
        layer_stack_live,
        colours,
    } = *state;
    let TabMenuLook { theme, tokens } = look;
    // Layer stack... opens the layer stack manager (#2401) through its
    // catalog command; greyed when that command is.
    let layer_stack = live_row(
        Some(icons::icon_dd_layers(theme)),
        &text(i18n, ids::PCB_LAYER_TAB_LAYER_STACK),
        menu_msg(LayerTabMenuMsg::LayerStackPicked),
        layer_stack_live,
    );
    let name = layer_name(i18n, layer).into_string();
    let with_layer = |id| i18n.message_plain(id, &[("layer", Arg::from(name.clone()))]);
    let mut rows = vec![
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
    ];
    rows.extend(colour_rows(i18n, layer, colours, theme));
    rows.extend([
        DropdownEntry::Separator,
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
    ]);
    rows
}

/// The rows of Hide layers ▸ or Show layers ▸ (mockup row 104): a HIDE or SHOW
/// header and All layers; then per kind of layer a row for every layer of
/// it, followed by the layers of that kind the row would change one by one
/// (the shown ones under Hide, the hidden ones under Show).
pub(super) fn layer_kind_entries(
    i18n: &I18n,
    submenu: LayerTabSubmenu,
    layers: &[LayerDef],
    view: &PcbLayerView,
) -> Vec<DropdownEntry<Message>> {
    let visible = submenu == LayerTabSubmenu::Show;
    let mode = if visible { "show" } else { "hide" };
    let header = i18n.message_plain(ids::PCB_LAYER_TAB_FLYOUT, &[("mode", Arg::from(mode))]);
    let kind_row = |kind: LayerKind| {
        let label = i18n.message_plain(ids::PCB_LAYER_KIND, &[("kind", Arg::from(kind.token()))]);
        let group = PcbViewMsg::LayerKindVisibilitySet { kind, visible };
        dd_msg(None, &label, "", pick(group))
    };
    let mut rows = vec![
        DropdownEntry::Header(i18n.upper_for_display(&header)),
        kind_row(LayerKind::All),
    ];
    for kind in LayerKind::ALL.into_iter().filter(|k| *k != LayerKind::All) {
        rows.push(DropdownEntry::Separator);
        rows.push(kind_row(kind));
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
        let colours_live = crate::keymap::AppCommandId::new("reset_layer_colors").is_ok_and(|id| {
            crate::app::command::is_enabled(&id, &self.command_context(menu.window))
        });
        let board = self.pcb_view_target(menu.window).map(|(_, board)| board);
        let colours = board.map_or_else(TabColours::default, |board| TabColours {
            own: board.options.layer_color(menu.layer).is_some(),
            any: !board.options.layer_colors.is_empty(),
            read_only: !colours_live,
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
        let panel_ctx = &self.document_view.panel_ctx;
        let (tokens, font) = (&panel_ctx.tokens, &panel_ctx.ui_font_name);
        let look = TabMenuLook {
            theme: self.ui_state.effective_theme_id(),
            tokens,
        };
        let rows = layer_tab_menu_rows(i18n, &state, look);
        let card = super::placement::menu_card(rows, tokens, font);
        let mut anchored = super::anchored::AnchoredMenu::new(card.element, (menu.x, menu.y));
        if let (Some(submenu), Some(offset)) = (menu.submenu, card.flyout_offset) {
            let rows = layer_kind_entries(i18n, submenu, layers, &view);
            let flyout = super::placement::menu_card(rows, tokens, font);
            anchored = anchored.flyout(flyout.element, offset);
        }
        Self::anchored_layers(menu_msg(LayerTabMenuMsg::Dismissed), anchored)
    }
}

/// The menu's rows in the Graphite Dark look: the row data the tests read.
#[cfg(test)]
pub(super) fn layer_tab_menu_entries(
    i18n: &I18n,
    state: &TabMenuState<'_>,
) -> Vec<DropdownEntry<Message>> {
    let theme = ThemeId::GraphiteDark;
    let tokens = signex_types::theme::theme_tokens(theme);
    layer_tab_menu_rows(
        i18n,
        state,
        TabMenuLook {
            theme,
            tokens: &tokens,
        },
    )
}

#[cfg(test)]
#[path = "layer_tab_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "layer_tab_look_tests.rs"]
mod look_tests;
