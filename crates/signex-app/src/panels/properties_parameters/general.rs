//! Properties panel views — the General tab of Document Options
//! (`view_properties_general`: General, Page Options, Net Classes and the
//! Custom Selection Filters section) — plus the filter-tab / preset-chip /
//! segmented-button chrome the Properties views use. The sheet's parameters
//! live in the Parameters tab (`parameters_tab`), Page Options in
//! `page_options`.

use super::super::*;
use super::doc_rows::{enable_box, label_row, segmented_row, unless_off};
use super::document_options::{hover_row, illustration_view};
use super::grid_illustration::{GridFacts, HOTSPOTS, SNAP_GRID, VISIBLE_GRID, grid_illustration};
use crate::choice::Choice;
use crate::panels::form_look;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::column;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::SheetOrientation;
use signex_widgets::Label;
use signex_widgets::dropdown_style::GraphiteDropdown as _;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The catalog name of a page orientation.
pub(super) fn orientation_label(orientation: SheetOrientation) -> signex_i18n::MessageId {
    match orientation {
        SheetOrientation::Landscape => ids::PROPERTIES_ORIENTATION_LANDSCAPE,
        SheetOrientation::Portrait => ids::PROPERTIES_ORIENTATION_PORTRAIT,
    }
}

/// The catalog name of a page origin.
pub(super) fn origin_label(origin: PageOrigin) -> signex_i18n::MessageId {
    match origin {
        PageOrigin::UpperLeft => ids::PROPERTIES_ORIGIN_UPPER_LEFT,
        PageOrigin::LowerLeft => ids::PROPERTIES_ORIGIN_LOWER_LEFT,
    }
}

/// A Sheet Color pick-list choice, named from the catalog; a colour that is
/// no preset shows its hex code.
fn sheet_color_choice(
    i18n: &I18n,
    color: crate::panels::SchematicSheetColor,
) -> Choice<crate::panels::SchematicSheetColor> {
    use crate::panels::SchematicSheetColor as S;
    use crate::panels::SheetColor as P;
    let id = match color {
        S::Default => ids::PROPERTIES_SHEET_COLOR_DEFAULT,
        S::Preset(P::Black) => ids::PROPERTIES_SHEET_COLOR_BLACK,
        S::Preset(P::White) => ids::PROPERTIES_SHEET_COLOR_WHITE,
        S::Preset(P::DarkGray) => ids::PROPERTIES_SHEET_COLOR_DARK_GRAY,
        S::Preset(P::LightGray) => ids::PROPERTIES_SHEET_COLOR_LIGHT_GRAY,
        S::Preset(P::Cream) => ids::PROPERTIES_SHEET_COLOR_CREAM,
        S::Custom(custom) => {
            return Choice {
                value: color,
                label: i18n.message_plain(
                    ids::PROPERTIES_SHEET_COLOR_CUSTOM,
                    &[("hex", custom.to_hex().into())],
                ),
            };
        }
    };
    Choice {
        value: color,
        label: i18n.message_plain(id, &[]),
    }
}

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
            ids::PROPERTIES_SECTION_SELECTION_FILTERS,
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

/// The grid pitch pick list in `unit`; picking reports the pitch in mm.
fn grid_pick<'a>(
    current_mm: f32,
    unit: Unit,
    on_size: impl Fn(f32) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZE_LABELS_MIL, GRID_SIZES_MM};
    let labels: &'static [&'static str] = if unit == Unit::Mil {
        GRID_SIZE_LABELS_MIL
    } else {
        GRID_SIZE_LABELS
    };
    let selected: Option<&'static str> = GRID_SIZES_MM
        .iter()
        .zip(labels.iter())
        .find(|(size, _)| (**size - current_mm).abs() < 1e-4)
        .map(|(_, label)| *label);
    // The pitches with their unit symbol (mm, mil), not language.
    iced::widget::pick_list(selected, labels, |l: &&'static str| (*l).to_string())
        .graphite()
        .on_select(move |picked: &'static str| {
            let mm = GRID_SIZE_LABELS
                .iter()
                .chain(GRID_SIZE_LABELS_MIL.iter())
                .zip(GRID_SIZES_MM.iter().chain(GRID_SIZES_MM.iter()))
                .find(|(l, _)| **l == picked)
                .map_or(2.54, |(_, v)| *v);
            on_size(mm)
        })
        .padding([SP.xs, 6.0])
        .width(Length::Fill)
        .boxed()
}

/// Visible Grid / Snap Grid: the enable box first, in the label column; the
/// pitch greys and takes no input while the box is off.
fn grid_row<'a>(
    ctx: &'a PanelContext,
    label: signex_i18n::Localized,
    is_on: bool,
    on_toggle: PanelMsg,
    current_mm: f32,
    on_size: impl Fn(f32) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    label_row(
        enable_box(label, is_on, on_toggle),
        unless_off(is_on, &ctx.tokens, grid_pick(current_mm, ctx.unit, on_size)),
    )
}

/// The General section: the Grid and snap illustration, then one row per
/// setting.
fn general_section<'a>(
    ctx: &'a PanelContext,
    palette: PanelPalette,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a I18n = &ctx.i18n;
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let hover = &ctx.document_options.hover;
    let facts = GridFacts {
        unit: ctx.unit,
        visible_grid_mm: ctx.visible_grid_mm,
        snap_grid_mm: ctx.grid_size_mm,
        grid_visible: ctx.grid_visible,
        snap_enabled: ctx.snap_enabled,
        snap_hotspots: ctx.snap_hotspots,
    };
    let mut c: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    c = c.push(illustration_view(
        grid_illustration(&facts, i18n),
        hover,
        &ctx.tokens,
    ));
    c = c.push(segmented_row(
        t!(i18n, ids::PANELS_PROPERTIES_UNITS),
        vec![
            (Unit::Mm, t!(i18n, ids::PROPERTIES_UNIT_MM).into()),
            (Unit::Mil, t!(i18n, ids::PROPERTIES_UNIT_MILS).into()),
        ],
        if ctx.unit == Unit::Mil {
            Unit::Mil
        } else {
            Unit::Mm
        },
        &ctx.tokens,
        PanelMsg::SetUnit,
    ));
    let visible = grid_row(
        ctx,
        t!(i18n, ids::PROPERTIES_VISIBLE_GRID),
        ctx.grid_visible,
        PanelMsg::ToggleGrid,
        ctx.visible_grid_mm,
        PanelMsg::SetVisibleGridSize,
    );
    c = c.push(hover_row(VISIBLE_GRID, hover, &ctx.tokens, visible));
    let snap = grid_row(
        ctx,
        t!(i18n, ids::PROPERTIES_SNAP_GRID),
        ctx.snap_enabled,
        PanelMsg::ToggleSnap,
        ctx.grid_size_mm,
        PanelMsg::SetGridSize,
    );
    c = c.push(hover_row(SNAP_GRID, hover, &ctx.tokens, snap));
    let hotspots = form_check_row_shortcut(
        t!(i18n, ids::PROPERTIES_SNAP_TO_HOTSPOTS),
        ctx.snap_hotspots,
        PanelMsg::ToggleSnapHotspots,
        "Shift+E",
        muted,
    );
    c = c.push(hover_row(HOTSPOTS, hover, &ctx.tokens, hotspots));
    c = c.push(form_font_link_row(
        t!(i18n, ids::PROPERTIES_CANVAS_FONT),
        &ctx.canvas_font_name,
        ctx.canvas_font_size,
        ctx.canvas_font_bold,
        ctx.canvas_font_italic,
        muted,
    ));
    if ctx.canvas_font_popup_open {
        c = c.push(
            container(canvas_font_popup(
                &ctx.canvas_font_name,
                ctx.canvas_font_size,
                ctx.canvas_font_bold,
                ctx.canvas_font_italic,
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
    let sheet_colors = crate::panels::SchematicSheetColor::options()
        .into_iter()
        .map(|color| sheet_color_choice(i18n, color))
        .collect();
    c.push(form_choice_row(
        t!(i18n, ids::PROPERTIES_SHEET_COLOR),
        sheet_colors,
        sheet_color_choice(i18n, ctx.sheet_color),
        PanelMsg::SetSheetColor,
        muted,
    ))
}

pub fn view_properties_general<'a>(
    ctx: &'a PanelContext,
    _primary: Color,
) -> Column<Element<'a, PanelMsg>> {
    let hairline = crate::styles::tokens::resolve(&ctx.tokens).line_soft;
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        ctx.collapsible(
            "prop_general",
            ids::PROPERTIES_SECTION_GENERAL,
            &ctx.collapsed_sections,
            move || general_section(ctx, palette),
        )
        .boxed(),
    );

    col = col.push(thin_sep(hairline));
    col = col.push(
        ctx.collapsible(
            "prop_page_opts",
            ids::PROPERTIES_SECTION_PAGE_OPTIONS,
            &ctx.collapsed_sections,
            move || super::page_options::page_options_section(ctx),
        )
        .boxed(),
    );

    col = col.push(thin_sep(hairline));
    col = col.push(view_net_classes_section(ctx).boxed());

    // Custom Selection Filters sit last: Document Options opens on General
    // (#2262); the presets stay here and in the active bar's Filter menu.
    col = col.push(thin_sep(hairline));
    col = col.push(selection_filters_section_for(ctx, palette).boxed());

    col
}

/// Graphite filter chip: accent-soft fill when on, hairline outline when
/// off, radius 7.
pub(crate) fn graphite_filter_chip(
    theme: &Theme,
    on: bool,
    hovered: bool,
) -> iced::widget::button::Style {
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
pub(crate) fn chip_label<'a>(label: Label<'a>, on: bool) -> iced::widget::Text<'a> {
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

/// Segmented button (for units toggle etc), a cell of a
/// [`signex_widgets::FitRow`], which sizes it.
pub fn seg_btn<'a>(
    label: impl Into<Label<'a>>,
    active: bool,
    msg: PanelMsg,
    active_bg: Color,
    text_active: Color,
    text_inactive: Color,
) -> Element<'a, PanelMsg> {
    let text_c = seg_text(active, text_active, text_inactive, active_bg);
    signex_widgets::button(
        text(label)
            .size(TS.body)
            .color(text_c)
            .align_x(iced::alignment::Horizontal::Center)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
    )
    .padding([SP.s, SP.xs])
    .on_press(msg)
    .style(move |theme: &Theme, status: iced::widget::button::Status| {
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        form_look::segment_cell(theme, active, hovered && !active)
    })
    .boxed()
}

#[cfg(test)]
#[path = "general_tests.rs"]
mod tests;
