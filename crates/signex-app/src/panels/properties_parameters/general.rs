//! Properties panel views — Custom Selection Filters section, the
//! General + Page Options tab (`view_properties_general`), and the
//! document-parameter table (`view_properties_parameters`) — plus the
//! filter-tab / preset-chip / segmented-button chrome those views use.
//! Moved verbatim from the former single-file `properties_parameters`
//! module.

use super::super::*;
use crate::panels::PaperEdge;
use crate::panels::form_look;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::column;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::{SheetOrientation, SheetSize};
use signex_widgets::Label;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The section on the Graphite section chrome.
fn selection_filters_section_for<'a>(
    ctx: &'a PanelContext,
    palette: PanelPalette,
) -> Column<Element<'a, PanelMsg>> {
    let presets = ctx.custom_filter_presets.clone();
    let tab = ctx.active_custom_filter_tab;
    let i18n = std::sync::Arc::clone(&ctx.i18n);
    Column::new().spacing(0).width(Length::Fill).push(
        ctx.collapsible(
            "prop_sel_filter",
            "Custom Selection Filters",
            &ctx.collapsed_sections,
            move || selection_filters_body(presets, tab, palette, &i18n),
        )
        .boxed(),
    )
}

/// Body of the Custom Selection Filters section.
fn selection_filters_body<'a>(
    presets: Vec<crate::active_bar::CustomFilterPreset>,
    active_custom_filter_tab: usize,
    palette: PanelPalette,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let PanelPalette { muted, primary, .. } = palette;
    use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, SelectionFilter};
    let active_tab = active_custom_filter_tab.min(presets.len().saturating_sub(1));
    let muted_c = muted;
    let primary_c = primary;
    let mut c: Column<Element<'a, PanelMsg>> = Column::new().spacing(6).width(Length::Fill);
    if presets.is_empty() {
        c = c.push(
            container(
                text(t!(i18n, ids::PANELS_FILTERS_NO_PRESETS))
                    .size(TS.body)
                    .color(muted_c),
            )
            .padding([SP.s, SP.m])
            .boxed(),
        );
        c = c.push(
            container(
                signex_widgets::button(
                    text(t!(i18n, ids::PANELS_FILTERS_ADD_FILTER)).size(TS.body),
                )
                .padding([SP.s, 10.0])
                .on_press(PanelMsg::AddCustomFilterPreset)
                .style(form_look::ghost_button),
            )
            .padding([SP.s, SP.m])
            .boxed(),
        );
        return c;
    }
    let mut tabs: iced::widget::Row<Element<'a, PanelMsg>> = iced::widget::Row::new()
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center);
    for (idx, preset) in presets.iter().enumerate() {
        let label: Label<'static> = if preset.name.trim().is_empty() {
            t!(i18n, ids::PANELS_FILTERS_UNNAMED, "number" => idx + 1).into()
        } else {
            UserText::new(preset.name.clone()).into()
        };
        tabs = tabs.push(custom_filter_tab(label, idx == active_tab, idx));
    }
    if presets.len() < CUSTOM_FILTER_PRESET_LIMIT {
        tabs = tabs.push(
            signex_widgets::button(
                text(t!(i18n, ids::PANELS_FILTERS_ADD_PRESET))
                    .size(TS.label)
                    .color(primary_c),
            )
            .padding([3, 10])
            .on_press(PanelMsg::AddCustomFilterPreset)
            .style(form_look::ghost_button)
            .boxed(),
        );
    }
    c = c.push(container(tabs).padding([SP.s, SP.m]).boxed());
    let preset = &presets[active_tab];
    let included: std::collections::HashSet<SelectionFilter> =
        preset.filters.iter().copied().collect();
    let header = row![
        signex_widgets::text_input(
            t!(i18n, ids::PANELS_FILTERS_PRESET_NAME),
            preset.name.clone()
        )
        .size(TS.body)
        .padding([3, 6])
        .on_input(move |s| PanelMsg::RenameCustomFilterPreset(active_tab, s))
        .style(form_look::text_input_style)
        .width(Length::Fill)
        .boxed(),
        signex_widgets::button(
            text(t!(i18n, ids::PANELS_FILTERS_DELETE))
                .size(TS.body)
                .color(primary_c),
        )
        .padding([3.0, SP.m])
        .on_press(PanelMsg::RemoveCustomFilterPreset(active_tab))
        .style(form_look::ghost_button)
        .boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let mut wrap = Wrap::new().spacing(SP.s).line_spacing(4.0);
    for &f in SelectionFilter::ALL {
        wrap = wrap.push(preset_chip(
            t!(i18n, f.message()).into(),
            active_tab,
            f,
            included.contains(&f),
        ));
    }
    c = c.push(
        container(
            column![header.boxed(), container(wrap).padding([SP.s, 0.0]).boxed()].spacing(SP.s),
        )
        .padding([6.0, SP.m])
        .boxed(),
    );
    c
}

pub fn view_properties_general<'a>(
    ctx: &'a PanelContext,
    primary: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a I18n = &ctx.i18n;
    // Graphite muted ink for labels and hints, on the shared label column.
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let label = LabelInk::graphite(muted);
    let hairline = crate::styles::tokens::resolve(&ctx.tokens).line_soft;
    // Derive button/input colors from tokens (Copy values captured in closures)
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let PanelPalette {
        input_bg,
        input_bdr,
        panel_bg,
        ..
    } = palette;

    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

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
        col = col.push(
            ctx.collapsible(
                "prop_general",
                "General",
                &ctx.collapsed_sections,
                move || {
                    let mut c: Column<Element<'a, PanelMsg>> =
                        Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_label(t!(i18n, ids::PANELS_PROPERTIES_UNITS)));
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
                                ),
                                seg_btn(
                                    "mils",
                                    unit == Unit::Mil,
                                    PanelMsg::SetUnit(Unit::Mil),
                                    input_bg,
                                    primary,
                                    muted,
                                ),
                            ]
                            .spacing(0.0)
                            .width(Length::Fill),
                        )
                        .padding([SP.xs, SP.m])
                        .style(form_look::segmented_frame)
                        .boxed(),
                    );
                    // Altium-style: Visible Grid and Snap Grid are independent
                    c = c.push(form_grid_row(
                        "Visible Grid",
                        visible_grid_mm,
                        unit,
                        false,
                        PanelMsg::SetVisibleGridSize,
                        label,
                        grid_visible,
                        PanelMsg::ToggleGrid,
                    ));
                    c = c.push(form_grid_row(
                        "Snap Grid",
                        grid_size_mm,
                        unit,
                        true,
                        PanelMsg::SetGridSize,
                        label,
                        snap_enabled,
                        PanelMsg::ToggleSnap,
                    ));
                    c = c.push(form_check_row_shortcut(
                        "Snap to Hotspots",
                        snap_hotspots,
                        PanelMsg::ToggleSnapHotspots,
                        "Shift+E",
                        label,
                    ));
                    c = c.push(form_font_link_row(
                        "Canvas Font",
                        &canvas_font_name,
                        canvas_font_size,
                        canvas_font_bold,
                        canvas_font_italic,
                        label,
                    ));
                    if canvas_font_popup_open {
                        c = c.push(
                            container(canvas_font_popup(
                                &canvas_font_name,
                                canvas_font_size,
                                canvas_font_bold,
                                canvas_font_italic,
                                muted,
                                palette,
                                i18n,
                            ))
                            .padding(iced::Padding {
                                top: 0.0,
                                right: 16.0,
                                bottom: 4.0,
                                left: 8.0,
                            })
                            .boxed(),
                        );
                    }
                    c = c.push(form_pick_row(
                        "Sheet Color",
                        crate::panels::SchematicSheetColor::options(),
                        sheet_color,
                        PanelMsg::SetSheetColor,
                        label,
                    ));
                    c
                },
            )
            .boxed(),
        );
    }

    // Page Options (collapsible)
    col = col.push(thin_sep(hairline));
    {
        let paper_size = ctx.paper_size.clone();
        let format_mode = ctx.page_format_mode;
        let sheet_frame = ctx.sheet_frame;
        let origin = ctx.page_origin;
        let document_font = ctx.document_font_family.clone();
        col = col.push(
            ctx.collapsible(
                "prop_page_opts",
                "Page Options",
                &ctx.collapsed_sections,
                move || {
                    let mut c: Column<Element<'a, PanelMsg>> =
                        Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_label(t!(i18n, ids::PANELS_PROPERTIES_FORMATTING)));
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
                                ),
                                seg_btn(
                                    "Standard",
                                    format_mode == PageFormatMode::Standard,
                                    PanelMsg::SetPageFormatMode(PageFormatMode::Standard),
                                    input_bg,
                                    primary,
                                    muted,
                                ),
                                seg_btn(
                                    "Custom",
                                    format_mode == PageFormatMode::Custom,
                                    PanelMsg::SetPageFormatMode(PageFormatMode::Custom),
                                    input_bg,
                                    primary,
                                    muted,
                                ),
                            ]
                            .spacing(0.0)
                            .width(Length::Fill),
                        )
                        .padding([SP.xs, SP.m])
                        .style(form_look::segmented_frame)
                        .boxed(),
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
                                label,
                            ));
                            if matches!(format_mode, PageFormatMode::Template) {
                                c = c.push(
                                    container(
                                        text(t!(i18n, ids::PANELS_PROPERTIES_TEMPLATE_DEFAULTS))
                                            .size(TS.caption)
                                            .color(muted),
                                    )
                                    .padding([0.0, SP.m])
                                    .boxed(),
                                );
                            }
                        }
                        PageFormatMode::Custom => {
                            let (w, h) = paper_dimensions(&paper_size);
                            c = c.push(form_mm_edit_row(
                                "Width",
                                w,
                                ctx.title_block_sheet
                                    .as_deref()
                                    .and_then(|sheet| {
                                        ctx.drafts.paper_edge_text(sheet, PaperEdge::Width)
                                    })
                                    .as_deref(),
                                PaperEdge::Width,
                                label,
                                input_bg,
                                input_bdr,
                            ));
                            c = c.push(form_mm_edit_row(
                                "Height",
                                h,
                                ctx.title_block_sheet
                                    .as_deref()
                                    .and_then(|sheet| {
                                        ctx.drafts.paper_edge_text(sheet, PaperEdge::Height)
                                    })
                                    .as_deref(),
                                PaperEdge::Height,
                                label,
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
                            label,
                        ));
                    }
                    let (w, h) = paper_dimensions(&paper_size);
                    let dims = format!("Width: {w:.0}mm  Height: {h:.0}mm");
                    c = c.push(
                        container(text(raw(dims)).size(TS.caption).color(muted))
                            .padding([3.0, SP.m])
                            .boxed(),
                    );
                    c = c.push(form_label(t!(i18n, ids::PANELS_PROPERTIES_BORDER_ZONES)));
                    c = c.push(form_check_row(
                        "Show Border",
                        sheet_frame.show_border,
                        PanelMsg::EditSheetFrame(SheetFrameEdit::ShowBorder(
                            !sheet_frame.show_border,
                        )),
                        label,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        "Show Title Block",
                        sheet_frame.show_title_block,
                        PanelMsg::EditSheetFrame(SheetFrameEdit::ShowTitleBlock(
                            !sheet_frame.show_title_block,
                        )),
                        label,
                        panel_bg,
                    ));
                    let zone_counts: Vec<u8> = (0..=signex_types::schematic::MAX_ZONES).collect();
                    c = c.push(form_pick_row(
                        "Vertical",
                        zone_counts.clone(),
                        sheet_frame.vertical_zones,
                        |n| PanelMsg::EditSheetFrame(SheetFrameEdit::VerticalZones(n)),
                        label,
                    ));
                    c = c.push(form_pick_row(
                        "Horizontal",
                        zone_counts,
                        sheet_frame.horizontal_zones,
                        |n| PanelMsg::EditSheetFrame(SheetFrameEdit::HorizontalZones(n)),
                        label,
                    ));
                    let origin_opts: Vec<PageOrigin> =
                        vec![PageOrigin::UpperLeft, PageOrigin::LowerLeft];
                    c = c.push(form_pick_row(
                        "Origin",
                        origin_opts,
                        origin,
                        PanelMsg::SetPageOrigin,
                        label,
                    ));
                    c = c.push(form_label(t!(i18n, ids::PANELS_PROPERTIES_TEXT)));
                    c = c.push(crate::panels::element_properties::font_family_row(
                        "Default Font",
                        document_font.as_deref(),
                        muted,
                        PanelMsg::SetDocumentFontFamily,
                    ));
                    c
                },
            )
            .boxed(),
        );
    }

    // Title Block (collapsible, #1080)
    col = col.push(thin_sep(hairline));
    col = col.push(super::view_title_block_section(ctx));

    col = col.push(thin_sep(hairline));
    col = col.push(view_net_classes_section(ctx).boxed());

    // Custom Selection Filters sit last: Document Options opens on General
    // (#2262); the presets stay here and in the active bar's Filter menu.
    col = col.push(thin_sep(hairline));
    col = col.push(selection_filters_section_for(ctx, palette).boxed());

    col
}

/// Standard document parameters the Parameters tab lists.
pub(super) const DOCUMENT_PARAMETERS: &[(&str, &str)] = &[
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

pub fn view_properties_parameters<'a>(
    tokens: &ThemeTokens,
    muted: Color,
    primary: Color,
    input_bg: Color,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(form_look::section_label("Parameters", tokens, i18n));

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
                ),
                seg_btn(
                    "Parameters",
                    true,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                ),
                seg_btn(
                    "Rules",
                    false,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                ),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([SP.s, SP.m])
        .style(form_look::segmented_frame)
        .boxed(),
    );

    col.push(super::graphite_table::parameter_table(
        tokens,
        DOCUMENT_PARAMETERS,
    ))
}

/// Properties panel tab button (General / Parameters).
pub fn props_tab_btn(
    label: signex_i18n::Localized,
    active: bool,
    msg: PanelMsg,
    text_active: Color,
    text_inactive: Color,
) -> Element<'static, PanelMsg> {
    let text_c = if active { text_active } else { text_inactive };
    signex_widgets::button(text(label).size(TS.body).color(text_c))
        .padding([SP.s, SP.l])
        .on_press(msg)
        .style(move |theme: &Theme, status: iced::widget::button::Status| {
            let hover = matches!(status, iced::widget::button::Status::Hovered);
            form_look::tab_chip(theme, active, hover)
        })
        .boxed()
}

/// Graphite filter chip: accent-soft fill when on, hairline outline when
/// off, radius 7.
fn graphite_filter_chip(theme: &Theme, on: bool, hovered: bool) -> iced::widget::button::Style {
    let l = form_look::FormLook::of(theme);
    let fill = if on {
        Some(Background::Color(l.accent_soft))
    } else if hovered {
        Some(Background::Color(l.sunken))
    } else {
        None
    };
    iced::widget::button::Style {
        background: fill,
        text_color: if on { l.ink } else { l.ink3 },
        border: Border {
            width: 1.0,
            radius: crate::panels::graphite::PROPERTY_FIELD_RADIUS.into(),
            color: if on { l.accent } else { l.line },
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    }
}

/// Label of a filter tab or chip: the Graphite ink pair resolved from the theme.
fn chip_label<'a>(label: Label<'a>, on: bool) -> iced::widget::Text<'a> {
    text(label)
        .size(TS.body)
        .style(move |theme: &Theme| iced::widget::text::Style {
            color: {
                let l = form_look::FormLook::of(theme);
                Some(if on { l.ink } else { l.ink3 })
            },
            ..Default::default()
        })
}

/// Tab button in the Custom Selection Filters tab strip. Active tab
/// gets a filled background; both states use the theme accent for the
/// border so the section reads as one piece with the chips and the
/// Active Bar dropdown.
pub fn custom_filter_tab(
    label: Label<'static>,
    active: bool,
    idx: usize,
) -> Element<'static, PanelMsg> {
    signex_widgets::button(chip_label(label, active))
        .padding([3, 10])
        .on_press(PanelMsg::SelectCustomFilterTab(idx))
        .style(move |theme: &Theme, status: iced::widget::button::Status| {
            form_look::tab_chip(
                theme,
                active,
                matches!(status, iced::widget::button::Status::Hovered),
            )
        })
        .boxed()
}

/// Member chip for a custom-filter preset card (Properties panel).
/// Border colour matches the Active Bar Filter dropdown chips (theme
/// accent), so chip styling stays consistent across both surfaces.
pub fn preset_chip(
    label: Label<'static>,
    preset_idx: usize,
    filter: crate::active_bar::SelectionFilter,
    enabled: bool,
) -> Element<'static, PanelMsg> {
    signex_widgets::button(chip_label(label, enabled).align_x(iced::alignment::Horizontal::Center))
        .padding([3.0, SP.m])
        .on_press(PanelMsg::ToggleCustomFilterPresetMember(preset_idx, filter))
        .style(move |theme: &Theme, status: iced::widget::button::Status| {
            graphite_filter_chip(
                theme,
                enabled,
                matches!(status, iced::widget::button::Status::Hovered),
            )
        })
        .boxed()
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
pub fn seg_btn<'a>(
    label: &str,
    active: bool,
    msg: PanelMsg,
    active_bg: Color,
    text_active: Color,
    text_inactive: Color,
) -> Element<'a, PanelMsg> {
    let text_c = seg_text(active, text_active, text_inactive, active_bg);
    signex_widgets::button(
        text(raw(label.to_string()))
            .size(TS.body)
            .color(text_c)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([SP.s, 0.0])
    .width(Length::Fill)
    .on_press(msg)
    .style(move |theme: &Theme, status: iced::widget::button::Status| {
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        form_look::segment_cell(theme, active, hovered && !active)
    })
    .boxed()
}
