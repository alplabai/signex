//! Properties panel — General + Parameters tabs (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code, zero behaviour change.
//! Two large view fns:
//!
//! - `view_properties_general` — General + Page Options tabs (document
//!   options, grid, units, canvas font, sheet color, page sizing).
//! - `view_properties_parameters` — Altium-style document parameter
//!   table with type pickers, value editors, and tag chips.

use iced::mouse;
use iced::widget::{
    Column, Row, Space, button, canvas, column, container, pick_list, row, scrollable, svg, text,
    text_input,
};
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Theme};
use iced_aw::{NumberInput, Wrap};
use std::sync::OnceLock;

use super::{
    LABEL_W, PAPER_SIZES, PROPERTY_CONTROL_PORTION, PROPERTY_LABEL_PORTION, PROPERTY_ROW_PAD_X,
    PageFormatMode, PageOrigin, PanelContext, PanelMsg, SheetColor, collapsible_section,
    paper_dimensions, property_label,
};
use signex_types::coord::Unit;

pub fn view_properties_general<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    // Derive button/input colors from tokens (Copy values captured in closures)
    let input_bg = crate::styles::ti(ctx.tokens.selection); // deep blue tint
    let input_bdr = crate::styles::ti(ctx.tokens.accent);
    let tag_hover = {
        let c = crate::styles::ti(ctx.tokens.accent);
        Color {
            r: (c.r * 1.3).min(1.0),
            g: (c.g * 1.3).min(1.0),
            b: (c.b * 1.3).min(1.0),
            ..c
        }
    };
    let seg_hover = crate::styles::ti(ctx.tokens.hover);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Custom Selection Filters (collapsible) — tabbed editor for up to
    // CUSTOM_FILTER_PRESET_LIMIT named presets that also surface as
    // shortcut buttons in the Active Bar's filter dropdown.
    {
        use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, SelectionFilter};
        let presets = ctx.custom_filter_presets.clone();
        let active_tab = ctx
            .active_custom_filter_tab
            .min(presets.len().saturating_sub(1));
        let muted_c = muted;
        let primary_c = primary;
        // Border colour for tabs and member chips — matches the Active
        // Bar Filter dropdown's chip border treatment so the section
        // reads as one cohesive piece.
        let accent_c = crate::styles::ti(ctx.tokens.accent);
        col = col.push(collapsible_section(
            "prop_sel_filter",
            "Custom Selection Filters",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(6).width(Length::Fill);
                if presets.is_empty() {
                    c = c.push(
                        container(
                            text("No presets yet. Click + to define one.")
                                .size(11)
                                .color(muted_c),
                        )
                        .padding([4, 8]),
                    );
                    c = c.push(
                        container(
                            iced::widget::button(text("+ Add Filter").size(11).color(primary_c))
                                .padding([4, 10])
                                .on_press(PanelMsg::AddCustomFilterPreset),
                        )
                        .padding([4, 8]),
                    );
                    return c;
                }
                // Tab strip: one tab per preset + a trailing "+" tab
                // when room remains. Each tab is its own button — the
                // active one gets the accent border, others a muted one.
                let mut tabs = iced::widget::Row::new()
                    .spacing(2)
                    .align_y(iced::Alignment::Center);
                for (idx, preset) in presets.iter().enumerate() {
                    let label = if preset.name.trim().is_empty() {
                        format!("Filter {}", idx + 1)
                    } else {
                        preset.name.clone()
                    };
                    tabs = tabs.push(custom_filter_tab(
                        label,
                        idx == active_tab,
                        idx,
                        tag_hover,
                        accent_c,
                    ));
                }
                if presets.len() < CUSTOM_FILTER_PRESET_LIMIT {
                    tabs = tabs.push(
                        iced::widget::button(text("+").size(12).color(primary_c))
                            .padding([3, 10])
                            .on_press(PanelMsg::AddCustomFilterPreset),
                    );
                }
                c = c.push(container(tabs).padding([4, 8]));
                // Active tab body — name input + chips + delete.
                let preset = &presets[active_tab];
                let included: std::collections::HashSet<SelectionFilter> =
                    preset.filters.iter().copied().collect();
                let header = row![
                    iced::widget::text_input("Preset name", &preset.name)
                        .size(11)
                        .padding([3, 6])
                        .on_input(move |s| PanelMsg::RenameCustomFilterPreset(active_tab, s))
                        .width(Length::Fill),
                    iced::widget::button(text("Delete").size(10).color(primary_c))
                        .padding([3, 8])
                        .on_press(PanelMsg::RemoveCustomFilterPreset(active_tab)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center);
                let mut wrap = Wrap::new().spacing(4.0).line_spacing(4.0);
                for &f in SelectionFilter::ALL {
                    wrap = wrap.push(preset_chip(
                        f.label(),
                        active_tab,
                        f,
                        included.contains(&f),
                        tag_hover,
                        accent_c,
                    ));
                }
                c = c.push(
                    container(column![header, container(wrap).padding([4, 0])].spacing(4))
                        .padding([6, 8]),
                );
                c
            },
        ));
    }

    // General (collapsible)
    {
        let unit = ctx.unit;
        let grid_size_mm = ctx.grid_size_mm;
        let visible_grid_mm = ctx.visible_grid_mm;
        let snap_enabled = ctx.snap_enabled;
        let snap_hotspots = ctx.snap_hotspots;
        let grid_visible = ctx.grid_visible;
        let canvas_font_name = ctx.canvas_font_name.clone();
        let canvas_font_size = ctx.canvas_font_size;
        let canvas_font_bold = ctx.canvas_font_bold;
        let canvas_font_italic = ctx.canvas_font_italic;
        let canvas_font_popup_open = ctx.canvas_font_popup_open;
        let sheet_color = ctx.sheet_color;
        col = col.push(collapsible_section(
            "prop_general",
            "General",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_label("Units", muted));
                c = c.push(
                    container(
                        row![
                            seg_btn(
                                "mm",
                                unit == Unit::Mm,
                                PanelMsg::SetUnit(Unit::Mm),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr
                            ),
                            seg_btn(
                                "mils",
                                unit == Unit::Mil,
                                PanelMsg::SetUnit(Unit::Mil),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr
                            ),
                        ]
                        .spacing(0.0)
                        .width(Length::Fill),
                    )
                    .padding([2, 8]),
                );
                // Altium-style: Visible Grid and Snap Grid are independent
                c = c.push(form_grid_row(
                    "Visible Grid",
                    visible_grid_mm,
                    unit,
                    false,
                    PanelMsg::SetVisibleGridSize,
                    muted,
                    grid_visible,
                    PanelMsg::ToggleGrid,
                ));
                c = c.push(form_grid_row(
                    "Snap Grid",
                    grid_size_mm,
                    unit,
                    true,
                    PanelMsg::SetGridSize,
                    muted,
                    snap_enabled,
                    PanelMsg::ToggleSnap,
                ));
                c = c.push(form_check_row_shortcut(
                    "Snap to Hotspots",
                    snap_hotspots,
                    PanelMsg::ToggleSnapHotspots,
                    "Shift+E",
                    muted,
                ));
                c = c.push(form_font_link_row(
                    "Canvas Font",
                    &canvas_font_name,
                    canvas_font_size,
                    canvas_font_bold,
                    canvas_font_italic,
                    muted,
                ));
                if canvas_font_popup_open {
                    c = c.push(
                        container(canvas_font_popup(
                            &canvas_font_name,
                            canvas_font_size,
                            canvas_font_bold,
                            canvas_font_italic,
                            muted,
                            input_bg,
                            input_bdr,
                        ))
                        .padding(iced::Padding {
                            top: 0.0,
                            right: 16.0,
                            bottom: 4.0,
                            left: 8.0,
                        }),
                    );
                }
                let sheet_colors: Vec<SheetColor> = SheetColor::ALL.to_vec();
                c = c.push(form_pick_row(
                    "Sheet Color",
                    sheet_colors,
                    sheet_color,
                    PanelMsg::SetSheetColor,
                    muted,
                ));
                c
            },
        ));
    }

    // Page Options (collapsible)
    {
        let paper_size = ctx.paper_size.clone();
        let format_mode = ctx.page_format_mode;
        let margin_v = ctx.margin_vertical;
        let margin_h = ctx.margin_horizontal;
        let origin = ctx.page_origin;
        let custom_w = ctx.custom_paper_w_mm;
        let custom_h = ctx.custom_paper_h_mm;
        col = col.push(collapsible_section(
            "prop_page_opts",
            "Page Options",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_label("Formatting and Size", muted));
                c = c.push(
                    container(
                        row![
                            seg_btn(
                                "Template",
                                format_mode == PageFormatMode::Template,
                                PanelMsg::SetPageFormatMode(PageFormatMode::Template),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr,
                            ),
                            seg_btn(
                                "Standard",
                                format_mode == PageFormatMode::Standard,
                                PanelMsg::SetPageFormatMode(PageFormatMode::Standard),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr,
                            ),
                            seg_btn(
                                "Custom",
                                format_mode == PageFormatMode::Custom,
                                PanelMsg::SetPageFormatMode(PageFormatMode::Custom),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr,
                            ),
                        ]
                        .spacing(0.0)
                        .width(Length::Fill),
                    )
                    .padding([2, 8]),
                );
                // Standard + Template modes share the size picker. Custom mode replaces
                // it with width/height inputs.
                match format_mode {
                    PageFormatMode::Standard | PageFormatMode::Template => {
                        let paper_options: Vec<String> =
                            PAPER_SIZES.iter().map(|s| (*s).to_string()).collect();
                        c = c.push(form_pick_row(
                            "Paper",
                            paper_options,
                            paper_size.clone(),
                            PanelMsg::SetPaperSize,
                            muted,
                        ));
                        let (w, h) = paper_dimensions(&paper_size);
                        let dims = format!("Width: {w:.0}mm  Height: {h:.0}mm");
                        c = c.push(container(text(dims).size(10).color(muted)).padding([3, 8]));
                        if matches!(format_mode, PageFormatMode::Template) {
                            c = c.push(
                                container(
                                    text("Template: using A-series defaults")
                                        .size(10)
                                        .color(muted),
                                )
                                .padding([0, 8]),
                            );
                        }
                    }
                    PageFormatMode::Custom => {
                        c = c.push(form_mm_edit_row(
                            "Width",
                            custom_w,
                            PanelMsg::SetCustomPaperWidth,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_mm_edit_row(
                            "Height",
                            custom_h,
                            PanelMsg::SetCustomPaperHeight,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                    }
                }
                c = c.push(form_label("Margin and Zones", muted));
                c = c.push(form_int_edit_row(
                    "Vertical",
                    margin_v,
                    PanelMsg::SetMarginVertical,
                    muted,
                    input_bg,
                    input_bdr,
                ));
                c = c.push(form_int_edit_row(
                    "Horizontal",
                    margin_h,
                    PanelMsg::SetMarginHorizontal,
                    muted,
                    input_bg,
                    input_bdr,
                ));
                let origin_opts: Vec<PageOrigin> =
                    vec![PageOrigin::UpperLeft, PageOrigin::LowerLeft];
                c = c.push(form_pick_row(
                    "Origin",
                    origin_opts,
                    origin,
                    PanelMsg::SetPageOrigin,
                    muted,
                ));
                c
            },
        ));
    }

    col
}

pub fn view_properties_parameters<'a>(
    muted: Color,
    primary: Color,
    border_c: Color,
    input_bg: Color,
    input_bdr: Color,
    seg_hover: Color,
) -> Column<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(section_hdr("\u{25BC} Parameters", primary, border_c));

    // Sub-tabs: All | Parameters | Rules
    col = col.push(
        container(
            row![
                seg_btn(
                    "All",
                    false,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr
                ),
                seg_btn(
                    "Parameters",
                    true,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr
                ),
                seg_btn(
                    "Rules",
                    false,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr
                ),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    // Table header
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(
            row![
                text("Name")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Value")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(4.0),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // Parameter rows (standard Altium document parameters)
    let params: &[(&str, &str)] = &[
        ("CurrentTime", "*"),
        ("CurrentDate", "*"),
        ("Time", "*"),
        ("Date", "*"),
        ("DocumentFullPathAndName", "*"),
        ("DocumentName", "*"),
        ("ModifiedDate", "*"),
        ("ApprovedBy", "*"),
        ("CheckedBy", "*"),
        ("Author", "*"),
        ("CompanyName", "*"),
        ("DrawnBy", "*"),
        ("Engineer", "*"),
        ("Organization", "*"),
        ("Title", "*"),
        ("Address1", "*"),
        ("Address2", "*"),
        ("Address3", "*"),
        ("Address4", "*"),
    ];

    for (name, val) in params {
        col = col.push(param_table_row(name, val, primary, muted, border_c));
    }

    col
}

/// Parameter table row with subtle bottom border.
pub fn param_table_row<'a, M: 'a>(
    name: &str,
    value: &str,
    name_c: Color,
    val_c: Color,
    border_c: Color,
) -> Element<'a, M> {
    column![
        container(
            row![
                text(name.to_string())
                    .size(11)
                    .color(name_c)
                    .width(Length::FillPortion(3))
                    .wrapping(iced::widget::text::Wrapping::None),
                text(value.to_string())
                    .size(11)
                    .color(val_c)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(4.0),
        )
        .padding([4, 8])
        .width(Length::Fill),
        thin_sep(border_c),
    ]
    .spacing(0)
    .into()
}

/// Properties panel tab button (General / Parameters).
pub fn props_tab_btn(
    label: &str,
    active: bool,
    msg: PanelMsg,
    text_active: Color,
    text_inactive: Color,
    hover_bg: Color,
    border_c: Color,
) -> Element<'static, PanelMsg> {
    let text_c = if active { text_active } else { text_inactive };
    iced::widget::button(text(label.to_string()).size(11).color(text_c))
        .padding([4, 12])
        .on_press(msg)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hover = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: if active || hover {
                    Some(Background::Color(hover_bg))
                } else {
                    None
                },
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border_c,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

/// Thin 1px separator line. `pub(super)` so sibling modules
/// (footprint_editor_properties, symbol_editor_properties, etc.)
/// extracted from this file can share the single implementation.
pub fn thin_sep<'a, M: 'a>(border_c: Color) -> Element<'a, M> {
    container(Space::new())
        .height(1.0)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(border_c)),
            ..container::Style::default()
        })
        .into()
}

/// Section header: bold label + separator line.
pub fn section_hdr<'a, M: 'a>(title: &str, text_c: Color, border_c: Color) -> Column<'a, M> {
    column![
        container(text(title.to_string()).size(12).color(text_c))
            .padding([6, 8])
            .width(Length::Fill),
        container(Space::new())
            .height(1.0)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border_c)),
                ..container::Style::default()
            }),
    ]
    .spacing(0)
}

/// Form row: label | styled input-like value display.
pub fn form_input_row<'a, M: 'a>(
    label: &str,
    value: &str,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, M> {
    container(
        row![
            property_label(label.to_string(), label_c),
            container(
                container(
                    text(value.to_string())
                        .size(11)
                        .color(Color::WHITE)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(Length::Fill)
                .clip(true),
            )
            .padding([3, 6])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(input_bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_border,
                },
                ..container::Style::default()
            }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | integer text_input (no spinner buttons).
pub fn form_int_edit_row<'a>(
    label: &str,
    value: u32,
    on_change: impl Fn(u32) -> PanelMsg + 'a + Clone,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, PanelMsg> {
    let text_value = value.to_string();
    let on_change_cl = on_change.clone();
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::text_input("", &text_value)
                .on_input(move |s| {
                    let parsed: u32 = s.trim().parse().unwrap_or(0);
                    (on_change_cl)(parsed.min(99))
                })
                .size(11)
                .padding([3, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .style(move |_: &Theme, _| iced::widget::text_input::Style {
                    background: Background::Color(input_bg),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_border,
                    },
                    icon: Color::TRANSPARENT,
                    placeholder: Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0),
                    value: Color::WHITE,
                    selection: Color::from_rgba8(0x4D, 0x52, 0x66, 0.6),
                }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | floating-point mm text_input (no spinner buttons).
pub fn form_mm_edit_row<'a>(
    label: &str,
    value: f32,
    on_change: impl Fn(f32) -> PanelMsg + 'a + Clone,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, PanelMsg> {
    let text_value = format!("{value:.1}");
    let on_change_cl = on_change.clone();
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::text_input("", &text_value)
                .on_input(move |s| {
                    let parsed: f32 = s.trim().parse().unwrap_or(0.0);
                    (on_change_cl)(parsed.clamp(1.0, 2000.0))
                })
                .size(11)
                .padding([3, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .style(move |_: &Theme, _| iced::widget::text_input::Style {
                    background: Background::Color(input_bg),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_border,
                    },
                    icon: Color::TRANSPARENT,
                    placeholder: Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0),
                    value: Color::WHITE,
                    selection: Color::from_rgba8(0x4D, 0x52, 0x66, 0.6),
                }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | pick_list (dropdown).
pub fn form_pick_row<'a, T>(
    label: &str,
    options: Vec<T>,
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::pick_list(options, Some(selected), on_change)
                .text_size(11)
                .padding([2, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | custom widget.
#[allow(dead_code)]
pub fn form_label_row<'a>(
    label: &str,
    control: Row<'a, PanelMsg>,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            container(control).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Checkbox form row.
pub fn form_check_row<'a>(
    label: &str,
    checked: bool,
    msg: PanelMsg,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            row![
                iced::widget::checkbox(checked)
                    .on_toggle(move |_| msg.clone())
                    .size(14)
                    .spacing(4),
                text(if checked { "On" } else { "Off" })
                    .size(11)
                    .color(if checked { Color::WHITE } else { label_c }),
            ]
            .spacing(8)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | pick_list for grid size presets (2.54 mm multiples).
#[allow(dead_code)]
pub fn form_grid_size_row(current_mm: f32, label_c: Color) -> Element<'static, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZES_MM};
    // Find the label that matches the current value (fallback to first).
    let selected: Option<&'static str> = GRID_SIZES_MM
        .iter()
        .zip(GRID_SIZE_LABELS.iter())
        .find(|(sz, _)| (**sz - current_mm).abs() < 1e-4)
        .map(|(_, lbl)| *lbl);
    container(
        row![
            container(
                text("Visible Grid".to_string())
                    .size(11)
                    .color(label_c)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(LABEL_W)
            .clip(true),
            iced::widget::pick_list(GRID_SIZE_LABELS, selected, |lbl: &'static str| {
                // Map label back to mm value
                let mm = GRID_SIZES_MM
                    .iter()
                    .zip(GRID_SIZE_LABELS.iter())
                    .find(|(_, l)| **l == lbl)
                    .map(|(v, _)| *v)
                    .unwrap_or(2.54);
                PanelMsg::SetGridSize(mm)
            },)
            .text_size(11)
            .width(Length::Fill),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// Altium-style grid row: [Label] [checkbox toggle] [pick_list] [shortcut hint]
/// Used for both "Visible Grid" (eye/visible toggle) and "Snap Grid" (snap enable toggle).
/// Labels and values are shown in the current `unit` (mm or mil).
#[allow(clippy::too_many_arguments)]
pub fn form_grid_row(
    label: &'static str,
    current_mm: f32,
    unit: Unit,
    has_checkbox: bool,
    on_size: impl Fn(f32) -> PanelMsg + 'static,
    label_c: Color,
    active: bool,
    on_toggle: PanelMsg,
) -> Element<'static, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZE_LABELS_MIL, GRID_SIZES_MM};

    let labels: &'static [&'static str] = if unit == Unit::Mil {
        GRID_SIZE_LABELS_MIL
    } else {
        GRID_SIZE_LABELS
    };

    let selected: Option<&'static str> = GRID_SIZES_MM
        .iter()
        .zip(labels.iter())
        .find(|(sz, _)| (**sz - current_mm).abs() < 1e-4)
        .map(|(_, lbl)| *lbl);

    let pick = iced::widget::pick_list(labels, selected, move |lbl: &'static str| {
        // Map label back to mm value (labels and GRID_SIZES_MM are parallel arrays)
        let mm = GRID_SIZE_LABELS
            .iter()
            .chain(GRID_SIZE_LABELS_MIL.iter())
            .zip(GRID_SIZES_MM.iter().chain(GRID_SIZES_MM.iter()))
            .find(|(l, _)| **l == lbl)
            .map(|(_, v)| *v)
            .unwrap_or(2.54);
        on_size(mm)
    })
    .text_size(11)
    .width(Length::Fill);

    let label_widget = text(label.to_string())
        .size(11)
        .color(label_c)
        .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
        .wrapping(iced::widget::text::Wrapping::None);

    let content: Element<PanelMsg> = if has_checkbox {
        // Snap Grid row: checkbox before pick_list
        row![
            label_widget,
            iced::widget::checkbox(active)
                .on_toggle(move |_| on_toggle.clone())
                .size(12)
                .spacing(4),
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        // Visible Grid row: no checkbox
        row![
            label_widget,
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    };

    container(content)
        .padding([2, PROPERTY_ROW_PAD_X])
        .width(Length::Fill)
        .into()
}

/// Form row: checkbox with a keyboard shortcut hint on the right.
pub fn form_check_row_shortcut<'a>(
    label: &'a str,
    value: bool,
    on_toggle: PanelMsg,
    shortcut: &'a str,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    let shortcut_owned = shortcut.to_string();
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::Word),
            row![
                iced::widget::checkbox(value)
                    .on_toggle(move |_| on_toggle.clone())
                    .size(12)
                    .spacing(4),
                Space::new().width(Length::Fill),
                text(shortcut_owned)
                    .size(9)
                    .color(label_c)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(4)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .clip(true)
    .into()
}

pub fn form_font_link_row<'a>(
    label: &'static str,
    current_family: &str,
    current_size_px: f32,
    _bold: bool,
    _italic: bool,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    let summary = format!("{current_family}, {:.0}px", current_size_px);

    container(
        row![
            text(label)
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::button(
                text(summary)
                    .size(11)
                    .color(Color::from_rgb(0.35, 0.7, 1.0))
                    .width(Length::Fill),
            )
            .on_press(PanelMsg::OpenCanvasFontPopup)
            .padding([1, 0])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let underline = matches!(status, iced::widget::button::Status::Hovered);
                iced::widget::button::Style {
                    background: None,
                    text_color: if underline {
                        Color::from_rgb(0.55, 0.82, 1.0)
                    } else {
                        Color::from_rgb(0.35, 0.7, 1.0)
                    },
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    ..Default::default()
                }
            }),
        ]
        .spacing(4)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

pub fn canvas_font_popup<'a>(
    current_family: &str,
    current_size_px: f32,
    bold: bool,
    italic: bool,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let families = crate::fonts::system_font_families();
    let family_pick = iced::widget::pick_list(
        families.as_slice(),
        Some(current_family.to_string()),
        PanelMsg::SetCanvasFont,
    )
    .text_size(11)
    .width(Length::Fill);

    let size_input = NumberInput::new(&current_size_px, 6.0..=36.0, PanelMsg::SetCanvasFontSize)
        .step(1.0)
        .width(Length::Fill)
        .padding(4);

    container(
        column![
            row![
                text("Canvas Font Settings").size(11).color(label_c),
                Space::new().width(Length::Fill),
                iced::widget::button(text("Close").size(10))
                    .on_press(PanelMsg::CloseCanvasFontPopup)
                    .padding([2, 6])
            ]
            .align_y(iced::Alignment::Center),
            row![
                text("Family").size(10).color(label_c).width(56),
                family_pick,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            row![text("Size").size(10).color(label_c).width(56), size_input,]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            row![
                text("Style").size(10).color(label_c).width(56),
                row![
                    iced::widget::checkbox(bold)
                        .on_toggle(PanelMsg::SetCanvasFontBold)
                        .size(12)
                        .spacing(4),
                    text("Bold").size(10).color(label_c),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
                row![
                    iced::widget::checkbox(italic)
                        .on_toggle(PanelMsg::SetCanvasFontItalic)
                        .size(12)
                        .spacing(4),
                    text("Italic").size(10).color(label_c),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            text("Applies immediately to canvas text rendering.")
                .size(9)
                .color(label_c),
        ]
        .spacing(6),
    )
    .padding([6, 8])
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(Background::Color(input_bg)),
        border: Border {
            color: input_bdr,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}

/// Form row: label | NumberInput (iced_aw) with step/bounds.
#[allow(dead_code)]
fn form_number_row<'a, T>(
    label: &str,
    value: T,
    bounds: impl std::ops::RangeBounds<T> + 'a,
    step: T,
    on_change: impl Fn(T) -> PanelMsg + 'static + Clone,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: num_traits::Num
        + num_traits::NumAssignOps
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + num_traits::Bounded
        + 'static,
{
    container(
        row![
            property_label(label.to_string(), label_c),
            NumberInput::new(&value, bounds, on_change)
                .step(step)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .padding(4),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | editable text_input that emits a PanelMsg on input.
pub fn form_edit_row<'a>(
    label: &str,
    value: &str,
    label_c: Color,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::text_input("", value)
                .on_input(on_input)
                .size(11)
                .padding(4)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Standalone label row (no value, used before segmented controls).
pub fn form_label<'a, M: 'a>(label: &str, label_c: Color) -> Element<'a, M> {
    container(text(label.to_string()).size(11).color(label_c))
        .padding([4, 8])
        .width(Length::Fill)
        .into()
}

/// Altium-style B/I/U/T (Bold / Italic / Underline / Strikethrough) row.
pub fn font_style_row<'a>(
    _label_c: Color,
    primary: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let btn = |glyph: &'static str, style: iced::font::Weight| -> Element<'static, PanelMsg> {
        iced::widget::button(
            text(glyph.to_string())
                .size(12)
                .color(primary)
                .font(iced::Font {
                    weight: style,
                    ..iced::Font::DEFAULT
                })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([4, 6])
        .on_press(PanelMsg::Noop)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(if hovered {
                    input_bdr
                } else {
                    input_bg
                })),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: primary,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    container(
        row![
            btn("B", iced::font::Weight::Bold),
            btn("I", iced::font::Weight::Normal),
            btn("U", iced::font::Weight::Normal),
            btn("T", iced::font::Weight::Normal),
        ]
        .spacing(2.0)
        .width(Length::Fill),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Net-attribute row: label | checkbox | text value | unit. Used for
/// "Power Net = 0.000 V" and "High Speed = 0.000 Hz".
pub fn net_numeric_row<'a>(
    label: &str,
    value: &str,
    unit: &str,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::checkbox(false)
                .on_toggle(|_| PanelMsg::Noop)
                .size(12)
                .spacing(4),
            container(text(value.to_string()).size(11).color(label_c),)
                .padding([3, 6])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(input_bg)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_bdr
                    },
                    ..container::Style::default()
                }),
            text(unit.to_string()).size(10).color(label_c),
        ]
        .spacing(6.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Parameters (Net) segmented tabs — All / Parameters / Rules / Classes.
pub fn net_params_tabs<'a>(
    primary: Color,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let tab = |label: &'static str, active: bool| -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        iced::widget::button(
            text(label.to_string())
                .size(11)
                .color(if active { fg_active } else { fg_inactive })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([3, 12])
        .on_press(PanelMsg::Noop)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(if active {
                    bg_active
                } else if hovered {
                    input_bdr
                } else {
                    input_bg
                })),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    let _ = label_c;
    container(
        row![
            tab("All", true),
            tab("Parameters", false),
            tab("Rules", false),
            tab("Classes", false),
        ]
        .spacing(4.0),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Two-column Name / Value header for the Parameters (Net) table.
pub fn net_params_header<'a>(label_c: Color, border_c: Color) -> Element<'a, PanelMsg> {
    container(
        row![
            text("Name".to_string())
                .size(10)
                .color(label_c)
                .width(Length::FillPortion(2)),
            text("Value".to_string())
                .size(10)
                .color(label_c)
                .width(Length::FillPortion(3)),
        ]
        .spacing(4.0),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
        },
        ..container::Style::default()
    })
    .into()
}

/// Empty-state row — centered muted text spanning the whole row.
pub fn empty_section_row<'a>(
    label: &str,
    label_c: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        container(text(label.to_string()).size(10).color(label_c))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
        },
        ..container::Style::default()
    })
    .into()
}

/// Add / edit / delete toolbar at the bottom of the Parameters table.
pub fn net_params_add_bar<'a>(
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let icon_btn = |label: &'static str| -> Element<'static, PanelMsg> {
        iced::widget::button(text(label.to_string()).size(11).color(label_c))
            .padding([4, 8])
            .on_press(PanelMsg::Noop)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(input_bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: label_c,
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    container(
        row![
            Space::new().width(Length::Fill),
            iced::widget::button(
                text("Add \u{25BE}".to_string())
                    .size(11)
                    .color(Color::WHITE)
            )
            .padding([4, 12])
            .on_press(PanelMsg::Noop)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(input_bdr)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr
                },
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            }),
            icon_btn("\u{270E}"),
            icon_btn("\u{1F5D1}"),
        ]
        .spacing(4.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Altium-style 3x3 justification picker with proper SVG arrow icons.
/// Only horizontal is wired to state for now; vertical slots toggle visually
/// but don't mutate the label.
pub fn justification_grid(
    id: uuid::Uuid,
    rotation_deg: f64,
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    muted: Color,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let _ = muted;

    // Cell size mimics Altium's compact 24×24 px anchor picker.
    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: iced::widget::svg::Handle,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget =
            iced::widget::svg(handle)
                .width(12.0)
                .height(12.0)
                .style(move |_: &Theme, _| iced::widget::svg::Style {
                    color: Some(if active { fg_active } else { fg_inactive }),
                });
        iced::widget::button(
            container(svg_widget)
                .width(Length::Fill)
                .height(Length::Fill)
                .center(Length::Fill),
        )
        .width(CELL_SIZE)
        .height(CELL_SIZE)
        .padding(0)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let bg = if active {
                bg_active
            } else if hovered {
                input_bdr
            } else {
                input_bg
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LabelDir {
        Left,
        Up,
        Right,
        Down,
    }

    let normalize_rot = |deg: f64| {
        let r = (deg.round() as i32) % 360;
        if r < 0 { r + 360 } else { r }
    };

    let current_dir = {
        match normalize_rot(rotation_deg) {
            90 => LabelDir::Up,
            270 => LabelDir::Down,
            180 => {
                if matches!(h, HAlign::Right) {
                    LabelDir::Right
                } else {
                    LabelDir::Left
                }
            }
            _ => {
                if matches!(h, HAlign::Right) {
                    LabelDir::Left
                } else {
                    LabelDir::Right
                }
            }
        }
    };

    let to_msg = |dir: LabelDir| -> PanelMsg {
        match dir {
            LabelDir::Right => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Left),
            LabelDir::Left => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Right),
            LabelDir::Up => PanelMsg::EditLabelDirection(id, 90.0, HAlign::Left),
            LabelDir::Down => PanelMsg::EditLabelDirection(id, 270.0, HAlign::Left),
        }
    };

    let hl = |dir: LabelDir| current_dir == dir;

    iced::widget::column![
        iced::widget::row![
            cell(
                crate::icons::icon_justify_tl(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
            cell(
                crate::icons::icon_justify_t(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
            cell(
                crate::icons::icon_justify_tr(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_l(theme),
                hl(LabelDir::Left),
                to_msg(LabelDir::Left)
            ),
            cell(
                crate::icons::icon_justify_c(theme),
                false,
                to_msg(current_dir)
            ),
            cell(
                crate::icons::icon_justify_r(theme),
                hl(LabelDir::Right),
                to_msg(LabelDir::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_bl(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
            cell(
                crate::icons::icon_justify_b(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
            cell(
                crate::icons::icon_justify_br(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
        ]
        .spacing(2),
    ]
    .spacing(2)
    .into()
}

/// Pre-placement 3x3 justification picker. Same visual grid as the
/// selection-aware `justification_grid` but dispatches to the
/// `SetPrePlacementJustifyH` message family (no UUID needed).
pub fn preplacement_justification_grid(
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    muted: Color,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let _ = muted;

    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: iced::widget::svg::Handle,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget =
            iced::widget::svg(handle)
                .width(12.0)
                .height(12.0)
                .style(move |_: &Theme, _| iced::widget::svg::Style {
                    color: Some(if active { fg_active } else { fg_inactive }),
                });
        iced::widget::button(
            container(svg_widget)
                .width(Length::Fill)
                .height(Length::Fill)
                .center(Length::Fill),
        )
        .width(CELL_SIZE)
        .height(CELL_SIZE)
        .padding(0)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let bg = if active {
                bg_active
            } else if hovered {
                input_bdr
            } else {
                input_bg
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    let hl_mid = |target: HAlign| -> bool { h == target };
    iced::widget::column![
        iced::widget::row![
            cell(
                crate::icons::icon_justify_tl(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_t(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_tr(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_l(theme),
                hl_mid(HAlign::Left),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_c(theme),
                hl_mid(HAlign::Center),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_r(theme),
                hl_mid(HAlign::Right),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_bl(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_b(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_br(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
    ]
    .spacing(2)
    .into()
}

/// Tab button in the Custom Selection Filters tab strip. Active tab
/// gets a filled background; both states use the theme accent for the
/// border so the section reads as one piece with the chips and the
/// Active Bar dropdown.
pub fn custom_filter_tab(
    label: String,
    active: bool,
    idx: usize,
    hover_bg: Color,
    border_c: Color,
) -> Element<'static, PanelMsg> {
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x99, 0x9D, 0xAE, 1.0);
    iced::widget::button(
        text(label)
            .size(11)
            .color(if active { text_on } else { text_off }),
    )
    .padding([3, 10])
    .on_press(PanelMsg::SelectCustomFilterTab(idx))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered if !active => Background::Color(hover_bg),
            _ if active => Background::Color(active_bg),
            _ => Background::Color(Color::TRANSPARENT),
        };
        iced::widget::button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            text_color: if active { text_on } else { text_off },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// Member chip for a custom-filter preset card (Properties panel).
/// Border colour matches the Active Bar Filter dropdown chips (theme
/// accent), so chip styling stays consistent across both surfaces.
pub fn preset_chip(
    label: &str,
    preset_idx: usize,
    filter: crate::active_bar::SelectionFilter,
    enabled: bool,
    hover_bg: Color,
    border_c: Color,
) -> Element<'static, PanelMsg> {
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
    iced::widget::button(
        text(label.to_string())
            .size(10)
            .color(if enabled { text_on } else { text_off })
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3, 8])
    .on_press(PanelMsg::ToggleCustomFilterPresetMember(preset_idx, filter))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Background::Color(hover_bg),
            _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
        };
        iced::widget::button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            text_color: if enabled { text_on } else { text_off },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// Selection filter tag button — Altium pill with active/inactive state.
#[allow(dead_code)]
pub fn tag_btn(
    label: &str,
    filter: crate::active_bar::SelectionFilter,
    enabled: bool,
    hover_bg: Color,
) -> Element<'static, PanelMsg> {
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let active_border = Color::from_rgba8(0x4D, 0x52, 0x66, 1.0);
    let inactive_border = Color::from_rgba8(0x33, 0x36, 0x44, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
    iced::widget::button(
        text(label.to_string())
            .size(10)
            .color(if enabled { text_on } else { text_off })
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3, 8])
    .on_press(PanelMsg::ToggleSelectionFilter(filter))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Background::Color(hover_bg),
            _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
        };
        iced::widget::button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 12.0.into(),
                color: if enabled {
                    active_border
                } else {
                    inactive_border
                },
            },
            text_color: if enabled { text_on } else { text_off },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// Segmented button (for units toggle etc).
#[allow(clippy::too_many_arguments)]
pub fn seg_btn<'a>(
    label: &str,
    active: bool,
    msg: PanelMsg,
    active_bg: Color,
    text_active: Color,
    text_inactive: Color,
    hover_bg: Color,
    seg_border: Color,
) -> Element<'a, PanelMsg> {
    let bg = if active {
        active_bg
    } else {
        Color::TRANSPARENT
    };
    let text_c = if active { text_active } else { text_inactive };
    iced::widget::button(
        text(label.to_string())
            .size(11)
            .color(text_c)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([4, 0])
    .width(Length::Fill)
    .on_press(msg)
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: Some(Background::Color(if hovered && !active {
                hover_bg
            } else {
                bg
            })),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: seg_border,
            },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

// ─── ERC Panel ────────────────────────────────────────────────
