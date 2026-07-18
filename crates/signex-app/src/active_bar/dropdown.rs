//! Active Bar dropdown menus — data-driven. Each `ActiveBarMenu` builds
//! pure `DropdownEntry` rows that the shared
//! `signex_widgets::active_bar_dropdown` widget renders, so the
//! schematic, footprint, and future PCB active bars share one dropdown
//! renderer (see ADR-0003). Enable/disable is folded into each
//! `DropdownItem` at build time, so no render-time selection guard is
//! needed here.

use iced::widget::{Space, button, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::icons as ic;
use crate::styles;

use super::{
    AbColors, ActiveBarAction, ActiveBarMenu, ActiveBarMsg, CustomFilterPreset, SelectionFilter,
    requires_net_color, requires_selection,
};

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
    // from the passed booleans, so the old render-time `HasSelectionGuard`
    // thread-local is no longer needed for the dropdown.
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

/// Whether `action`'s dropdown row is clickable given the current
/// selection / net-colour context. Pure replacement for the render-time
/// `HasSelectionGuard` thread-local: the enable state is computed at
/// build time and baked into the `DropdownItem` rather than read from a
/// global during draw.
fn dd_action_enabled(action: &ActiveBarAction, has_selection: bool, has_net_colors: bool) -> bool {
    if requires_selection(action) && !has_selection {
        return false;
    }
    if requires_net_color(action) && !has_net_colors {
        return false;
    }
    true
}

/// One icon + label dropdown row. Disabled rows drop their `on_press`
/// (the widget greys the row and ignores clicks — Altium parity).
fn dd_item(
    icon: svg::Handle,
    label: &str,
    action: ActiveBarAction,
    has_selection: bool,
    has_net_colors: bool,
) -> DropdownEntry<ActiveBarMsg> {
    let enabled = dd_action_enabled(&action, has_selection, has_net_colors);
    DropdownEntry::Item(
        DropdownItem::new(label, ActiveBarMsg::Action(action))
            .icon(icon)
            .disabled(!enabled),
    )
}

/// Route each `ActiveBarMenu` to its pure entry-builder. Uniform menus
/// are one line each; the two irregular menus (Filter chip grid and
/// NetColor swatches) use the widget's `Custom` escape hatch.
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
        ActiveBarMenu::SelectMode => select_mode_entries(tid, sel, nc),
        ActiveBarMenu::Select => select_entries(tid, sel, nc),
        ActiveBarMenu::Align => align_entries(tid, sel, nc),
        ActiveBarMenu::Wiring => wiring_entries(tid, sel, nc),
        ActiveBarMenu::Power => power_entries(tid, sel, nc),
        ActiveBarMenu::Harness => harness_entries(tid, sel, nc),
        ActiveBarMenu::SheetSymbol => sheet_symbol_entries(tid, sel, nc),
        ActiveBarMenu::Port => port_entries(tid, sel, nc),
        ActiveBarMenu::Directives => directives_entries(tid, sel, nc),
        ActiveBarMenu::TextTools => text_tools_entries(tid, sel, nc),
        ActiveBarMenu::Shapes => shapes_entries(tid, sel, nc),
        ActiveBarMenu::NetColor => net_color_entries(tokens, tid, sel, nc),
    }
}

fn select_mode_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_select_lasso(tid), "Lasso Select", ActiveBarAction::LassoSelect, sel, nc),
        dd_item(ic::icon_dd_select_inside(tid), "Inside Area", ActiveBarAction::InsideArea, sel, nc),
        dd_item(ic::icon_dd_select_outside(tid), "Outside Area", ActiveBarAction::OutsideArea, sel, nc),
        dd_item(ic::icon_dd_select_touching_rect(tid), "Touching Rectangle", ActiveBarAction::TouchingRectangle, sel, nc),
        dd_item(ic::icon_dd_select_touching_line(tid), "Touching Line", ActiveBarAction::TouchingLine, sel, nc),
        dd_item(ic::icon_dd_select_all(tid), "All", ActiveBarAction::SelectAll, sel, nc),
        dd_item(ic::icon_dd_select_connection(tid), "Connection", ActiveBarAction::SelectConnection, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_select_toggle(tid), "Toggle Selection", ActiveBarAction::ToggleSelection, sel, nc),
    ]
}

fn select_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_drag(tid), "Drag", ActiveBarAction::Drag, sel, nc),
        dd_item(ic::icon_dd_move(tid), "Move", ActiveBarAction::MoveSelection, sel, nc),
        dd_item(ic::icon_dd_move_sel(tid), "Move Selection", ActiveBarAction::MoveSelection, sel, nc),
        dd_item(ic::icon_dd_move_xy(tid), "Move Selection by X, Y...", ActiveBarAction::MoveSelectionXY, sel, nc),
        dd_item(ic::icon_dd_drag_sel(tid), "Drag Selection", ActiveBarAction::DragSelection, sel, nc),
        dd_item(ic::icon_dd_move_to_front(tid), "Move To Front", ActiveBarAction::MoveToFront, sel, nc),
        dd_item(ic::icon_dd_rotate(tid), "Rotate Selection", ActiveBarAction::RotateSelection, sel, nc),
        dd_item(ic::icon_dd_rotate_cw(tid), "Rotate Selection Clockwise", ActiveBarAction::RotateSelectionCW, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_bring_front(tid), "Bring To Front", ActiveBarAction::BringToFront, sel, nc),
        dd_item(ic::icon_dd_send_back(tid), "Send To Back", ActiveBarAction::SendToBack, sel, nc),
        dd_item(ic::icon_dd_bring_front_of(tid), "Bring To Front Of", ActiveBarAction::BringToFrontOf, sel, nc),
        dd_item(ic::icon_dd_send_back_of(tid), "Send To Back Of", ActiveBarAction::SendToBackOf, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_flip_x(tid), "Flip Selected Sheet Symbols Along X", ActiveBarAction::FlipSelectedX, sel, nc),
        dd_item(ic::icon_dd_flip_y(tid), "Flip Selected Sheet Symbols Along Y", ActiveBarAction::FlipSelectedY, sel, nc),
    ]
}

fn align_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_align_left(tid), "Align Left", ActiveBarAction::AlignLeft, sel, nc),
        dd_item(ic::icon_dd_align_right(tid), "Align Right", ActiveBarAction::AlignRight, sel, nc),
        dd_item(ic::icon_dd_align_hcenter(tid), "Align Horizontal Centers", ActiveBarAction::AlignHorizontalCenters, sel, nc),
        dd_item(ic::icon_dd_dist_horiz(tid), "Distribute Horizontally", ActiveBarAction::DistributeHorizontally, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_align_top(tid), "Align Top", ActiveBarAction::AlignTop, sel, nc),
        dd_item(ic::icon_dd_align_bottom(tid), "Align Bottom", ActiveBarAction::AlignBottom, sel, nc),
        dd_item(ic::icon_dd_align_vcenter(tid), "Align Vertical Centers", ActiveBarAction::AlignVerticalCenters, sel, nc),
        dd_item(ic::icon_dd_dist_vert(tid), "Distribute Vertically", ActiveBarAction::DistributeVertically, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_align_grid(tid), "Align To Grid", ActiveBarAction::AlignToGrid, sel, nc),
    ]
}

fn wiring_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_wire(tid), "Wire", ActiveBarAction::DrawWire, sel, nc),
        dd_item(ic::icon_dd_bus(tid), "Bus", ActiveBarAction::DrawBus, sel, nc),
        dd_item(ic::icon_dd_bus_entry(tid), "Bus Entry", ActiveBarAction::PlaceBusEntry, sel, nc),
        dd_item(ic::icon_dd_net_label(tid), "Net Label", ActiveBarAction::PlaceNetLabel, sel, nc),
    ]
}

fn power_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_gnd(tid), "Place GND power port", ActiveBarAction::PlacePowerGND, sel, nc),
        dd_item(ic::icon_dd_vcc(tid), "Place VCC power port", ActiveBarAction::PlacePowerVCC, sel, nc),
        dd_item(ic::icon_dd_pwr_plus12(tid), "Place +12 power port", ActiveBarAction::PlacePowerPlus12, sel, nc),
        dd_item(ic::icon_dd_pwr_plus5(tid), "Place +5 power port", ActiveBarAction::PlacePowerPlus5, sel, nc),
        dd_item(ic::icon_dd_pwr_minus5(tid), "Place -5 power port", ActiveBarAction::PlacePowerMinus5, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_pwr_arrow(tid), "Place Arrow style power port", ActiveBarAction::PlacePowerArrow, sel, nc),
        dd_item(ic::icon_dd_pwr_wave(tid), "Place Wave style power port", ActiveBarAction::PlacePowerWave, sel, nc),
        dd_item(ic::icon_dd_pwr_bar(tid), "Place Bar style power port", ActiveBarAction::PlacePowerBar, sel, nc),
        dd_item(ic::icon_dd_pwr_circle(tid), "Place Circle style power port", ActiveBarAction::PlacePowerCircle, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_pwr_signal_gnd(tid), "Place Signal Ground power port", ActiveBarAction::PlacePowerSignalGND, sel, nc),
        dd_item(ic::icon_dd_pwr_earth(tid), "Place Earth power port", ActiveBarAction::PlacePowerEarth, sel, nc),
    ]
}

fn harness_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_harness(tid), "Signal Harness", ActiveBarAction::PlaceSignalHarness, sel, nc),
        dd_item(ic::icon_dd_harness_conn(tid), "Harness Connector", ActiveBarAction::PlaceHarnessConnector, sel, nc),
        dd_item(ic::icon_dd_harness_entry(tid), "Harness Entry", ActiveBarAction::PlaceHarnessEntry, sel, nc),
    ]
}

fn sheet_symbol_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_sheet_symbol(tid), "Sheet Symbol", ActiveBarAction::PlaceSheetSymbol, sel, nc),
        dd_item(ic::icon_dd_sheet_entry(tid), "Sheet Entry", ActiveBarAction::PlaceSheetEntry, sel, nc),
        dd_item(ic::icon_dd_device_sheet(tid), "Device Sheet Symbol", ActiveBarAction::PlaceDeviceSheetSymbol, sel, nc),
        dd_item(ic::icon_dd_reuse_block(tid), "Reuse Block...", ActiveBarAction::PlaceReuseBlock, sel, nc),
    ]
}

fn port_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_port(tid), "Port", ActiveBarAction::PlacePort, sel, nc),
        dd_item(ic::icon_dd_off_sheet(tid), "Off Sheet Connector", ActiveBarAction::PlaceOffSheetConnector, sel, nc),
    ]
}

fn directives_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_param_set(tid), "Parameter Set", ActiveBarAction::PlaceParameterSet, sel, nc),
        dd_item(ic::icon_dd_no_erc(tid), "Generic No ERC", ActiveBarAction::PlaceNoERC, sel, nc),
        dd_item(ic::icon_dd_diff_pair(tid), "Differential Pair", ActiveBarAction::PlaceDiffPair, sel, nc),
        dd_item(ic::icon_dd_blanket(tid), "Blanket", ActiveBarAction::PlaceBlanket, sel, nc),
        dd_item(ic::icon_dd_blanket(tid), "Compile Mask", ActiveBarAction::PlaceCompileMask, sel, nc),
    ]
}

fn text_tools_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_text_string(tid), "Text String", ActiveBarAction::PlaceTextString, sel, nc),
        dd_item(ic::icon_dd_text_frame(tid), "Text Frame", ActiveBarAction::PlaceTextFrame, sel, nc),
        dd_item(ic::icon_dd_note(tid), "Note", ActiveBarAction::PlaceNote, sel, nc),
    ]
}

fn shapes_entries(tid: ThemeId, sel: bool, nc: bool) -> Vec<DropdownEntry<ActiveBarMsg>> {
    vec![
        dd_item(ic::icon_dd_arc(tid), "Arc", ActiveBarAction::DrawArc, sel, nc),
        dd_item(ic::icon_dd_circle(tid), "Full Circle", ActiveBarAction::DrawFullCircle, sel, nc),
        dd_item(ic::icon_dd_arc(tid), "Elliptical Arc", ActiveBarAction::DrawEllipticalArc, sel, nc),
        dd_item(ic::icon_dd_ellipse(tid), "Ellipse", ActiveBarAction::DrawEllipse, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_line(tid), "Line", ActiveBarAction::DrawLine, sel, nc),
        dd_item(ic::icon_dd_rect(tid), "Rectangle", ActiveBarAction::DrawRectangle, sel, nc),
        dd_item(ic::icon_dd_round_rect(tid), "Round Rectangle", ActiveBarAction::DrawRoundRectangle, sel, nc),
        dd_item(ic::icon_dd_polygon(tid), "Polygon", ActiveBarAction::DrawPolygon, sel, nc),
        dd_item(ic::icon_dd_bezier(tid), "Bezier", ActiveBarAction::DrawBezier, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_graphic(tid), "Graphic...", ActiveBarAction::PlaceGraphic, sel, nc),
    ]
}

/// NetColor menu: seven colour swatches (each an irregular `Custom` row —
/// a colour chip in place of an SVG icon), then the Custom / Clear rows
/// as ordinary items. The Clear rows grey out when no net carries a
/// custom colour (`requires_net_color`).
fn net_color_entries(
    tokens: &ThemeTokens,
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let ac = AbColors::from_tokens(tokens);
    let color_item = |label: &str,
                      color: Color,
                      action: ActiveBarAction|
     -> Element<'static, ActiveBarMsg> {
        // The 14×14 swatch sits inside a 20×20 slot so the label column
        // lines up with the SVG-icon rows below (the shared dropdown
        // widget uses a 20-px icon column).
        let swatch = container(Space::new())
            .width(14)
            .height(14)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(color)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: Color::from_rgb(0.3, 0.3, 0.35),
                },
                ..container::Style::default()
            });
        let swatch_slot = container(swatch)
            .width(20)
            .height(20)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);
        button(
            row![
                swatch_slot,
                text(label.to_string())
                    .size(13)
                    .color(ac.text)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .width(iced::Length::Fill)
        .padding([5, 12])
        .on_press(ActiveBarMsg::Action(action))
        .style(dd_btn_style_f(ac.text, ac.hover))
        .into()
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
        dd_item(ic::icon_dd_net_color_custom(tid), "Custom Color...", ActiveBarAction::NetColorCustom, sel, nc),
        DropdownEntry::Separator,
        dd_item(ic::icon_dd_net_color_clear(tid), "Clear Net Color", ActiveBarAction::ClearNetColor, sel, nc),
        dd_item(ic::icon_dd_net_color_clear_all(tid), "Clear All Net Colors", ActiveBarAction::ClearAllNetColors, sel, nc),
    ]
}

/// Selection Filter menu: an irregular chip-wrap layout (All toggle +
/// user presets, then two rows of six category chips) that can't be
/// expressed as a vertical `Item` list, so it rides the widget's
/// `Custom` escape hatch as a single owned `Element<'static>`.
fn filter_entry(
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
    let tag = |filter: SelectionFilter, enabled: bool| -> Element<'static, ActiveBarMsg> {
        let label = filter.label();
        let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
        let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
        let text_on = text_primary;
        let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
        button(text(label.to_string()).size(11).color(if enabled {
            text_on
        } else {
            text_off
        }))
        .padding([4, 10])
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
                },
                text_color: if enabled { text_on } else { text_off },
                ..button::Style::default()
            }
        })
        .into()
    };
    let all_label = if all_on { "All - On" } else { "All - Off" };
    // All-On/Off as a real toggle button (matches chip styling).
    let all_active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let all_inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let all_text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
    let all_toggle = button(text(all_label.to_string()).size(11).color(if all_on {
        text_primary
    } else {
        all_text_off
    }))
    .padding([4, 12])
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
            },
            text_color: if all_on { text_primary } else { all_text_off },
            ..button::Style::default()
        }
    });
    // Row 1 = All toggle followed by user-defined preset shortcuts
    // (clicking one replaces the active filter set).
    let mut top_row = iced::widget::Row::new()
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .push(all_toggle);
    for (idx, preset) in custom_presets_owned.iter().enumerate() {
        let label = if preset.name.trim().is_empty() {
            format!("Filter {}", idx + 1)
        } else {
            preset.name.clone()
        };
        let preset_btn = button(text(label).size(11).color(text_primary))
            .padding([4, 10])
            .on_press(ActiveBarMsg::ApplyCustomFilter(idx))
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Background::Color(hover_c),
                    _ => Background::Color(Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0)),
                };
                button::Style {
                    background: Some(bg),
                    border: Border {
                        width: 1.0,
                        radius: chip_radius.into(),
                        color: chip_border,
                    },
                    text_color: text_primary,
                    ..button::Style::default()
                }
            });
        top_row = top_row.push(preset_btn);
    }
    // 3-row layout: row 1 = All toggle + presets. Rows 2 & 3 = 6+6 chips.
    let filter_content: Element<'static, ActiveBarMsg> = column![
        container(top_row).padding([4, 8]),
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
                .spacing(4),
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
                .spacing(4),
            ]
            .spacing(4),
        )
        .padding([4, 8]),
    ]
    .spacing(2)
    .into();
    DropdownEntry::Custom(filter_content)
}

/// Pinned column width per dropdown menu.
///
/// `view_dropdown` wraps the items column in a `Length::Fixed(W)`
/// container using this value. That bound lets each item set
/// `button.width(Length::Fill)` (so the hover background covers the
/// full row) without `Fill` propagating to the viewport — which is the
/// `Length::Fill`-inside-`Length::Shrink` trap iced 0.14 falls into.
///
/// Widths are sized to the longest label in each menu (Roboto @ 13 +
/// 28 px icon column + 24 px button padding + a small safety margin).
/// `Filter` returns `None` because its chip wrap layout already drives
/// its own width.
fn dropdown_min_width(menu: ActiveBarMenu) -> Option<f32> {
    // Width formula: ~6.5 px/char × longest_label + 60 px overhead
    // (24 px button padding + 20 px icon column + 8 px spacing +
    //  small safety). Roboto @ 13 px is narrower than the 8 px/char
    // estimate I used originally — tightening removes the right-side
    // dead space the user noticed.
    Some(match menu {
        ActiveBarMenu::Filter => return None,
        // "Flip Selected Sheet Symbols Along X" (36 chars)
        ActiveBarMenu::Select => 300.0,
        // "Touching Rectangle" (18)
        ActiveBarMenu::SelectMode => 180.0,
        // "Align Horizontal Centers" (24)
        ActiveBarMenu::Align => 220.0,
        // "Net Label" (9) — keep a usable minimum.
        ActiveBarMenu::Wiring => 140.0,
        // "Place Signal Ground power port" (30)
        ActiveBarMenu::Power => 260.0,
        // "Harness Connector" (17)
        ActiveBarMenu::Harness => 180.0,
        // "Device Sheet Symbol" (19)
        ActiveBarMenu::SheetSymbol => 190.0,
        // "Off Sheet Connector" (19)
        ActiveBarMenu::Port => 190.0,
        // "Differential Pair" (17)
        ActiveBarMenu::Directives => 180.0,
        // "Text String" (11)
        ActiveBarMenu::TextTools => 140.0,
        // "Round Rectangle" (15)
        ActiveBarMenu::Shapes => 170.0,
        // "Clear All Net Colors" (20)
        ActiveBarMenu::NetColor => 200.0,
    })
}

/// Horizontal offset (in px) to align dropdown below a given button index.
pub fn dropdown_x_offset(menu: ActiveBarMenu) -> f32 {
    // Bar layout (`view_bar`): each `ab_icon_btn` is a 26 px container,
    // separators are `width(1)`, the row uses `.spacing(2)`, the bar
    // container uses `.padding([2, 2])`. So advancing past one button
    // costs 26 + 2 = 28 px and advancing past one separator costs
    // 1 + 2 = 3 px.
    // Layout: [Filter][Move] | [Select][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    //  btn:     0      1    s    2      3     s   4      5    s    6      7     8    9    s  10     11     12
    let btn = 28.0_f32;
    let s = 3.0_f32;
    let pad = 2.0_f32;
    pad + match menu {
        ActiveBarMenu::Filter => 0.0,
        ActiveBarMenu::Select => btn,
        ActiveBarMenu::SelectMode => 2.0 * btn + s,
        ActiveBarMenu::Align => 3.0 * btn + s,
        ActiveBarMenu::Wiring => 4.0 * btn + 2.0 * s,
        ActiveBarMenu::Power => 5.0 * btn + 2.0 * s,
        ActiveBarMenu::Harness => 6.0 * btn + 3.0 * s,
        ActiveBarMenu::SheetSymbol => 7.0 * btn + 3.0 * s,
        ActiveBarMenu::Port => 8.0 * btn + 3.0 * s,
        ActiveBarMenu::Directives => 9.0 * btn + 3.0 * s,
        ActiveBarMenu::TextTools => 10.0 * btn + 4.0 * s,
        ActiveBarMenu::Shapes => 11.0 * btn + 4.0 * s,
        ActiveBarMenu::NetColor => 12.0 * btn + 4.0 * s,
    }
}

fn dd_btn_style_f(
    text_c: Color,
    hover_c: Color,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Some(Background::Color(hover_c)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..button::Style::default()
        }
    }
}

#[cfg(test)]
mod tests {
    //! Data-to-view tests (iced-rust skill §10): assert on the pure
    //! entry builders — no GPU, no window, no widget tree.
    use super::*;

    const TID: ThemeId = ThemeId::CatppuccinMocha;

    fn items(entries: &[DropdownEntry<ActiveBarMsg>]) -> Vec<(String, bool)> {
        entries
            .iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(it) => Some((it.label.clone(), it.disabled)),
                _ => None,
            })
            .collect()
    }
    fn labels(entries: &[DropdownEntry<ActiveBarMsg>]) -> Vec<String> {
        items(entries).into_iter().map(|(l, _)| l).collect()
    }
    fn seps(entries: &[DropdownEntry<ActiveBarMsg>]) -> usize {
        entries
            .iter()
            .filter(|e| matches!(e, DropdownEntry::Separator))
            .count()
    }
    fn customs(entries: &[DropdownEntry<ActiveBarMsg>]) -> usize {
        entries
            .iter()
            .filter(|e| matches!(e, DropdownEntry::Custom(_)))
            .count()
    }
    fn disabled_of(entries: &[DropdownEntry<ActiveBarMsg>], label: &str) -> bool {
        items(entries)
            .into_iter()
            .find(|(l, _)| l == label)
            .unwrap_or_else(|| panic!("missing row {label:?}"))
            .1
    }

    #[test]
    fn action_enable_predicate() {
        // selection-gated action
        assert!(!dd_action_enabled(&ActiveBarAction::MoveSelection, false, true));
        assert!(dd_action_enabled(&ActiveBarAction::MoveSelection, true, false));
        // net-colour-gated action
        assert!(!dd_action_enabled(&ActiveBarAction::ClearNetColor, true, false));
        assert!(dd_action_enabled(&ActiveBarAction::ClearNetColor, true, true));
        // ungated action is always on
        assert!(dd_action_enabled(&ActiveBarAction::DrawWire, false, false));
    }

    #[test]
    fn wiring_menu_is_four_ungated_items() {
        let e = wiring_entries(TID, false, false);
        assert_eq!(labels(&e), ["Wire", "Bus", "Bus Entry", "Net Label"]);
        assert!(items(&e).iter().all(|(_, d)| !d)); // wiring is never gated
        assert_eq!(seps(&e), 0);
    }

    #[test]
    fn disabled_state_flips_but_labels_are_stable() {
        let on = select_entries(TID, true, false);
        let off = select_entries(TID, false, false);
        // same rows regardless of selection — only the disabled flag moves
        assert_eq!(labels(&on), labels(&off));
        // a selection-family row greys out with no selection
        assert!(!disabled_of(&on, "Move Selection"));
        assert!(disabled_of(&off, "Move Selection"));
    }

    #[test]
    fn select_mode_toggle_sits_after_the_separator() {
        let e = select_mode_entries(TID, true, true);
        assert_eq!(seps(&e), 1);
        assert_eq!(items(&e).last().unwrap().0, "Toggle Selection");
    }

    #[test]
    fn align_and_shapes_have_expected_shape() {
        let a = align_entries(TID, true, true);
        assert_eq!(items(&a).len(), 9);
        assert_eq!(seps(&a), 2);
        let s = shapes_entries(TID, true, true);
        assert_eq!(items(&s).len(), 10);
        assert_eq!(seps(&s), 2);
    }

    #[test]
    fn net_color_swatches_and_gated_clear_rows() {
        let tokens = signex_types::theme::theme_tokens(TID);
        let with = net_color_entries(&tokens, TID, false, true);
        let without = net_color_entries(&tokens, TID, false, false);
        assert_eq!(customs(&with), 7); // seven colour swatches
        assert_eq!(seps(&with), 2);
        // Clear rows grey out when no net carries a custom colour
        assert!(!disabled_of(&with, "Clear Net Color"));
        assert!(disabled_of(&without, "Clear Net Color"));
        assert!(disabled_of(&without, "Clear All Net Colors"));
        // Custom Color... is the arm phase — always enabled
        assert!(!disabled_of(&with, "Custom Color..."));
    }

    #[test]
    fn filter_menu_is_a_single_custom_entry() {
        let tokens = signex_types::theme::theme_tokens(TID);
        let filters = std::collections::HashSet::new();
        let entry = filter_entry(&tokens, &filters, &[]);
        assert!(matches!(entry, DropdownEntry::Custom(_)));
    }
}
