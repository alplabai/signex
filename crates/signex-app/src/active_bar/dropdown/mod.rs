//! Active Bar dropdown menus — data-driven. Each `ActiveBarMenu` builds
//! pure `DropdownEntry` rows that the shared
//! `signex_widgets::active_bar_dropdown` widget renders, so the
//! schematic, footprint, and future PCB active bars share one dropdown
//! renderer (see ADR-0003). Enable/disable is folded into each
//! `DropdownItem` at build time, so no render-time selection guard is
//! needed here.

use iced::widget::{button, svg};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, RowPaint};

use super::{
    ActiveBarAction, ActiveBarMenu, ActiveBarMsg, CustomFilterPreset, SelectionFilter,
    action_enabled,
};
use custom::{filter_entry, net_color_entries};
use tables::{
    ALIGN, DIRECTIVES, HARNESS, PORT, POWER, SELECT, SELECT_MODE, SHAPES, SHEET_SYMBOL, TEXT_TOOLS,
    WIRING, render,
};

mod custom;
mod graphite;
mod tables;
#[cfg(test)]
mod tests;

// ─── View: Dropdown menus ────────────────────────────────────

/// Render the dropdown menu for the given Active Bar button.
pub fn view_dropdown<'a>(
    menu: ActiveBarMenu,
    tokens: &'a ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
    tid: ThemeId,
    has_selection: bool,
    has_net_colors: bool,
) -> Element<'a, ActiveBarMsg> {
    // Data-driven: each menu produces pure `DropdownEntry` rows and the
    // shared `signex_widgets::active_bar_dropdown` widget renders them, so
    // the schematic, footprint, and future PCB active bars share ONE
    // dropdown widget (see ADR-0003 — active_bar / menus data-driven
    // redesign). Enable/disable is folded into each `DropdownItem` here
    // from the passed booleans — #633 removed the render-time
    // thread-locals the bar buttons used to share.
    let entries = dropdown_entries(
        menu,
        tokens,
        filters,
        custom_presets,
        tid,
        has_selection,
        has_net_colors,
    );
    signex_widgets::active_bar_dropdown::view(entries, tokens, dropdown_min_width(menu))
}

/// One icon + label dropdown row. Disabled rows drop their `on_press`
/// (the widget greys the row and ignores clicks — Altium parity).
fn dd_item(
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
    let label = crate::app::command::active_bar::action_label(&action, label);
    let planned = super::is_planned(&action);
    let mut item = DropdownItem::new(label, ActiveBarMsg::Action(action))
        .icon(icon)
        .disabled(!enabled);
    if planned {
        item = item.tooltip(super::PLANNED_TOOLTIP);
    }
    DropdownEntry::Item(item)
}

/// Route each `ActiveBarMenu` to its entries. Uniform menus resolve
/// through the `EntrySpec` data table + `render` below; the two
/// irregular menus (Filter chip grid and NetColor swatches) use the
/// widget's `Custom` escape hatch and keep their own builder.
fn dropdown_entries(
    menu: ActiveBarMenu,
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
    tid: ThemeId,
    has_selection: bool,
    has_net_colors: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let sel = has_selection;
    let nc = has_net_colors;
    match menu {
        ActiveBarMenu::Filter => vec![filter_entry(tokens, filters, custom_presets)],
        ActiveBarMenu::SelectMode => render(SELECT_MODE, tid, sel, nc),
        ActiveBarMenu::Select => render(SELECT, tid, sel, nc),
        ActiveBarMenu::Align => render(ALIGN, tid, sel, nc),
        ActiveBarMenu::Wiring => render(WIRING, tid, sel, nc),
        ActiveBarMenu::Power => render(POWER, tid, sel, nc),
        ActiveBarMenu::Harness => render(HARNESS, tid, sel, nc),
        ActiveBarMenu::SheetSymbol => render(SHEET_SYMBOL, tid, sel, nc),
        ActiveBarMenu::Port => render(PORT, tid, sel, nc),
        ActiveBarMenu::Directives => render(DIRECTIVES, tid, sel, nc),
        ActiveBarMenu::TextTools => render(TEXT_TOOLS, tid, sel, nc),
        ActiveBarMenu::Shapes => render(SHAPES, tid, sel, nc),
        ActiveBarMenu::NetColor => net_color_entries(tokens, tid, sel, nc),
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
/// Widths are sized to the longest label in each menu (Inter @ 13 +
/// 28 px icon column + 24 px button padding + a small safety margin).
/// `Filter` returns `None` because its chip wrap layout already drives
/// its own width.
fn dropdown_min_width(menu: ActiveBarMenu) -> Option<f32> {
    // Width formula: ~7 px/char × longest_label + 60 px overhead
    // (24 px button padding + 20 px icon column + 8 px spacing +
    //  small safety). Inter @ 13 px averages ~7 px/char, about 7% wider than the
    // Roboto these were first measured in.
    Some(match menu {
        ActiveBarMenu::Filter => return None,
        // "Flip Selected Sheet Symbols Along X" (36 chars)
        ActiveBarMenu::Select => 320.0,
        // "Touching Rectangle" (18)
        ActiveBarMenu::SelectMode => 195.0,
        // "Align Horizontal Centers" (24)
        ActiveBarMenu::Align => 235.0,
        // "Net Label" (9) — keep a usable minimum.
        ActiveBarMenu::Wiring => 150.0,
        // "Place Signal Ground power port" (30)
        ActiveBarMenu::Power => 280.0,
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
        // "Clear All Net Colors" (20)
        ActiveBarMenu::NetColor => 215.0,
    })
}

/// Horizontal offset (in px) to align dropdown below a given button index.
pub fn dropdown_x_offset(menu: ActiveBarMenu, m: &signex_widgets::active_bar::Metrics) -> f32 {
    super::layout::trigger_left(menu, m)
}

/// A `Custom` row's button: hovers with the widget's own row paint.
fn dd_btn_style_f(
    text_c: Color,
    paint: RowPaint,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Some(Background::Color(paint.hover)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border {
                radius: paint.radius.into(),
                ..Border::default()
            },
            text_color: text_c,
            ..button::Style::default()
        }
    }
}
