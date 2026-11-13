//! The two irregular dropdowns that ride the widget's `Custom` escape
//! hatch: the NetColor swatches and the Selection Filter chip grid.
//! Graphite themes paint both through `super::graphite`.

use iced::widget::{Space, column, container, row};
use iced::{Color, Element, Widget};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, RowPaint};
use signex_widgets::{button, raw, text};

use crate::active_bar::{
    AbColors, ActiveBarAction, ActiveBarMsg, CustomFilterPreset, SelectionFilter,
};
use crate::icons as ic;

use super::{dd_btn_style_f, dd_item, graphite};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// NetColor menu: seven colour swatches (each an irregular `Custom` row —
/// a colour chip in place of an SVG icon), then the Custom / Clear rows
/// as ordinary items. The Clear rows grey out when no net carries a
/// custom colour (`requires_net_color`).
pub(super) fn net_color_entries(
    i18n: &signex_i18n::I18n,
    tokens: &ThemeTokens,
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let ac = AbColors::from_tokens(tokens);
    // The row hovers like the widget's own item rows (ink wash, rounded).
    let row_paint = RowPaint::of(tokens);
    let color_item =
        |label: &str, color: Color, action: ActiveBarAction| -> Element<'static, ActiveBarMsg> {
            // The swatch sits inside a 20×20 slot so the label column
            // lines up with the SVG-icon rows below (the shared dropdown
            // widget uses a 20-px icon column).
            let swatch = container(Space::new())
                .width(graphite::SWATCH_SIZE)
                .height(graphite::SWATCH_SIZE)
                .style(graphite::swatch_style(tokens, color));
            let swatch_slot = container(swatch)
                .width(20)
                .height(20)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center);
            button(
                row![
                    swatch_slot,
                    text(raw(label.to_string()))
                        .size(TS.title)
                        .color(ac.text)
                        .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center),
            )
            .width(iced::Length::Fill)
            .padding([5.0, SP.l])
            .on_press(ActiveBarMsg::Action(action))
            .style(dd_btn_style_f(ac.text, row_paint))
            .boxed()
        };
    vec![
        DropdownEntry::Custom(color_item(
            "Blue",
            Color::from_rgb(0.40, 0.40, 0.93),
            ActiveBarAction::NetColorBlue,
        )),
        DropdownEntry::Custom(color_item(
            "Light Green",
            Color::from_rgb(0.40, 0.93, 0.40),
            ActiveBarAction::NetColorLightGreen,
        )),
        DropdownEntry::Custom(color_item(
            "Light Blue",
            Color::from_rgb(0.40, 0.80, 0.93),
            ActiveBarAction::NetColorLightBlue,
        )),
        DropdownEntry::Custom(color_item(
            "Red",
            Color::from_rgb(0.93, 0.30, 0.30),
            ActiveBarAction::NetColorRed,
        )),
        DropdownEntry::Custom(color_item(
            "Fuchsia",
            Color::from_rgb(0.80, 0.30, 0.80),
            ActiveBarAction::NetColorFuchsia,
        )),
        DropdownEntry::Custom(color_item(
            "Yellow",
            Color::from_rgb(0.93, 0.80, 0.20),
            ActiveBarAction::NetColorYellow,
        )),
        DropdownEntry::Custom(color_item(
            "Dark Green",
            Color::from_rgb(0.13, 0.55, 0.13),
            ActiveBarAction::NetColorDarkGreen,
        )),
        DropdownEntry::Separator,
        dd_item(
            i18n,
            ic::icon_dd_net_color_custom(tid),
            "Custom Color...",
            ActiveBarAction::NetColorCustom,
            sel,
            nc,
        ),
        DropdownEntry::Separator,
        dd_item(
            i18n,
            ic::icon_dd_net_color_clear(tid),
            "Clear Net Color",
            ActiveBarAction::ClearNetColor,
            sel,
            nc,
        ),
        dd_item(
            i18n,
            ic::icon_dd_net_color_clear_all(tid),
            "Clear All Net Colors",
            ActiveBarAction::ClearAllNetColors,
            sel,
            nc,
        ),
    ]
}

/// Selection Filter menu: an irregular chip-wrap layout (All toggle +
/// user presets, then two rows of six category chips) that can't be
/// expressed as a vertical `Item` list, so it rides the widget's
/// `Custom` escape hatch as a single owned `Element<'static>`.
pub(super) fn filter_entry(
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
) -> DropdownEntry<ActiveBarMsg> {
    graphite_filter_entry(tokens, filters, custom_presets)
}

/// The preset's button label: its name, or "Filter N" when unnamed.
fn preset_label(idx: usize, preset: &CustomFilterPreset) -> String {
    if preset.name.trim().is_empty() {
        format!("Filter {}", idx + 1)
    } else {
        preset.name.clone()
    }
}

/// Graphite Selection Filter: an All / None segmented control and the
/// presets on top, then the categories as toggle chips, six per row.
fn graphite_filter_entry(
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
) -> DropdownEntry<ActiveBarMsg> {
    let scope = graphite::FilterScope::of(filters.len(), SelectionFilter::ALL.len());
    let mut top_row = iced::widget::Row::<Element<'static, ActiveBarMsg>>::new()
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .push(graphite::all_none(tokens, scope));
    for (idx, preset) in custom_presets.iter().enumerate() {
        top_row = top_row.push(graphite::chip(
            tokens,
            preset_label(idx, preset),
            false,
            ActiveBarMsg::ApplyCustomFilter(idx),
        ));
    }
    let mut chips = iced::widget::Column::<Element<'static, ActiveBarMsg>>::new().spacing(SP.s);
    for categories in SelectionFilter::ALL.chunks(6) {
        let mut line = iced::widget::Row::<Element<'static, ActiveBarMsg>>::new().spacing(SP.s);
        for &filter in categories {
            line = line.push(graphite::chip(
                tokens,
                filter.label().to_string(),
                filters.contains(&filter),
                ActiveBarMsg::ToggleFilter(filter),
            ));
        }
        chips = chips.push(line.boxed());
    }
    DropdownEntry::Custom(
        column![
            container(top_row).padding([SP.s, SP.m]),
            container(chips).padding([SP.s, SP.m]),
        ]
        .spacing(SP.xs)
        .boxed(),
    )
}
