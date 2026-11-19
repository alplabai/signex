//! Grid preview of Appearance ▸ Canvas (#144): the schematic canvas at
//! 1.27 mm beside the symbol canvas at the symbol editor's default grid,
//! each in its drafted grid style, in the theme's canvas colours. It sets
//! nothing: the fast grid pitches are edited on the Editing page's drawing.

use super::grid_style_label;
use crate::canvas::grid::pitch_mm_exact;
use crate::preferences::pages_view::controls::{PreviewBlock, PreviewColours, preview_block};
use crate::preferences::pages_view::{PREVIEW_PX_PER_MM, PreviewKind};
use crate::preferences::{PrefMsg, PrefsView};
use crate::render_config::GridStyle;
use crate::styles;
use iced::widget::canvas;
use iced::{
    Color, Element, Font, Length, Pixels, Point, Rectangle, Size, Theme, Widget as _, mouse,
};
use signex_i18n::{Arg, I18n, Localized, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_widgets::LengthFormat;

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;

/// The schematic half is drawn at the default schematic grid.
const SCHEMATIC_GRID_MM: f32 = 1.27;
/// Mockup geometry, in drawing px.
const CAPTION_AT: (f32, f32) = (12.0, 8.0);
const MARKS_TOP: f32 = 30.0;
const MARKS_MARGIN: f32 = 10.0;
const SYMBOL_INSET: f32 = 12.0;
const RESISTOR_X: f32 = 140.0;
const BODY_LINE: f32 = 1.4;
const WIRE_LINE: f32 = 1.6;
/// Marks closer than this are not drawn at all.
const MIN_MARK_STEP: f32 = 4.0;
const DOT_R: f32 = 1.2;
const DOT_ALPHA: f32 = 0.55;
const CROSS_ARM: f32 = 2.5;
const CROSS_LINE: f32 = 0.9;
const GRID_LINE: f32 = 0.8;

/// The preview block: label, caption, expand button and the drawing.
pub(super) fn grid_preview<'a>(v: PrefsView<'a>, expanded: bool) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let (schematic, symbol) = (v.draft_grid_style, v.draft_symbol_grid_style);
    let symbol_mm = v.draft_symbol_grid_size_mm;
    let drawing = GridPreview {
        schematic,
        symbol,
        symbol_mm,
        captions: [
            half_caption(
                i18n,
                ids::PREFERENCES_APPEARANCE_GRID_PREVIEW_SCHEMATIC,
                schematic,
                SCHEMATIC_GRID_MM,
            ),
            half_caption(
                i18n,
                ids::PREFERENCES_APPEARANCE_GRID_PREVIEW_SYMBOL,
                symbol,
                symbol_mm,
            ),
        ],
        colours: PreviewColours::new(t),
    };
    preview_block(
        PreviewBlock {
            kind: PreviewKind::Grid,
            expanded,
            title: t!(i18n, ids::PREFERENCES_APPEARANCE_GRID_PREVIEW),
            caption: caption(i18n, schematic, symbol, symbol_mm),
            drawing: canvas::Canvas::new(drawing)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
            footer: None,
        },
        t,
        i18n,
    )
}

/// The caption beside the label: both styles and the symbol grid.
pub(super) fn caption(
    i18n: &I18n,
    schematic: GridStyle,
    symbol: GridStyle,
    symbol_mm: f32,
) -> Localized {
    t!(
        i18n,
        ids::PREFERENCES_APPEARANCE_GRID_PREVIEW_CAPTION,
        "schematic" => style_name(i18n, schematic),
        "symbol" => style_name(i18n, symbol),
        "grid" => millimetres(i18n, symbol_mm)
    )
}

fn half_caption(i18n: &I18n, id: MessageId, style: GridStyle, mm: f32) -> String {
    i18n.message_plain(
        id,
        &[
            ("style", Arg::from(style_name(i18n, style))),
            ("grid", Arg::from(millimetres(i18n, mm))),
        ],
    )
}

fn style_name(i18n: &I18n, style: GridStyle) -> String {
    i18n.message_plain(grid_style_label(style), &[])
}

/// `mm` as "1.27 mm"; lengths keep `.` in every locale.
fn millimetres(i18n: &I18n, mm: f32) -> String {
    let value = LengthFormat::new(i18n, Unit::Mm).number(pitch_mm_exact(mm));
    i18n.message_plain(
        ids::PREFERENCES_PAGES_MILLIMETRES,
        &[("value", Arg::from(value))],
    )
}

struct GridPreview {
    schematic: GridStyle,
    symbol: GridStyle,
    symbol_mm: f32,
    /// Drawn over the schematic and the symbol half.
    captions: [String; 2],
    colours: PreviewColours,
}

impl canvas::Program<PrefMsg> for GridPreview {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let c = &self.colours;
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        let half = w / 2.0;
        frame.fill_rectangle(Point::ORIGIN, Size::new(w, h), c.paper);
        frame.stroke(
            &canvas::Path::line(Point::new(half, 0.0), Point::new(half, h)),
            canvas::Stroke::default()
                .with_color(c.border)
                .with_width(1.0),
        );
        let s1 = SCHEMATIC_GRID_MM * PREVIEW_PX_PER_MM;
        let s2 = self.symbol_mm * PREVIEW_PX_PER_MM;
        let top = MARKS_TOP + s1 / 4.0;
        let left = Rectangle::new(
            Point::new(s1 / 2.0, top),
            Size::new(half - MARKS_MARGIN - s1 / 2.0, h - MARKS_MARGIN - top),
        );
        let right = Rectangle::new(
            Point::new(half + SYMBOL_INSET, MARKS_TOP),
            Size::new(
                half - SYMBOL_INSET - MARKS_MARGIN,
                h - MARKS_MARGIN - MARKS_TOP,
            ),
        );
        grid_marks(&mut frame, left, s1, self.schematic, c);
        grid_marks(&mut frame, right, s2, self.symbol, c);
        draw_resistor(&mut frame, Point::new(RESISTOR_X, top + 3.0 * s1), s1, c);
        draw_symbol(
            &mut frame,
            Point::new(half + SYMBOL_INSET, MARKS_TOP),
            s2,
            c,
        );
        for (x, caption) in [CAPTION_AT.0, half + CAPTION_AT.0]
            .into_iter()
            .zip(&self.captions)
        {
            frame.fill_text(canvas::Text {
                content: caption.clone(),
                position: Point::new(x, CAPTION_AT.1),
                color: c.ink3,
                size: Pixels(TS.caption),
                font: Font::MONOSPACE,
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}

/// Grid marks every `step` px over `area`, in `style`: lines, dots or
/// small crosses, as the canvases draw them.
pub(in crate::preferences) fn grid_marks(
    frame: &mut canvas::Frame,
    area: Rectangle,
    step: f32,
    style: GridStyle,
    c: &PreviewColours,
) {
    if !step.is_finite() || step < MIN_MARK_STEP {
        return;
    }
    let (x1, y1) = (area.x + area.width, area.y + area.height);
    let columns = || {
        std::iter::successors(Some(area.x), move |x| Some(x + step)).take_while(move |x| *x <= x1)
    };
    let rows = || {
        std::iter::successors(Some(area.y), move |y| Some(y + step)).take_while(move |y| *y <= y1)
    };
    match style {
        GridStyle::Lines => {
            let lines = canvas::Path::new(|b| {
                for x in columns() {
                    b.move_to(Point::new(x, area.y));
                    b.line_to(Point::new(x, y1));
                }
                for y in rows() {
                    b.move_to(Point::new(area.x, y));
                    b.line_to(Point::new(x1, y));
                }
            });
            frame.stroke(
                &lines,
                canvas::Stroke::default()
                    .with_color(c.line_strong)
                    .with_width(GRID_LINE),
            );
        }
        GridStyle::Dots => {
            let dots = canvas::Path::new(|b| {
                for x in columns() {
                    for y in rows() {
                        b.circle(Point::new(x, y), DOT_R);
                    }
                }
            });
            frame.fill(
                &dots,
                Color {
                    a: c.ink4.a * DOT_ALPHA,
                    ..c.ink4
                },
            );
        }
        GridStyle::SmallCrosses => {
            let crosses = canvas::Path::new(|b| {
                for x in columns() {
                    for y in rows() {
                        b.move_to(Point::new(x - CROSS_ARM, y));
                        b.line_to(Point::new(x + CROSS_ARM, y));
                        b.move_to(Point::new(x, y - CROSS_ARM));
                        b.line_to(Point::new(x, y + CROSS_ARM));
                    }
                }
            });
            frame.stroke(
                &crosses,
                canvas::Stroke::default()
                    .with_color(c.ink4)
                    .with_width(CROSS_LINE),
            );
        }
    }
}

/// A resistor centred at `at`, one grid step `s` either side, and the wire
/// leaving its right lead with a bend.
fn draw_resistor(frame: &mut canvas::Frame, at: Point, s: f32, c: &PreviewColours) {
    let body = canvas::Path::rectangle(
        Point::new(at.x - s, at.y - s * 0.4),
        Size::new(2.0 * s, s * 0.8),
    );
    frame.fill(&body, c.part_fill);
    let line = canvas::Stroke::default()
        .with_color(c.ink2)
        .with_width(BODY_LINE);
    frame.stroke(&body, line);
    let leads = canvas::Path::new(|b| {
        b.move_to(Point::new(at.x - 2.0 * s, at.y));
        b.line_to(Point::new(at.x - s, at.y));
        b.move_to(Point::new(at.x + s, at.y));
        b.line_to(Point::new(at.x + 2.0 * s, at.y));
    });
    frame.stroke(&leads, line);
    let wire = canvas::Path::new(|b| {
        b.move_to(Point::new(at.x + 2.0 * s, at.y));
        b.line_to(Point::new(at.x + 4.0 * s, at.y));
        b.line_to(Point::new(at.x + 4.0 * s, at.y + s));
    });
    frame.stroke(
        &wire,
        canvas::Stroke::default()
            .with_color(c.info)
            .with_width(WIRE_LINE),
    );
}

/// A symbol body on the grid `s` from `origin`, with two pins on its left.
fn draw_symbol(frame: &mut canvas::Frame, origin: Point, s: f32, c: &PreviewColours) {
    if !s.is_finite() || s <= 0.0 {
        return;
    }
    let steps = |px: f32, at_least: f32| (px / s).round().max(at_least) * s;
    let (x, y) = (origin.x + steps(70.0, 0.0), origin.y + steps(40.0, 0.0));
    let body = canvas::Path::rectangle(
        Point::new(x, y),
        Size::new(steps(100.0, 2.0), steps(80.0, 2.0)),
    );
    frame.fill(&body, c.part_fill);
    let line = canvas::Stroke::default()
        .with_color(c.ink2)
        .with_width(BODY_LINE);
    frame.stroke(&body, line);
    let pin = steps(24.0, 1.0);
    let pins = canvas::Path::new(|b| {
        for row in [1.0, 2.0] {
            b.move_to(Point::new(x - pin, y + row * s));
            b.line_to(Point::new(x, y + row * s));
        }
    });
    frame.stroke(&pins, line);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caption_follows_both_pickers_and_the_symbol_grid() {
        let en = I18n::en_us();
        let shown = |sch, sym, mm| en_plain(caption(&en, sch, sym, mm));
        assert_eq!(
            shown(GridStyle::Dots, GridStyle::SmallCrosses, 1.27),
            "Schematic: Dots · Symbol: Small crosses, 1.27 mm"
        );
        assert_eq!(
            shown(GridStyle::Lines, GridStyle::Dots, 0.635),
            "Schematic: Lines · Symbol: Dots, 0.635 mm"
        );
    }

    #[test]
    fn each_half_names_its_style_and_grid() {
        let en = I18n::en_us();
        assert_eq!(
            half_caption(
                &en,
                ids::PREFERENCES_APPEARANCE_GRID_PREVIEW_SYMBOL,
                GridStyle::SmallCrosses,
                2.54
            ),
            "Symbol · Small crosses · 2.54 mm"
        );
    }

    /// The text without Fluent's isolation marks.
    fn en_plain(text: Localized) -> String {
        text.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }
}
