//! Properties panel views — Custom Selection Filters section, the
//! General + Page Options tab (`view_properties_general`), and the
//! document-parameter table (`view_properties_parameters`) — plus the
//! filter-tab / preset-chip / segmented-button chrome those views use.
//! Moved verbatim from the former single-file `properties_parameters`
//! module.

use super::super::*;
use iced::widget::column;

/// Custom Selection Filters collapsible section — tabbed editor for up to
/// `CUSTOM_FILTER_PRESET_LIMIT` named presets. Pulled out of
/// `view_properties_general` so the schematic Properties panel and the
/// Footprint editor's Properties panel render the EXACT same widget.
pub fn view_custom_selection_filters_section<'a>(
    presets: Vec<crate::active_bar::CustomFilterPreset>,
    active_custom_filter_tab: usize,
    collapsed_sections: &'a CollapsedSections,
    muted: Color,
    primary: Color,
    border_c: Color,
    accent_c: Color,
    tag_hover: Color,
) -> Column<'a, PanelMsg> {
    use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, SelectionFilter};
    let active_tab = active_custom_filter_tab.min(presets.len().saturating_sub(1));
    let muted_c = muted;
    let primary_c = primary;
    Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(collapsible_section(
            "prop_sel_filter",
            "Custom Selection Filters",
            collapsed_sections,
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
        ))
}

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

    // Custom Selection Filters — shared widget reused by the Footprint
    // editor's Properties panel so both editors render the EXACT same
    // chrome (tabs, chips, name input, accent border).
    let accent_c = crate::styles::ti(ctx.tokens.accent);
    col = col.push(view_custom_selection_filters_section(
        ctx.custom_filter_presets.clone(),
        ctx.active_custom_filter_tab,
        &ctx.collapsed_sections,
        muted,
        primary,
        border_c,
        accent_c,
        tag_hover,
    ));

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
