//! Properties panel views — the General tab of Document Options
//! (`view_properties_general`: General, Page Options and Net Classes; the
//! selection filter presets live in the Filter panel, mockup row 40) — plus
//! the filter-chip / segmented-button chrome the Properties views use. The sheet's parameters
//! live in the Parameters tab (`parameters_tab`), Page Options in
//! `page_options`.

use super::super::*;
use super::doc_rows::{enable_box, label_row, segmented_row, unless_off};
use super::document_options::{hover_row, illustration_view};
use super::form_rows::canvas_font_summary;
use super::grid_illustration::{GridFacts, HOTSPOTS, SNAP_GRID, VISIBLE_GRID, grid_illustration};
use crate::choice::Choice;
use crate::panels::doc_grid::{DocGridField, DocGridMsg, pitch_text, pitch_unit, typed_pitch_mm};
use crate::panels::element_properties::rows::unit_suffix;
use crate::panels::form_look;
use crate::panels::typed_pick::TypedPick;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::SheetOrientation;
use signex_widgets::Label;

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

/// The grid pitch dropdown in the panel's unit (mockup row 40): a row of the
/// list sets the pitch at once, typed text on Enter. Text the snap limits
/// refuse stays in the box, marked.
fn grid_pick<'a>(
    ctx: &'a PanelContext,
    field: DocGridField,
    current_mm: f32,
) -> Element<'a, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZE_LABELS_MIL, GRID_SIZES_MM};
    let unit = pitch_unit(ctx.unit);
    let labels: &'static [&'static str] = if unit == Unit::Mil {
        GRID_SIZE_LABELS_MIL
    } else {
        GRID_SIZE_LABELS
    };
    let options: Vec<(&'static str, f32)> = labels
        .iter()
        .copied()
        .zip(GRID_SIZES_MM.iter().copied())
        .collect();
    let draft = match field {
        DocGridField::Visible => ctx.visible_grid_draft.as_ref(),
        DocGridField::Snap => ctx.snap_grid_draft.as_ref(),
    };
    let is_refused = draft.is_some_and(|text| typed_pitch_mm(text, unit).is_err());
    TypedPick {
        placeholder: UserText::new(String::new()).into(),
        shown: draft
            .cloned()
            .unwrap_or_else(|| pitch_text(current_mm, unit)),
        error: is_refused.then(|| crate::styles::ti(ctx.tokens.error)),
        unit: Some(unit_suffix(&ctx.i18n, unit).into()),
    }
    .view(
        options,
        // The pitches with their unit symbol (mm, mil), not language.
        |(label, _): &(&'static str, f32)| (*label).to_string(),
        move |text| PanelMsg::DocGrid(DocGridMsg::DraftChanged(field, text)),
        PanelMsg::DocGrid(DocGridMsg::DraftSubmitted(field)),
        move |(_, mm)| field.set_message(mm),
    )
}

/// Visible Grid / Snap Grid: the enable box first, in the label column; the
/// pitch greys and takes no input while the box is off.
fn grid_row<'a>(
    ctx: &'a PanelContext,
    label: signex_i18n::Localized,
    is_on: bool,
    on_toggle: PanelMsg,
    current_mm: f32,
    field: DocGridField,
) -> Element<'a, PanelMsg> {
    label_row(
        enable_box(label, is_on, on_toggle),
        unless_off(is_on, &ctx.tokens, grid_pick(ctx, field, current_mm)),
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
        DocGridField::Visible,
    );
    c = c.push(hover_row(VISIBLE_GRID, hover, &ctx.tokens, visible));
    let snap = grid_row(
        ctx,
        t!(i18n, ids::PROPERTIES_SNAP_GRID),
        ctx.snap_enabled,
        PanelMsg::ToggleSnap,
        ctx.grid_size_mm,
        DocGridField::Snap,
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
        canvas_font_summary(i18n, &ctx.canvas_font_name, ctx.canvas_font_size),
        ctx.theme_id,
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
