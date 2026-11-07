//! The two irregular dropdowns that ride the widget's `Custom` escape
//! hatch: the NetColor swatches and the Selection Filter chip grid.
//! Graphite themes paint both through `super::graphite`.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Theme, Widget};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, RowPaint};

use crate::active_bar::{
    AbColors, ActiveBarAction, ActiveBarMsg, CustomFilterPreset, SelectionFilter,
};
use crate::icons as ic;
use crate::styles;

use super::{dd_btn_style_f, dd_item, graphite};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// NetColor menu: seven colour swatches (each an irregular `Custom` row —
/// a colour chip in place of an SVG icon), then the Custom / Clear rows
/// as ordinary items. The Clear rows grey out when no net carries a
/// custom colour (`requires_net_color`).
pub(super) fn net_color_entries(
    tokens: &ThemeTokens,
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let ac = AbColors::from_tokens(tokens);
    let swatch_border = ac.sep;
    let is_graphite = styles::is_graphite(tokens);
    // The row hovers like the widget's own item rows (Graphite: ink wash,
    // rounded; legacy: the theme hover on a square row).
    let row_paint = RowPaint::of(tokens);
    let color_item =
        |label: &str, color: Color, action: ActiveBarAction| -> Element<'static, ActiveBarMsg> {
            // The swatch sits inside a 20×20 slot so the label column
            // lines up with the SVG-icon rows below (the shared dropdown
            // widget uses a 20-px icon column).
            let swatch = if is_graphite {
                container(Space::new())
                    .width(graphite::SWATCH_SIZE)
                    .height(graphite::SWATCH_SIZE)
                    .style(graphite::swatch_style(tokens, color))
            } else {
                container(Space::new())
                    .width(14)
                    .height(14)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(color)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: swatch_border,
                            ..Border::default()
                        },
                        ..container::Style::default()
                    })
            };
            let swatch_slot = container(swatch)
                .width(20)
                .height(20)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center);
            button(
                row![
                    swatch_slot,
                    text(label.to_string())
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
            ic::icon_dd_net_color_custom(tid),
            "Custom Color...",
            ActiveBarAction::NetColorCustom,
            sel,
            nc,
        ),
        DropdownEntry::Separator,
        dd_item(
            ic::icon_dd_net_color_clear(tid),
            "Clear Net Color",
            ActiveBarAction::ClearNetColor,
            sel,
            nc,
        ),
        dd_item(
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
    if styles::is_graphite(tokens) {
        graphite_filter_entry(tokens, filters, custom_presets)
    } else {
        legacy_filter_entry(tokens, filters, custom_presets)
    }
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
        chips = chips.push(line);
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

/// The legacy-theme Selection Filter: square accent-edged chips and the
/// "All - On / Off" toggle.
fn legacy_filter_entry(
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
) -> DropdownEntry<ActiveBarMsg> {
    let ac = AbColors::from_tokens(tokens);
    // Snapshot the slice into an owned Vec, since each preset button's
    // closure must own its label/index for the returned `Element<'static>`.
    let custom_presets_owned: Vec<CustomFilterPreset> = custom_presets.to_vec();
    // Altium-style tag buttons for selection filter.
    let text_primary = ac.text;
    let hover_c = ac.hover;
    // Border colour matches the Properties-panel unit boxes (`seg_btn`
    // uses `tokens.accent`); near-square corners give the chips a more
    // "input-like" look than the old pill.
    let chip_border = styles::ti(tokens.accent);
    let chip_radius = 2.0_f32;
    let all_on = filters.len() == SelectionFilter::ALL.len();
    // Chip fills and off-state text come from the theme, so the chips
    // read on light themes as well as dark.
    let chip_on_bg = styles::tokens::overlay(tokens, styles::tokens::Overlay::Pressed);
    let chip_off_bg = styles::tokens::overlay(tokens, styles::tokens::Overlay::Faint);
    let chip_off_text = styles::tokens::muted_text(tokens);
    let tag = |filter: SelectionFilter, enabled: bool| -> Element<'static, ActiveBarMsg> {
        let label = filter.label();
        let active_bg = chip_on_bg;
        let inactive_bg = chip_off_bg;
        let text_on = text_primary;
        let text_off = chip_off_text;
        button(text(label.to_string()).size(TS.body).color(if enabled {
            text_on
        } else {
            text_off
        }))
        .padding([SP.s, 10.0])
        .on_press(ActiveBarMsg::ToggleFilter(filter))
        .style(move |_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => Background::Color(hover_c),
                _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
            };
            button::Style {
                background: Some(bg),
                border: Border {
                    width: 1.0,
                    radius: chip_radius.into(),
                    color: chip_border,
                    ..Border::default()
                },
                text_color: if enabled { text_on } else { text_off },
                ..button::Style::default()
            }
        })
        .boxed()
    };
    let all_label = if all_on { "All - On" } else { "All - Off" };
    // All-On/Off as a real toggle button (matches chip styling).
    let all_active_bg = chip_on_bg;
    let all_inactive_bg = chip_off_bg;
    let all_text_off = chip_off_text;
    let all_toggle = button(text(all_label.to_string()).size(TS.body).color(if all_on {
        text_primary
    } else {
        all_text_off
    }))
    .padding([SP.s, SP.l])
    .on_press(ActiveBarMsg::ToggleAllFilters)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Background::Color(hover_c),
            _ => Background::Color(if all_on {
                all_active_bg
            } else {
                all_inactive_bg
            }),
        };
        button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: chip_radius.into(),
                color: chip_border,
                ..Border::default()
            },
            text_color: if all_on { text_primary } else { all_text_off },
            ..button::Style::default()
        }
    });
    // Row 1 = All toggle followed by user-defined preset shortcuts
    // (clicking one replaces the active filter set).
    let mut top_row = iced::widget::Row::<Element<'static, ActiveBarMsg>>::new()
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .push(all_toggle.boxed());
    for (idx, preset) in custom_presets_owned.iter().enumerate() {
        let label = preset_label(idx, preset);
        let preset_btn = button(text(label).size(TS.body).color(text_primary))
            .padding([SP.s, 10.0])
            .on_press(ActiveBarMsg::ApplyCustomFilter(idx))
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Background::Color(hover_c),
                    _ => Background::Color(chip_off_bg),
                };
                button::Style {
                    background: Some(bg),
                    border: Border {
                        width: 1.0,
                        radius: chip_radius.into(),
                        color: chip_border,
                        ..Border::default()
                    },
                    text_color: text_primary,
                    ..button::Style::default()
                }
            });
        top_row = top_row.push(preset_btn.boxed());
    }
    // 3-row layout: row 1 = All toggle + presets. Rows 2 & 3 = 6+6 chips.
    let filter_content: Element<'static, ActiveBarMsg> = column![
        container(top_row).padding([SP.s, SP.m]),
        container(
            column![
                row![
                    tag(
                        SelectionFilter::Components,
                        filters.contains(&SelectionFilter::Components)
                    ),
                    tag(
                        SelectionFilter::Wires,
                        filters.contains(&SelectionFilter::Wires)
                    ),
                    tag(
                        SelectionFilter::Buses,
                        filters.contains(&SelectionFilter::Buses)
                    ),
                    tag(
                        SelectionFilter::SheetSymbols,
                        filters.contains(&SelectionFilter::SheetSymbols)
                    ),
                    tag(
                        SelectionFilter::SheetEntries,
                        filters.contains(&SelectionFilter::SheetEntries)
                    ),
                    tag(
                        SelectionFilter::NetLabels,
                        filters.contains(&SelectionFilter::NetLabels)
                    ),
                ]
                .spacing(SP.s),
                row![
                    tag(
                        SelectionFilter::Parameters,
                        filters.contains(&SelectionFilter::Parameters)
                    ),
                    tag(
                        SelectionFilter::Ports,
                        filters.contains(&SelectionFilter::Ports)
                    ),
                    tag(
                        SelectionFilter::PowerPorts,
                        filters.contains(&SelectionFilter::PowerPorts)
                    ),
                    tag(
                        SelectionFilter::Texts,
                        filters.contains(&SelectionFilter::Texts)
                    ),
                    tag(
                        SelectionFilter::DrawingObjects,
                        filters.contains(&SelectionFilter::DrawingObjects)
                    ),
                    tag(
                        SelectionFilter::Other,
                        filters.contains(&SelectionFilter::Other)
                    ),
                ]
                .spacing(SP.s),
            ]
            .spacing(SP.s),
        )
        .padding([SP.s, SP.m]),
    ]
    .spacing(SP.xs)
    .boxed();
    DropdownEntry::Custom(filter_content)
}
