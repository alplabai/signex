//! Document Options ▸ General ▸ Page Options (#2531): the Sheet
//! illustration, Format, the paper (a named size, or typed W / H for a
//! Custom format), Orientation, Origin, Show Border with its zone counts,
//! Show Title Block and the sheet's default font.

use super::super::*;
use super::doc_rows::{axis_field, enable_box, label_row, pair, row_label, segmented_row};
use super::document_options::{DocumentOptionsMsg, ZoneAxis, hover_row, illustration_view, msg};
use super::general::{orientation_label, origin_label};
use super::sheet_illustration::{
    BORDER, ORIENTATION, ORIGIN, PAPER, PAPER_HEIGHT, PAPER_WIDTH, SheetFacts, TITLE_BLOCK, ZONES,
    mm_text, sheet_illustration,
};
use crate::choice::{Choice, chosen};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{I18n, ids, t};
use signex_types::schematic::{SheetOrientation, SheetSize};
use signex_widgets::enable_section::EnableSection;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The paper sizes offered, each named with its size at `orientation`
/// ("A4 · 297 × 210 mm"); the value is the picker token.
pub(crate) fn paper_choices(i18n: &I18n, orientation: SheetOrientation) -> Vec<Choice<String>> {
    SheetSize::PICKABLE
        .iter()
        .map(|size| {
            let token = size.token(SheetOrientation::Landscape);
            let (w, h) = size.dimensions_mm(orientation);
            let label = i18n.message_plain(
                ids::PROPERTIES_PAPER_CHOICE,
                &[
                    ("name", token.clone().into()),
                    ("width", mm_text(w as f32).into()),
                    ("height", mm_text(h as f32).into()),
                ],
            );
            Choice {
                value: token,
                label,
            }
        })
        .collect()
}

/// The paper row: the named size, its size shown in the list.
fn paper_row<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let (size, orientation) = SheetSize::parse(&ctx.paper_size);
    let choices = paper_choices(i18n, orientation);
    let current = size.token(SheetOrientation::Landscape);
    let selected = chosen(&choices, &current).unwrap_or_else(|| Choice {
        label: current.clone(),
        value: current,
    });
    form_choice_row(
        t!(i18n, ids::PROPERTIES_PAPER),
        choices,
        selected,
        PanelMsg::SetPaperSize,
        muted,
    )
}

/// Format = Custom: the typed W / H of the paper, committed on Enter.
fn custom_size_row<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let hover = &ctx.document_options.hover;
    let (w, h) = paper_dimensions(&ctx.paper_size);
    let shown = |edge: PaperEdge, value: f32| {
        ctx.title_block_sheet
            .as_deref()
            .and_then(|sheet| ctx.drafts.paper_edge_text(sheet, edge))
            .unwrap_or_else(|| format!("{value:.1}"))
    };
    let field = |edge: PaperEdge, axis, value: f32| {
        axis_field(
            t!(i18n, axis),
            shown(edge, value),
            Some(t!(i18n, ids::PROPERTIES_UNIT_MM).into()),
            &ctx.tokens,
            move |s| PanelMsg::CustomPaperEdgeTyping(edge, s),
            PanelMsg::CommitCustomPaperEdge(edge),
        )
    };
    let width = hover_row(
        PAPER_WIDTH,
        hover,
        &ctx.tokens,
        field(PaperEdge::Width, ids::PROPERTIES_PAPER_W_SHORT, w),
    );
    let height = hover_row(
        PAPER_HEIGHT,
        hover,
        &ctx.tokens,
        field(PaperEdge::Height, ids::PROPERTIES_PAPER_H_SHORT, h),
    );
    label_row(
        row_label(t!(i18n, ids::PROPERTIES_SIZE), &ctx.tokens),
        pair(width, height),
    )
}

/// Zones V / H as typed numbers on one row.
fn zones_row<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let ui = &ctx.document_options;
    let sheet = ctx.title_block_sheet.as_deref();
    let frame = ctx.sheet_frame;
    let field = |axis: ZoneAxis, short, stored: u8| {
        axis_field(
            t!(i18n, short),
            ui.zone_text(sheet, axis, stored),
            None,
            &ctx.tokens,
            move |s| msg(DocumentOptionsMsg::ZoneCountTyped(axis, s)),
            msg(DocumentOptionsMsg::ZoneCountSubmitted(axis)),
        )
    };
    let fields = pair(
        field(
            ZoneAxis::Vertical,
            ids::PROPERTIES_ZONES_V_SHORT,
            frame.vertical_zones,
        ),
        field(
            ZoneAxis::Horizontal,
            ids::PROPERTIES_ZONES_H_SHORT,
            frame.horizontal_zones,
        ),
    );
    let zones = label_row(
        row_label(t!(i18n, ids::PROPERTIES_ZONES), &ctx.tokens),
        fields,
    );
    hover_row(ZONES, &ui.hover, &ctx.tokens, zones)
}

/// A check row with its box first: Show Title Block.
fn check_row<'a>(content: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    container(content)
        .padding([SP.xs, PROPERTY_ROW_PAD_X])
        .width(Length::Fill)
        .boxed()
}

/// The Page Options section body.
pub(super) fn page_options_section<'a>(ctx: &'a PanelContext) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a I18n = &ctx.i18n;
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let hover = &ctx.document_options.hover;
    let mode = ctx.page_format_mode;
    let is_custom = mode == PageFormatMode::Custom;
    let (width_mm, height_mm) = paper_dimensions(&ctx.paper_size);
    let frame = ctx.sheet_frame;
    let facts = SheetFacts {
        width_mm,
        height_mm,
        show_border: frame.show_border,
        show_title_block: frame.show_title_block,
        vertical_zones: frame.vertical_zones,
        horizontal_zones: frame.horizontal_zones,
        origin: ctx.page_origin,
        is_custom,
    };
    let mut c: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    c = c.push(illustration_view(
        sheet_illustration(&facts, i18n),
        hover,
        &ctx.tokens,
    ));
    c = c.push(segmented_row(
        t!(i18n, ids::PROPERTIES_FORMAT),
        vec![
            (
                PageFormatMode::Template,
                t!(i18n, ids::PROPERTIES_FORMAT_TEMPLATE).into(),
            ),
            (
                PageFormatMode::Standard,
                t!(i18n, ids::PROPERTIES_FORMAT_STANDARD).into(),
            ),
            (
                PageFormatMode::Custom,
                t!(i18n, ids::PROPERTIES_FORMAT_CUSTOM).into(),
            ),
        ],
        mode,
        &ctx.tokens,
        PanelMsg::SetPageFormatMode,
    ));
    // Standard and Template pick a named size; Custom swaps the size and
    // orientation rows for typed W / H. Both write the sheet's own size
    // through the engine, so the canvas, the PDF and the file agree.
    if is_custom {
        c = c.push(custom_size_row(ctx));
    } else {
        c = c.push(hover_row(PAPER, hover, &ctx.tokens, paper_row(ctx)));
        if mode == PageFormatMode::Template {
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
        let (size, orientation) = SheetSize::parse(&ctx.paper_size);
        if size.is_recognized() {
            let options = [SheetOrientation::Landscape, SheetOrientation::Portrait]
                .into_iter()
                .map(|o| (o, t!(i18n, orientation_label(o)).into()))
                .collect();
            let row = segmented_row(
                t!(i18n, ids::PROPERTIES_ORIENTATION),
                options,
                orientation,
                &ctx.tokens,
                PanelMsg::SetSheetOrientation,
            );
            c = c.push(hover_row(ORIENTATION, hover, &ctx.tokens, row));
        }
    }
    let origins = [PageOrigin::UpperLeft, PageOrigin::LowerLeft]
        .into_iter()
        .map(|o| (o, t!(i18n, origin_label(o)).into()))
        .collect();
    let origin = segmented_row(
        t!(i18n, ids::PROPERTIES_ORIGIN),
        origins,
        ctx.page_origin,
        &ctx.tokens,
        PanelMsg::SetPageOrigin,
    );
    c = c.push(hover_row(ORIGIN, hover, &ctx.tokens, origin));
    let border = EnableSection::new(t!(i18n, ids::PROPERTIES_SHOW_BORDER), frame.show_border)
        .on_toggle(|on| PanelMsg::EditSheetFrame(SheetFrameEdit::ShowBorder(on)))
        .view(
            crate::panels::enable_section::enable_section_style(&ctx.tokens),
            zones_row(ctx),
        );
    c = c.push(hover_row(BORDER, hover, &ctx.tokens, border));
    let title_block = check_row(enable_box(
        t!(i18n, ids::PROPERTIES_SHOW_TITLE_BLOCK),
        frame.show_title_block,
        PanelMsg::EditSheetFrame(SheetFrameEdit::ShowTitleBlock(!frame.show_title_block)),
    ));
    c = c.push(hover_row(TITLE_BLOCK, hover, &ctx.tokens, title_block));
    c.push(crate::panels::element_properties::font_family_row(
        t!(i18n, ids::PROPERTIES_DEFAULT_FONT),
        ctx.document_font_family.as_deref(),
        muted,
        PanelMsg::SetDocumentFontFamily,
        i18n,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The paper list names each size with its millimetres at the sheet's
    /// orientation, and picks by token.
    #[test]
    fn each_paper_choice_shows_its_size() {
        let en = I18n::en_us();
        let landscape = paper_choices(&en, SheetOrientation::Landscape);
        let a4 = chosen(&landscape, &"A4".to_string()).expect("A4 is offered");
        assert_eq!(
            a4.label.replace(['\u{2068}', '\u{2069}'], ""),
            "A4 · 297 × 210 mm"
        );
        let portrait = paper_choices(&en, SheetOrientation::Portrait);
        let a4 = chosen(&portrait, &"A4".to_string()).expect("A4 is offered");
        assert_eq!(
            a4.label.replace(['\u{2068}', '\u{2069}'], ""),
            "A4 · 210 × 297 mm"
        );
    }
}
