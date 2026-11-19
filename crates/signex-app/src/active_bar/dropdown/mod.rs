//! Active Bar dropdown menus — data-driven. Each `ActiveBarMenu` builds
//! pure `DropdownEntry` rows that the shared
//! `signex_widgets::active_bar_dropdown` widget renders, so the
//! schematic, footprint, and future PCB active bars share one dropdown
//! renderer (see ADR-0003). Enable/disable is folded into each
//! `DropdownItem` at build time, so no render-time selection guard is
//! needed here.

use iced::Element;
use iced::widget::svg;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use super::{
    ActiveBarAction, ActiveBarMenu, ActiveBarMsg, CustomFilterPreset, SelectionFilter,
    action_enabled,
};
pub(crate) use custom::{FilterChip, FilterMenu, filter_menu_entry, preset_label};
use custom::{filter_entry, net_color_entries};
#[cfg(test)]
use tables::render;
use tables::{
    ALIGN, DIRECTIVES, EntrySpec, HARNESS, PORT, POWER, SELECT, SELECT_MODE, SHAPES, SHEET_SYMBOL,
    TEXT_TOOLS, WIRING, render_with_hints,
};

mod custom;
mod graphite;
mod tables;
#[cfg(test)]
mod tests;

// ─── View: Dropdown menus ────────────────────────────────────

/// Render the dropdown menu for the given Active Bar button. `hint`
/// resolves a command id to the active keymap profile's shortcut text.
#[expect(
    clippy::too_many_arguments,
    reason = "the language, the menu, the bar's state and the keymap are separate inputs of one dropdown"
)]
pub fn view_dropdown<'a>(
    i18n: &signex_i18n::I18n,
    menu: ActiveBarMenu,
    tokens: &'a ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
    tid: ThemeId,
    has_selection: bool,
    has_net_colors: bool,
    hint: &dyn Fn(&str) -> String,
) -> Element<'a, ActiveBarMsg> {
    let state = MenuState {
        tokens,
        filters,
        custom_presets,
        tid,
        sel: has_selection,
        nc: has_net_colors,
    };
    let entries = dropdown_entries_with(i18n, menu, &state, hint);
    signex_widgets::active_bar_dropdown::view(entries, tokens, dropdown_min_width(menu))
}

/// The bar state one dropdown is built from.
struct MenuState<'s> {
    tokens: &'s ThemeTokens,
    filters: &'s std::collections::HashSet<SelectionFilter>,
    custom_presets: &'s [CustomFilterPreset],
    tid: ThemeId,
    sel: bool,
    nc: bool,
}

/// One icon + label dropdown row. Disabled rows drop their `on_press`
/// (the widget greys the row and ignores clicks).
fn dd_item(
    i18n: &signex_i18n::I18n,
    icon: svg::Handle,
    label: &'static str,
    action: ActiveBarAction,
    has_selection: bool,
    has_net_colors: bool,
) -> DropdownEntry<ActiveBarMsg> {
    let enabled = action_enabled(&action, has_selection, has_net_colors);
    // #271 — the visible text comes from the command catalog when the
    // action has an id, so the Active Bar stops being the only surface
    // keeping its own copy of a command's wording. `label` is the
    // fallback for actions still without an id (the parameterised
    // families awaiting `CommandArgs`), and a test pins the catalog text
    // to these literals so nothing the user sees moves.
    let label = crate::app::command::active_bar::action_label(i18n, &action, label);
    let planned = super::is_planned(&action);
    let mut item = DropdownItem::new(label, ActiveBarMsg::Action(action))
        .icon(icon)
        .disabled(!enabled);
    if planned {
        item = item.tooltip(i18n.message_plain(signex_i18n::menu_ids::MENU_PLANNED_TOOLTIP, &[]));
    }
    DropdownEntry::Item(item)
}

/// The uniform table behind `menu`, `None` for the two irregular menus
/// (Filter chip grid, Net Color swatches).
fn menu_table(menu: ActiveBarMenu) -> Option<&'static [EntrySpec]> {
    Some(match menu {
        ActiveBarMenu::Filter | ActiveBarMenu::NetColor => return None,
        ActiveBarMenu::SelectMode => SELECT_MODE,
        ActiveBarMenu::Select => SELECT,
        ActiveBarMenu::Align => ALIGN,
        ActiveBarMenu::Wiring => WIRING,
        ActiveBarMenu::Power => POWER,
        ActiveBarMenu::Harness => HARNESS,
        ActiveBarMenu::SheetSymbol => SHEET_SYMBOL,
        ActiveBarMenu::Port => PORT,
        ActiveBarMenu::Directives => DIRECTIVES,
        ActiveBarMenu::TextTools => TEXT_TOOLS,
        ActiveBarMenu::Shapes => SHAPES,
    })
}

/// Whether every row of `menu` is a planned tool: the bar greys its button
/// and opens no menu whose rows would all say "planned".
pub(super) fn is_planned_menu(menu: ActiveBarMenu) -> bool {
    menu_table(menu).is_some_and(|specs| {
        let mut actions = specs.iter().filter_map(EntrySpec::action).peekable();
        actions.peek().is_some() && actions.all(super::is_planned)
    })
}

/// [`dropdown_entries_with`] with no shortcut hints, as the golden pins it.
#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "the language, the menu and the bar's state are separate inputs of one dropdown"
)]
fn dropdown_entries(
    i18n: &signex_i18n::I18n,
    menu: ActiveBarMenu,
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
    tid: ThemeId,
    has_selection: bool,
    has_net_colors: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let state = MenuState {
        tokens,
        filters,
        custom_presets,
        tid,
        sel: has_selection,
        nc: has_net_colors,
    };
    dropdown_entries_with(i18n, menu, &state, &|_| String::new())
}

/// Route each `ActiveBarMenu` to its entries. Uniform menus resolve
/// through the `EntrySpec` data table; the Filter chip grid rides the
/// widget's `Custom` escape hatch and the Net Color menu keeps its own
/// builder.
fn dropdown_entries_with(
    i18n: &signex_i18n::I18n,
    menu: ActiveBarMenu,
    s: &MenuState<'_>,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    match (menu, menu_table(menu)) {
        (_, Some(specs)) => render_with_hints(i18n, specs, s.tid, s.sel, s.nc, hint),
        (ActiveBarMenu::Filter, None) => {
            vec![filter_entry(i18n, s.tokens, s.filters, s.custom_presets)]
        }
        (_, None) => net_color_entries(i18n, s.tokens, s.tid, s.sel, s.nc),
    }
}

/// Pinned column width per dropdown menu.
///
/// `view_dropdown` wraps the items column in a `Length::Fixed(W)`
/// container using this value. That bound lets each item set
/// `button.width(Length::Fill)` (so the hover background covers the
/// full row) without `Fill` propagating to the viewport — which is the
/// `Length::Fill`-inside-`Length::Shrink` trap iced 0.14 falls into.
///
/// Widths are sized to the longest label in each menu (Inter @ 12 +
/// the 16 px icon column, row padding and the shortcut column).
/// `Filter` returns `None` because its chip wrap layout already drives
/// its own width.
fn dropdown_min_width(menu: ActiveBarMenu) -> Option<f32> {
    Some(match menu {
        ActiveBarMenu::Filter => return None,
        // "Flip Selected Sheet Symbols Along X" (36 chars)
        ActiveBarMenu::Select => 320.0,
        // "Touching Rectangle" (18)
        ActiveBarMenu::SelectMode => 195.0,
        // "Align Horizontal Centers" (24)
        ActiveBarMenu::Align => 235.0,
        // "Net Label" plus its key hint.
        ActiveBarMenu::Wiring => 200.0,
        // "Signal Ground" (13), as drawn.
        ActiveBarMenu::Power => 214.0,
        // "Harness Connector" (17)
        ActiveBarMenu::Harness => 195.0,
        // "Device Sheet Symbol" (19)
        ActiveBarMenu::SheetSymbol => 205.0,
        // "Off Sheet Connector" (19)
        ActiveBarMenu::Port => 205.0,
        // "Differential Pair" (17)
        ActiveBarMenu::Directives => 195.0,
        // "Text String" (11)
        ActiveBarMenu::TextTools => 150.0,
        // "Round Rectangle" (15)
        ActiveBarMenu::Shapes => 180.0,
        // "Clear All Net Colors" (20), as drawn.
        ActiveBarMenu::NetColor => 212.0,
    })
}

/// Horizontal offset (in px) to align dropdown below a given button index.
pub fn dropdown_x_offset(menu: ActiveBarMenu, m: &signex_widgets::active_bar::Metrics) -> f32 {
    super::layout::trigger_left(menu, m)
}
