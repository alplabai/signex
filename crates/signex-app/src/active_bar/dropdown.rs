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

/// One row inside a uniform per-menu entry table — theme- and
/// context-free, unlike `DropdownEntry`/`DropdownItem`, so the table
/// itself can be a `const` array (icon fn pointers and fieldless enum
/// variants are const-constructible; `ActiveBarAction` is not `Copy`,
/// so `render` clones it per row). Collapses the ~78 near-identical
/// `dd_item(...)` call sites this file used to carry across ~11
/// uniform per-menu builder functions into ~11 data tables + one
/// renderer (#457, epic #278).
enum EntrySpec {
    Separator,
    Item {
        icon: fn(ThemeId) -> svg::Handle,
        label: &'static str,
        action: ActiveBarAction,
    },
}

impl EntrySpec {
    const fn item(
        icon: fn(ThemeId) -> svg::Handle,
        label: &'static str,
        action: ActiveBarAction,
    ) -> Self {
        Self::Item {
            icon,
            label,
            action,
        }
    }
}

const SELECT_MODE: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_select_lasso,
        "Lasso Select",
        ActiveBarAction::LassoSelect,
    ),
    EntrySpec::item(
        ic::icon_dd_select_inside,
        "Inside Area",
        ActiveBarAction::InsideArea,
    ),
    EntrySpec::item(
        ic::icon_dd_select_outside,
        "Outside Area",
        ActiveBarAction::OutsideArea,
    ),
    EntrySpec::item(
        ic::icon_dd_select_touching_rect,
        "Touching Rectangle",
        ActiveBarAction::TouchingRectangle,
    ),
    EntrySpec::item(
        ic::icon_dd_select_touching_line,
        "Touching Line",
        ActiveBarAction::TouchingLine,
    ),
    EntrySpec::item(ic::icon_dd_select_all, "All", ActiveBarAction::SelectAll),
    EntrySpec::item(
        ic::icon_dd_select_connection,
        "Connection",
        ActiveBarAction::SelectConnection,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_select_toggle,
        "Toggle Selection",
        ActiveBarAction::ToggleSelection,
    ),
];

const SELECT: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_drag, "Drag", ActiveBarAction::Drag),
    EntrySpec::item(ic::icon_dd_move, "Move", ActiveBarAction::MoveSelection),
    EntrySpec::item(
        ic::icon_dd_move_sel,
        "Move Selection",
        ActiveBarAction::MoveSelection,
    ),
    EntrySpec::item(
        ic::icon_dd_move_xy,
        "Move Selection by X, Y...",
        ActiveBarAction::MoveSelectionXY,
    ),
    EntrySpec::item(
        ic::icon_dd_drag_sel,
        "Drag Selection",
        ActiveBarAction::DragSelection,
    ),
    EntrySpec::item(
        ic::icon_dd_move_to_front,
        "Move To Front",
        ActiveBarAction::MoveToFront,
    ),
    EntrySpec::item(
        ic::icon_dd_rotate,
        "Rotate Selection",
        ActiveBarAction::RotateSelection,
    ),
    EntrySpec::item(
        ic::icon_dd_rotate_cw,
        "Rotate Selection Clockwise",
        ActiveBarAction::RotateSelectionCW,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_bring_front,
        "Bring To Front",
        ActiveBarAction::BringToFront,
    ),
    EntrySpec::item(
        ic::icon_dd_send_back,
        "Send To Back",
        ActiveBarAction::SendToBack,
    ),
    EntrySpec::item(
        ic::icon_dd_bring_front_of,
        "Bring To Front Of",
        ActiveBarAction::BringToFrontOf,
    ),
    EntrySpec::item(
        ic::icon_dd_send_back_of,
        "Send To Back Of",
        ActiveBarAction::SendToBackOf,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_flip_x,
        "Flip Selected Sheet Symbols Along X",
        ActiveBarAction::FlipSelectedX,
    ),
    EntrySpec::item(
        ic::icon_dd_flip_y,
        "Flip Selected Sheet Symbols Along Y",
        ActiveBarAction::FlipSelectedY,
    ),
];

const ALIGN: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_align_left,
        "Align Left",
        ActiveBarAction::AlignLeft,
    ),
    EntrySpec::item(
        ic::icon_dd_align_right,
        "Align Right",
        ActiveBarAction::AlignRight,
    ),
    EntrySpec::item(
        ic::icon_dd_align_hcenter,
        "Align Horizontal Centers",
        ActiveBarAction::AlignHorizontalCenters,
    ),
    EntrySpec::item(
        ic::icon_dd_dist_horiz,
        "Distribute Horizontally",
        ActiveBarAction::DistributeHorizontally,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_align_top,
        "Align Top",
        ActiveBarAction::AlignTop,
    ),
    EntrySpec::item(
        ic::icon_dd_align_bottom,
        "Align Bottom",
        ActiveBarAction::AlignBottom,
    ),
    EntrySpec::item(
        ic::icon_dd_align_vcenter,
        "Align Vertical Centers",
        ActiveBarAction::AlignVerticalCenters,
    ),
    EntrySpec::item(
        ic::icon_dd_dist_vert,
        "Distribute Vertically",
        ActiveBarAction::DistributeVertically,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_align_grid,
        "Align To Grid",
        ActiveBarAction::AlignToGrid,
    ),
];

const WIRING: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_wire, "Wire", ActiveBarAction::DrawWire),
    EntrySpec::item(ic::icon_dd_bus, "Bus", ActiveBarAction::DrawBus),
    EntrySpec::item(
        ic::icon_dd_bus_entry,
        "Bus Entry",
        ActiveBarAction::PlaceBusEntry,
    ),
    EntrySpec::item(
        ic::icon_dd_net_label,
        "Net Label",
        ActiveBarAction::PlaceNetLabel,
    ),
];

const POWER: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_gnd,
        "Place GND power port",
        ActiveBarAction::PlacePowerGND,
    ),
    EntrySpec::item(
        ic::icon_dd_vcc,
        "Place VCC power port",
        ActiveBarAction::PlacePowerVCC,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_plus12,
        "Place +12 power port",
        ActiveBarAction::PlacePowerPlus12,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_plus5,
        "Place +5 power port",
        ActiveBarAction::PlacePowerPlus5,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_minus5,
        "Place -5 power port",
        ActiveBarAction::PlacePowerMinus5,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_pwr_arrow,
        "Place Arrow style power port",
        ActiveBarAction::PlacePowerArrow,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_wave,
        "Place Wave style power port",
        ActiveBarAction::PlacePowerWave,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_bar,
        "Place Bar style power port",
        ActiveBarAction::PlacePowerBar,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_circle,
        "Place Circle style power port",
        ActiveBarAction::PlacePowerCircle,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_pwr_signal_gnd,
        "Place Signal Ground power port",
        ActiveBarAction::PlacePowerSignalGND,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_earth,
        "Place Earth power port",
        ActiveBarAction::PlacePowerEarth,
    ),
];

const HARNESS: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_harness,
        "Signal Harness",
        ActiveBarAction::PlaceSignalHarness,
    ),
    EntrySpec::item(
        ic::icon_dd_harness_conn,
        "Harness Connector",
        ActiveBarAction::PlaceHarnessConnector,
    ),
    EntrySpec::item(
        ic::icon_dd_harness_entry,
        "Harness Entry",
        ActiveBarAction::PlaceHarnessEntry,
    ),
];

const SHEET_SYMBOL: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_sheet_symbol,
        "Sheet Symbol",
        ActiveBarAction::PlaceSheetSymbol,
    ),
    EntrySpec::item(
        ic::icon_dd_sheet_entry,
        "Sheet Entry",
        ActiveBarAction::PlaceSheetEntry,
    ),
    EntrySpec::item(
        ic::icon_dd_device_sheet,
        "Device Sheet Symbol",
        ActiveBarAction::PlaceDeviceSheetSymbol,
    ),
    EntrySpec::item(
        ic::icon_dd_reuse_block,
        "Reuse Block...",
        ActiveBarAction::PlaceReuseBlock,
    ),
];

const PORT: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_port, "Port", ActiveBarAction::PlacePort),
    EntrySpec::item(
        ic::icon_dd_off_sheet,
        "Off Sheet Connector",
        ActiveBarAction::PlaceOffSheetConnector,
    ),
];

const DIRECTIVES: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_param_set,
        "Parameter Set",
        ActiveBarAction::PlaceParameterSet,
    ),
    EntrySpec::item(
        ic::icon_dd_no_erc,
        "Generic No ERC",
        ActiveBarAction::PlaceNoERC,
    ),
    EntrySpec::item(
        ic::icon_dd_diff_pair,
        "Differential Pair",
        ActiveBarAction::PlaceDiffPair,
    ),
    EntrySpec::item(
        ic::icon_dd_blanket,
        "Blanket",
        ActiveBarAction::PlaceBlanket,
    ),
    EntrySpec::item(
        ic::icon_dd_blanket,
        "Compile Mask",
        ActiveBarAction::PlaceCompileMask,
    ),
];

const TEXT_TOOLS: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_text_string,
        "Text String",
        ActiveBarAction::PlaceTextString,
    ),
    EntrySpec::item(
        ic::icon_dd_text_frame,
        "Text Frame",
        ActiveBarAction::PlaceTextFrame,
    ),
    EntrySpec::item(ic::icon_dd_note, "Note", ActiveBarAction::PlaceNote),
];

const SHAPES: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_arc, "Arc", ActiveBarAction::DrawArc),
    EntrySpec::item(
        ic::icon_dd_circle,
        "Full Circle",
        ActiveBarAction::DrawFullCircle,
    ),
    EntrySpec::item(
        ic::icon_dd_arc,
        "Elliptical Arc",
        ActiveBarAction::DrawEllipticalArc,
    ),
    EntrySpec::item(ic::icon_dd_ellipse, "Ellipse", ActiveBarAction::DrawEllipse),
    EntrySpec::Separator,
    EntrySpec::item(ic::icon_dd_line, "Line", ActiveBarAction::DrawLine),
    EntrySpec::item(
        ic::icon_dd_rect,
        "Rectangle",
        ActiveBarAction::DrawRectangle,
    ),
    EntrySpec::item(
        ic::icon_dd_round_rect,
        "Round Rectangle",
        ActiveBarAction::DrawRoundRectangle,
    ),
    EntrySpec::item(ic::icon_dd_polygon, "Polygon", ActiveBarAction::DrawPolygon),
    EntrySpec::item(ic::icon_dd_bezier, "Bezier", ActiveBarAction::DrawBezier),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_graphic,
        "Graphic...",
        ActiveBarAction::PlaceGraphic,
    ),
];

/// Render a uniform per-menu `EntrySpec` table into `DropdownEntry`
/// rows, resolving each row's icon for the active theme and folding in
/// the same `dd_item` enable/disable gating every row used before the
/// table refactor.
fn render(
    specs: &'static [EntrySpec],
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    specs
        .iter()
        .map(|spec| match spec {
            EntrySpec::Separator => DropdownEntry::Separator,
            EntrySpec::Item {
                icon,
                label,
                action,
            } => dd_item(icon(tid), label, action.clone(), sel, nc),
        })
        .collect()
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
    let color_item =
        |label: &str, color: Color, action: ActiveBarAction| -> Element<'static, ActiveBarMsg> {
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
        button(
            text(label.to_string())
                .size(11)
                .color(if enabled { text_on } else { text_off }),
        )
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
    // Bar layout (`view_bar`), in the widget's own constants so the
    // pixel geometry has one home. Advancing past one button costs
    // BTN_SIZE + ROW_SPACING; past one separator, SEP_W + ROW_SPACING.
    //
    // The index map below is still hand-maintained, unlike the
    // footprint bar's — converting it needs `view_bar` split into a
    // `bar_items()` the offsets can be measured from, which is its own
    // change. It is correct today: the bar's only Custom slot (the
    // draw-mode pill) is appended after all thirteen triggers.
    use signex_widgets::active_bar::{BAR_PADDING, BTN_SIZE, ROW_SPACING, SEP_W};
    // Layout: [Filter][Move] | [Select][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    //  btn:     0      1    s    2      3     s   4      5    s    6      7     8    9    s  10     11     12
    let btn = BTN_SIZE + ROW_SPACING;
    let s = SEP_W + ROW_SPACING;
    let pad = BAR_PADDING;
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
        assert!(!dd_action_enabled(
            &ActiveBarAction::MoveSelection,
            false,
            true
        ));
        assert!(dd_action_enabled(
            &ActiveBarAction::MoveSelection,
            true,
            false
        ));
        // net-colour-gated action
        assert!(!dd_action_enabled(
            &ActiveBarAction::ClearNetColor,
            true,
            false
        ));
        assert!(dd_action_enabled(
            &ActiveBarAction::ClearNetColor,
            true,
            true
        ));
        // ungated action is always on
        assert!(dd_action_enabled(&ActiveBarAction::DrawWire, false, false));
    }

    #[test]
    fn wiring_menu_is_four_ungated_items() {
        let e = render(WIRING, TID, false, false);
        assert_eq!(labels(&e), ["Wire", "Bus", "Bus Entry", "Net Label"]);
        assert!(items(&e).iter().all(|(_, d)| !d)); // wiring is never gated
        assert_eq!(seps(&e), 0);
    }

    #[test]
    fn disabled_state_flips_but_labels_are_stable() {
        let on = render(SELECT, TID, true, false);
        let off = render(SELECT, TID, false, false);
        // same rows regardless of selection — only the disabled flag moves
        assert_eq!(labels(&on), labels(&off));
        // a selection-family row greys out with no selection
        assert!(!disabled_of(&on, "Move Selection"));
        assert!(disabled_of(&off, "Move Selection"));
    }

    #[test]
    fn select_mode_toggle_sits_after_the_separator() {
        let e = render(SELECT_MODE, TID, true, true);
        assert_eq!(seps(&e), 1);
        assert_eq!(items(&e).last().unwrap().0, "Toggle Selection");
    }

    #[test]
    fn align_and_shapes_have_expected_shape() {
        let a = render(ALIGN, TID, true, true);
        assert_eq!(items(&a).len(), 9);
        assert_eq!(seps(&a), 2);
        let s = render(SHAPES, TID, true, true);
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

    /// Stable serializer for `DropdownEntry` rows, used by the
    /// behaviour-proof golden test below (#457) to prove the
    /// data-table refactor is byte-for-byte output identical to the
    /// pre-refactor per-menu builder functions.
    fn describe(entries: &[DropdownEntry<ActiveBarMsg>]) -> Vec<String> {
        entries
            .iter()
            .map(|e| match e {
                DropdownEntry::Separator => "SEP".to_string(),
                DropdownEntry::Header(label) => format!("HEADER:{label}"),
                DropdownEntry::Custom(_) => "CUSTOM".to_string(),
                DropdownEntry::Item(it) => format!(
                    "ITEM:{}|disabled={}|icon={}|checked={}|shortcut={:?}",
                    it.label,
                    it.disabled,
                    it.icon.is_some(),
                    it.checked,
                    it.shortcut
                ),
            })
            .collect()
    }

    /// GOLDEN (#457): `describe(dropdown_entries(menu, .., sel, nc))` for
    /// every `ActiveBarMenu` variant x every `(has_selection,
    /// has_net_colors)` combo, captured verbatim from the pre-refactor
    /// per-menu builder functions (`select_mode_entries`,
    /// `select_entries`, ... `shapes_entries`, plus `filter_entry` /
    /// `net_color_entries`) before they were replaced by the `EntrySpec`
    /// data table + generic `render`. If this test still passes after
    /// the refactor, the refactor is output-identical.
    const GOLDEN: &[(ActiveBarMenu, bool, bool, &[&str])] = &[
        (ActiveBarMenu::Filter, false, false, &["CUSTOM"]),
        (ActiveBarMenu::Filter, false, true, &["CUSTOM"]),
        (ActiveBarMenu::Filter, true, false, &["CUSTOM"]),
        (ActiveBarMenu::Filter, true, true, &["CUSTOM"]),
        (
            ActiveBarMenu::SelectMode,
            false,
            false,
            &[
                "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::SelectMode,
            false,
            true,
            &[
                "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::SelectMode,
            true,
            false,
            &[
                "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::SelectMode,
            true,
            true,
            &[
                "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Select,
            false,
            false,
            &[
                "ITEM:Drag|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection by X, Y...|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Drag Selection|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move To Front|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection Clockwise|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Bring To Front|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Bring To Front Of|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back Of|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Flip Selected Sheet Symbols Along X|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Flip Selected Sheet Symbols Along Y|disabled=true|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Select,
            false,
            true,
            &[
                "ITEM:Drag|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection by X, Y...|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Drag Selection|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Move To Front|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection Clockwise|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Bring To Front|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Bring To Front Of|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back Of|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Flip Selected Sheet Symbols Along X|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Flip Selected Sheet Symbols Along Y|disabled=true|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Select,
            true,
            false,
            &[
                "ITEM:Drag|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection by X, Y...|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Drag Selection|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move To Front|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection Clockwise|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Bring To Front|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bring To Front Of|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back Of|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Flip Selected Sheet Symbols Along X|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Flip Selected Sheet Symbols Along Y|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Select,
            true,
            true,
            &[
                "ITEM:Drag|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move Selection by X, Y...|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Drag Selection|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Move To Front|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rotate Selection Clockwise|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Bring To Front|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bring To Front Of|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Send To Back Of|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Flip Selected Sheet Symbols Along X|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Flip Selected Sheet Symbols Along Y|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Align,
            false,
            false,
            &[
                "ITEM:Align Left|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Right|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Horizontal Centers|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Horizontally|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align Top|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Bottom|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Vertical Centers|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Vertically|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align To Grid|disabled=true|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Align,
            false,
            true,
            &[
                "ITEM:Align Left|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Right|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Horizontal Centers|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Horizontally|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align Top|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Bottom|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Align Vertical Centers|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Vertically|disabled=true|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align To Grid|disabled=true|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Align,
            true,
            false,
            &[
                "ITEM:Align Left|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Right|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Horizontal Centers|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Horizontally|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align Top|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Bottom|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Vertical Centers|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Vertically|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align To Grid|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Align,
            true,
            true,
            &[
                "ITEM:Align Left|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Right|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Horizontal Centers|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Horizontally|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align Top|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Bottom|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Align Vertical Centers|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Distribute Vertically|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Align To Grid|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Wiring,
            false,
            false,
            &[
                "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Wiring,
            false,
            true,
            &[
                "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Wiring,
            true,
            false,
            &[
                "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Wiring,
            true,
            true,
            &[
                "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Power,
            false,
            false,
            &[
                "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Power,
            false,
            true,
            &[
                "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Power,
            true,
            false,
            &[
                "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Power,
            true,
            true,
            &[
                "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Harness,
            false,
            false,
            &[
                "ITEM:Signal Harness|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Connector|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Entry|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Harness,
            false,
            true,
            &[
                "ITEM:Signal Harness|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Connector|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Entry|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Harness,
            true,
            false,
            &[
                "ITEM:Signal Harness|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Connector|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Entry|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Harness,
            true,
            true,
            &[
                "ITEM:Signal Harness|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Connector|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Harness Entry|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::SheetSymbol,
            false,
            false,
            &[
                "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Device Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Reuse Block...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::SheetSymbol,
            false,
            true,
            &[
                "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Device Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Reuse Block...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::SheetSymbol,
            true,
            false,
            &[
                "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Device Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Reuse Block...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::SheetSymbol,
            true,
            true,
            &[
                "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Device Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Reuse Block...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Port,
            false,
            false,
            &[
                "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Port,
            false,
            true,
            &[
                "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Port,
            true,
            false,
            &[
                "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Port,
            true,
            true,
            &[
                "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Directives,
            false,
            false,
            &[
                "ITEM:Parameter Set|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Differential Pair|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Blanket|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Compile Mask|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Directives,
            false,
            true,
            &[
                "ITEM:Parameter Set|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Differential Pair|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Blanket|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Compile Mask|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Directives,
            true,
            false,
            &[
                "ITEM:Parameter Set|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Differential Pair|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Blanket|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Compile Mask|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Directives,
            true,
            true,
            &[
                "ITEM:Parameter Set|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Differential Pair|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Blanket|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Compile Mask|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::TextTools,
            false,
            false,
            &[
                "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::TextTools,
            false,
            true,
            &[
                "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::TextTools,
            true,
            false,
            &[
                "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::TextTools,
            true,
            true,
            &[
                "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Shapes,
            false,
            false,
            &[
                "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Elliptical Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Ellipse|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Round Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bezier|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Graphic...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Shapes,
            false,
            true,
            &[
                "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Elliptical Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Ellipse|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Round Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bezier|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Graphic...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Shapes,
            true,
            false,
            &[
                "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Elliptical Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Ellipse|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Round Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bezier|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Graphic...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::Shapes,
            true,
            true,
            &[
                "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Elliptical Arc|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Ellipse|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Round Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Bezier|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Graphic...|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::NetColor,
            false,
            false,
            &[
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "SEP",
                "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Clear Net Color|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Clear All Net Colors|disabled=true|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::NetColor,
            false,
            true,
            &[
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "SEP",
                "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Clear Net Color|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Clear All Net Colors|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::NetColor,
            true,
            false,
            &[
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "SEP",
                "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Clear Net Color|disabled=true|icon=true|checked=false|shortcut=None",
                "ITEM:Clear All Net Colors|disabled=true|icon=true|checked=false|shortcut=None",
            ],
        ),
        (
            ActiveBarMenu::NetColor,
            true,
            true,
            &[
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "CUSTOM",
                "SEP",
                "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
                "SEP",
                "ITEM:Clear Net Color|disabled=false|icon=true|checked=false|shortcut=None",
                "ITEM:Clear All Net Colors|disabled=false|icon=true|checked=false|shortcut=None",
            ],
        ),
    ];

    #[test]
    fn dropdown_entries_match_pre_refactor_golden() {
        let tokens = signex_types::theme::theme_tokens(TID);
        let filters: std::collections::HashSet<SelectionFilter> = std::collections::HashSet::new();
        let presets: Vec<CustomFilterPreset> = vec![];
        for &(menu, sel, nc, expected) in GOLDEN {
            let entries = dropdown_entries(menu, &tokens, &filters, &presets, TID, sel, nc);
            let actual = describe(&entries);
            let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
            assert_eq!(
                actual_refs.as_slice(),
                expected,
                "menu={menu:?} sel={sel} nc={nc}: dropdown output diverged from the pre-refactor golden"
            );
        }
    }
}
