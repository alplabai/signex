//! Properties panel views — Custom Selection Filters section, the
//! General + Page Options tab (`view_properties_general`), and the
//! document-parameter table (`view_properties_parameters`) — plus the
//! filter-tab / preset-chip / segmented-button chrome those views use.
//! Moved verbatim from the former single-file `properties_parameters`
//! module.

use super::super::*;
use crate::panels::PaperEdge;
use crate::styles::{Spacing, TypeScale};
use iced::widget::column;
use signex_types::schematic::{SheetOrientation, SheetSize};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Custom Selection Filters collapsible section — tabbed editor for up to
/// `CUSTOM_FILTER_PRESET_LIMIT` named presets. Pulled out of
/// `view_properties_general` so the schematic Properties panel and the
/// Footprint editor's Properties panel render the EXACT same widget.
pub fn view_custom_selection_filters_section<'a>(
    presets: Vec<crate::active_bar::CustomFilterPreset>,
    active_custom_filter_tab: usize,
    collapsed_sections: &'a CollapsedSections,
    palette: PanelPalette,
) -> Column<'a, PanelMsg> {
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        accent: accent_c,
        tag_hover,
        ..
    } = palette;
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
                                .size(TS.body)
                                .color(muted_c),
                        )
                        .padding([SP.s, SP.m]),
                    );
                    c = c.push(
                        container(
                            iced::widget::button(text("+ Add Filter").size(TS.body))
                                .padding([SP.s, 10.0])
                                .on_press(PanelMsg::AddCustomFilterPreset)
                                .style(move |theme: &Theme, status| {
                                    add_filter_button_style(theme, status, primary_c)
                                }),
                        )
                        .padding([SP.s, SP.m]),
                    );
                    return c;
                }
                let mut tabs = iced::widget::Row::new()
                    .spacing(SP.xs)
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
                        iced::widget::button(text("+").size(TS.label).color(primary_c))
                            .padding([3, 10])
                            .on_press(PanelMsg::AddCustomFilterPreset),
                    );
                }
                c = c.push(container(tabs).padding([SP.s, SP.m]));
                let preset = &presets[active_tab];
                let included: std::collections::HashSet<SelectionFilter> =
                    preset.filters.iter().copied().collect();
                let header = row![
                    iced::widget::text_input("Preset name", &preset.name)
                        .size(TS.body)
                        .padding([3, 6])
                        .on_input(move |s| PanelMsg::RenameCustomFilterPreset(active_tab, s))
                        .width(Length::Fill),
                    iced::widget::button(text("Delete").size(TS.caption).color(primary_c))
                        .padding([3.0, SP.m])
                        .on_press(PanelMsg::RemoveCustomFilterPreset(active_tab)),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center);
                let mut wrap = Wrap::new().spacing(SP.s).line_spacing(4.0);
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
                    container(column![header, container(wrap).padding([SP.s, 0.0])].spacing(SP.s))
                        .padding([6.0, SP.m]),
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
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let PanelPalette {
        input_bg,
        input_bdr,
        seg_hover,
        panel_bg,
        ..
    } = palette;

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Custom Selection Filters — shared widget reused by the Footprint
    // editor's Properties panel so both editors render the EXACT same
    // chrome (tabs, chips, name input, accent border).
    col = col.push(view_custom_selection_filters_section(
        ctx.custom_filter_presets.clone(),
        ctx.active_custom_filter_tab,
        &ctx.collapsed_sections,
        palette,
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
                    .padding([SP.xs, SP.m]),
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
        let sheet_frame = ctx.sheet_frame;
        let origin = ctx.page_origin;
        let document_font = ctx.document_font_family.clone();
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
                    .padding([SP.xs, SP.m]),
                );
                let (sheet_size, orientation) = SheetSize::parse(&paper_size);
                // Standard + Template modes share the size picker. Custom mode
                // replaces it with width/height inputs; both write the sheet's
                // own size through the engine, so the canvas, the PDF and the
                // saved file agree.
                match format_mode {
                    PageFormatMode::Standard | PageFormatMode::Template => {
                        let paper_options = paper_size_options();
                        c = c.push(form_pick_row(
                            "Paper",
                            paper_options,
                            SheetSize::parse(&paper_size)
                                .0
                                .token(SheetOrientation::Landscape),
                            PanelMsg::SetPaperSize,
                            muted,
                        ));
                        if matches!(format_mode, PageFormatMode::Template) {
                            c = c.push(
                                container(
                                    text("Template: using A-series defaults")
                                        .size(TS.caption)
                                        .color(muted),
                                )
                                .padding([0.0, SP.m]),
                            );
                        }
                    }
                    PageFormatMode::Custom => {
                        let (w, h) = paper_dimensions(&paper_size);
                        c = c.push(form_mm_edit_row(
                            "Width",
                            w,
                            ctx.paper_edge_drafts
                                .iter()
                                .find(|d| {
                                    d.edge == PaperEdge::Width
                                        && ctx.title_block_sheet.as_ref() == Some(&d.sheet)
                                })
                                .map(|d| d.text.as_str()),
                            PaperEdge::Width,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_mm_edit_row(
                            "Height",
                            h,
                            ctx.paper_edge_drafts
                                .iter()
                                .find(|d| {
                                    d.edge == PaperEdge::Height
                                        && ctx.title_block_sheet.as_ref() == Some(&d.sheet)
                                })
                                .map(|d| d.text.as_str()),
                            PaperEdge::Height,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                    }
                }
                if sheet_size.is_recognized() {
                    c = c.push(form_pick_row(
                        "Orientation",
                        vec![SheetOrientation::Landscape, SheetOrientation::Portrait],
                        orientation,
                        PanelMsg::SetSheetOrientation,
                        muted,
                    ));
                }
                let (w, h) = paper_dimensions(&paper_size);
                let dims = format!("Width: {w:.0}mm  Height: {h:.0}mm");
                c = c
                    .push(container(text(dims).size(TS.caption).color(muted)).padding([3.0, SP.m]));
                c = c.push(form_label("Border and Zones", muted));
                c = c.push(form_check_row(
                    "Show Border",
                    sheet_frame.show_border,
                    PanelMsg::EditSheetFrame(SheetFrameEdit::ShowBorder(!sheet_frame.show_border)),
                    muted,
                    panel_bg,
                ));
                c = c.push(form_check_row(
                    "Show Title Block",
                    sheet_frame.show_title_block,
                    PanelMsg::EditSheetFrame(SheetFrameEdit::ShowTitleBlock(
                        !sheet_frame.show_title_block,
                    )),
                    muted,
                    panel_bg,
                ));
                let zone_counts: Vec<u8> = (0..=signex_types::schematic::MAX_ZONES).collect();
                c = c.push(form_pick_row(
                    "Vertical",
                    zone_counts.clone(),
                    sheet_frame.vertical_zones,
                    |n| PanelMsg::EditSheetFrame(SheetFrameEdit::VerticalZones(n)),
                    muted,
                ));
                c = c.push(form_pick_row(
                    "Horizontal",
                    zone_counts,
                    sheet_frame.horizontal_zones,
                    |n| PanelMsg::EditSheetFrame(SheetFrameEdit::HorizontalZones(n)),
                    muted,
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
                c = c.push(form_label("Text", muted));
                c = c.push(crate::panels::element_properties::font_family_row(
                    "Default Font",
                    document_font.as_deref(),
                    muted,
                    PanelMsg::SetDocumentFontFamily,
                ));
                c
            },
        ));
    }

    // Title Block (collapsible, #1080)
    col = col.push(super::view_title_block_section(
        ctx, muted, primary, border_c,
    ));

    col = col.push(view_net_classes_section(ctx, palette));

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
        .padding([SP.s, SP.m]),
    );

    // Table header
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(
            row![
                text("Name")
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Value")
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(SP.s),
        )
        .padding([SP.s, SP.m])
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
                    .size(TS.body)
                    .color(name_c)
                    .width(Length::FillPortion(3))
                    .wrapping(iced::widget::text::Wrapping::None),
                text(value.to_string())
                    .size(TS.body)
                    .color(val_c)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(SP.s),
        )
        .padding([SP.s, SP.m])
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
    iced::widget::button(text(label.to_string()).size(TS.body).color(text_c))
        .padding([SP.s, SP.l])
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
            .size(TS.body)
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
            .size(TS.caption)
            .color(if enabled { text_on } else { text_off })
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3.0, SP.m])
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

/// Primary-fill button whose label keeps `preferred` text where it is
/// readable on the fill the theme paints behind it.
pub(crate) fn add_filter_button_style(
    theme: &Theme,
    status: iced::widget::button::Status,
    preferred: Color,
) -> iced::widget::button::Style {
    use crate::styles::tokens::{UNREADABLE_CONTRAST, contrast_ratio};
    use iced::widget::button::Status;
    let mut style = iced::widget::button::primary(theme, status);
    // One label for every state, so it cannot flip on hover: `preferred`
    // when it is visible on all three fills, else white or black, whichever
    // reads better on the weakest fill.
    let fills: Vec<Color> = [Status::Active, Status::Hovered, Status::Pressed]
        .into_iter()
        .filter_map(
            |s| match iced::widget::button::primary(theme, s).background {
                Some(Background::Color(fill)) => Some(fill),
                _ => None,
            },
        )
        .collect();
    let worst = |label: Color| {
        fills
            .iter()
            .map(|fill| contrast_ratio(label, *fill))
            .fold(f32::INFINITY, f32::min)
    };
    style.text_color = if worst(preferred) >= UNREADABLE_CONTRAST {
        preferred
    } else if worst(Color::WHITE) >= worst(Color::BLACK) {
        Color::WHITE
    } else {
        Color::BLACK
    };
    style
}

/// Label colour of a segmented-button cell: the active cell sits on
/// `active_fill`, so its text must stay readable on that fill.
pub(crate) fn seg_text(
    active: bool,
    text_active: Color,
    text_inactive: Color,
    active_fill: Color,
) -> Color {
    if active {
        crate::styles::tokens::keep_readable_on(text_active, active_fill)
    } else {
        text_inactive
    }
}

/// Segmented button (for units toggle etc).
#[expect(
    clippy::too_many_arguments,
    reason = "a general-properties row builder whose parameters are all independent"
)]
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
    let text_c = seg_text(active, text_active, text_inactive, active_bg);
    iced::widget::button(
        text(label.to_string())
            .size(TS.body)
            .color(text_c)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([SP.s, 0.0])
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
