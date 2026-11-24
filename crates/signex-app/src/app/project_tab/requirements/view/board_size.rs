//! The Board size card's extras (mockup row 38): Measured from, which names
//! what the check measures (the bounding box of the board outline), and a
//! read-only preview of the board against the limit, drawn from the stored
//! limits and the size the last run measured. The limits are typed in their
//! fields only: the preview has no drag handles (decision 2026-11-24, #2815).

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Element, Length, Point as P, Rectangle as R, Size, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, IllustrationHover, Ink, Paint, Part, Primitive, Style,
    TextLabel, TextRole, Tint, Weight, property_illustration,
};
use signex_widgets::{text, text_input};

use super::super::super::look::ui_font;
use super::super::super::table::{SP, TS};
use crate::app::Message;
use crate::styles::board_palette::{BoardPalette, BoardSwatch};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// The drawing's view box, in layout units (the mockup's 640 x 200).
const VIEW_W: f32 = 640.0;
const VIEW_H: f32 = 200.0;
/// Left edge of the board and the limit, past the height callout.
const X0: f32 = 60.0;
/// Bottom edge of the board and the limit, above the width callout.
const BOTTOM: f32 = VIEW_H - 26.0;
/// Room the scale leaves above the shapes and below them for the callout.
const ROOM_Y: f32 = 50.0;
/// Room the scale leaves for the height callout and the legend.
const ROOM_X: f32 = 260.0;
/// Left edge of the legend column.
const LEGEND_X: f32 = VIEW_W - 186.0;
/// Length of a legend line sample.
const SAMPLE: f32 = 22.0;
/// Corner radius of the drawn board.
const BOARD_RADIUS: f32 = 4.0;
/// Below a nanometre two lengths are the same, as the check judges them.
const SAME_MM: f64 = 1e-6;

/// What a Board size requirement asks, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app::project_tab::requirements) struct Limit {
    pub width_mm: f64,
    pub height_mm: f64,
}

/// `value` mm as the preview writes it: up to three decimals, no trailing
/// zeros.
pub(in crate::app::project_tab::requirements) fn mm_text(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    // A margin rounded to nothing from below reads 0, not -0.
    if text == "-0" { "0" } else { text }.to_string()
}

/// A positive, finite width and height, or `None`.
fn drawable((width, height): (f64, f64)) -> Option<(f64, f64)> {
    let ok = |v: f64| v.is_finite() && v > 0.0;
    (ok(width) && ok(height)).then_some((width, height))
}

/// Whether a board `measured` mm wide and high fits `limit`.
pub(in crate::app::project_tab::requirements) fn fits(limit: Limit, measured: (f64, f64)) -> bool {
    measured.0 <= limit.width_mm + SAME_MM && measured.1 <= limit.height_mm + SAME_MM
}

/// The header caption after the limits: what the last run measured.
pub(super) fn measured_words(measured: (f64, f64), i18n: &I18n) -> Localized {
    t!(
        i18n,
        ids::REQUIREMENTS_SUMMARY_MEASURED,
        "width" => mm_text(measured.0),
        "height" => mm_text(measured.1),
    )
}

/// The Measured from cell: the caption over a read-only box naming the
/// board outline, the one thing the check measures.
pub(super) fn measured_from<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let caption = text(t!(i18n, ids::REQUIREMENTS_FIELD_MEASURED_FROM))
        .size(TS.caption)
        .color(resolve(tokens).ink3);
    let source = t!(i18n, ids::REQUIREMENTS_MEASURED_FROM_OUTLINE).into_string();
    let shown = text_input(t!(i18n, ids::REQUIREMENTS_FIELD_MEASURED_FROM), source)
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fill);
    column![caption.boxed(), shown.boxed()]
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
}

/// The drawing of a board `measured` mm (none before a run, or with no
/// outline) against `limit`; `None` when the limit is no positive length.
pub(in crate::app::project_tab::requirements) fn illustration(
    limit: Limit,
    measured: Option<(f64, f64)>,
    i18n: &I18n,
) -> Option<Illustration> {
    let (mw, mh) = drawable((limit.width_mm, limit.height_mm))?;
    let measured = measured.and_then(drawable);
    let (bw, bh) = measured.unwrap_or((0.0, 0.0));
    let scale = ((f64::from(VIEW_H - ROOM_Y) / mh.max(bh))
        .min(f64::from(VIEW_W - ROOM_X) / mw.max(bw))) as f32;
    let (mw_px, mh_px) = (mw as f32 * scale, mh as f32 * scale);
    let palette = BoardPalette::new();
    let edge = Paint::Document(palette.swatch(BoardSwatch::BoardEdge));
    let label = |at: P, words: Localized, anchor: Anchor, role: TextRole, ink: Ink| {
        Part::label(
            TextLabel {
                at,
                text: words.into(),
                anchor,
                role,
            },
            ink,
        )
    };
    let mut drawing = Illustration::new(VIEW_W, VIEW_H);
    let over = measured.is_some_and(|m| !fits(limit, m));
    let verdict_ink = if over { Ink::Error } else { Ink::Secondary };
    if let Some((bw, bh)) = measured {
        let (bw_px, bh_px) = (bw as f32 * scale, bh as f32 * scale);
        let stroke = if over { Paint::Ink(Ink::Error) } else { edge };
        let board = Part::new(
            Primitive::RoundedRect {
                rect: R::new(P::new(X0, BOTTOM - bh_px), Size::new(bw_px, bh_px)),
                radius: BOARD_RADIUS,
            },
            Style::outline(Ink::Secondary, Weight::Heavy)
                .with_stroke_paint(stroke)
                .with_fill(Paint::Document(palette.board_area), Tint::Solid),
        );
        let words = t!(
            i18n,
            ids::REQUIREMENTS_BOARD_MEASURED,
            "width" => mm_text(bw),
            "height" => mm_text(bh),
        );
        let centre = P::new(X0 + bw_px / 2.0, BOTTOM - bh_px / 2.0 + 3.0);
        drawing = drawing.part(board).part(label(
            centre,
            words,
            Anchor::Middle,
            TextRole::Caption,
            verdict_ink,
        ));
    }
    let limit_rect = Part::new(
        Primitive::Rect {
            rect: R::new(P::new(X0, BOTTOM - mh_px), Size::new(mw_px, mh_px)),
        },
        Style::dashed(),
    );
    let width = t!(i18n, ids::REQUIREMENTS_BOARD_MAX_WIDTH, "width" => mm_text(mw));
    let height = t!(i18n, ids::REQUIREMENTS_BOARD_MAX_HEIGHT, "height" => mm_text(mh));
    let below = BOTTOM + 12.0;
    let beside = X0 - 14.0;
    drawing = drawing
        .part(limit_rect)
        .part(Part::callout(
            Callout::new(P::new(X0, below), P::new(X0 + mw_px, below))
                .label(width)
                .label_at(P::new(X0 + mw_px / 2.0, below + 11.0), Anchor::Middle),
        ))
        .part(Part::callout(
            Callout::new(P::new(beside, BOTTOM - mh_px), P::new(beside, BOTTOM))
                .label(height)
                .label_at(
                    P::new(beside - 4.0, BOTTOM - mh_px / 2.0 + 3.0),
                    Anchor::End,
                ),
        ));
    drawing = drawing
        .part(Part::new(
            Primitive::Line {
                from: P::new(LEGEND_X, 30.0),
                to: P::new(LEGEND_X + SAMPLE, 30.0),
            },
            Style::dashed(),
        ))
        .part(label(
            P::new(LEGEND_X + SAMPLE + 6.0, 33.0),
            t!(i18n, ids::REQUIREMENTS_BOARD_LEGEND_LIMIT),
            Anchor::Start,
            TextRole::Note,
            Ink::Secondary,
        ));
    let Some((bw, bh)) = measured else {
        return Some(drawing.part(label(
            P::new(LEGEND_X, 58.0),
            t!(i18n, ids::REQUIREMENTS_BOARD_NOT_MEASURED),
            Anchor::Start,
            TextRole::Note,
            Ink::Tertiary,
        )));
    };
    let room = |limit: f64, size: f64| mm_text(((limit - size) * 100.0).round() / 100.0);
    let margin = t!(
        i18n,
        ids::REQUIREMENTS_BOARD_MARGIN,
        "width" => room(mw, bw),
        "height" => room(mh, bh),
    );
    let verdict = if over {
        ids::REQUIREMENTS_BOARD_DOES_NOT_FIT
    } else {
        ids::REQUIREMENTS_BOARD_FITS
    };
    let drawing = drawing
        .part(Part::new(
            Primitive::Line {
                from: P::new(LEGEND_X, 48.0),
                to: P::new(LEGEND_X + SAMPLE, 48.0),
            },
            Style::outline(Ink::Secondary, Weight::Heavy).with_stroke_paint(edge),
        ))
        .part(label(
            P::new(LEGEND_X + SAMPLE + 6.0, 51.0),
            t!(i18n, ids::REQUIREMENTS_BOARD_LEGEND_OUTLINE),
            Anchor::Start,
            TextRole::Note,
            Ink::Secondary,
        ))
        .part(label(
            P::new(LEGEND_X, 76.0),
            margin,
            Anchor::Start,
            TextRole::Caption,
            verdict_ink,
        ))
        .part(label(
            P::new(LEGEND_X, 92.0),
            t!(i18n, verdict),
            Anchor::Start,
            TextRole::Note,
            verdict_ink,
        ));
    Some(drawing)
}

/// The preview under the fields: its title and hint over the drawing, on
/// the dark board ground that board views keep under the light chrome too.
/// `None` when the limit cannot be drawn.
pub(super) fn preview<'a>(
    limit: Limit,
    measured: Option<(f64, f64)>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    let drawing = illustration(limit, measured, i18n)?;
    let r = resolve(tokens);
    let title = row![
        text(t!(i18n, ids::REQUIREMENTS_BOARD_PREVIEW))
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        text(t!(i18n, ids::REQUIREMENTS_BOARD_PREVIEW_HINT))
            .size(TS.caption)
            .color(r.ink4),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let ground = BoardPalette::new().canvas;
    let dark = theme_tokens(ThemeId::GraphiteDark);
    let art = property_illustration(drawing, &IllustrationHover::default(), &dark, TS.caption)
        .map(|_| Message::noop());
    let framed = container(art)
        .padding(SP.s)
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(ground)),
            border: iced::Border {
                radius: SP.xs.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    let col: Column<Element<'a, Message>> = column![title.boxed(), framed.boxed()]
        .spacing(SP.xs)
        .width(Length::Fill);
    Some(col.boxed())
}
