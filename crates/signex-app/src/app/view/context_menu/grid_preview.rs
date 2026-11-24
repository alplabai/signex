//! The grid picker's read-only preview (Graphite mockup #63): line art of a resistor, a
//! power port and a ground on the picked grid, each pin ringed green when the
//! 2.54 mm pin pitch lands on the grid and amber when it does not. No expand
//! button and no editable label: the ladder is the only control.

use super::*;

use iced::{Length, Point, Rectangle};
use signex_i18n::{I18n, menu_ids};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::property_illustration::{
    Illustration, IllustrationHover, Ink, Paint, Part, Primitive, Style, Tint, Weight,
    property_illustration,
};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The pin pitch of the parts drawn, in mm.
pub(super) const PIN_PITCH_MM: f64 = 2.54;
/// The drawing's view box, in mm: six pitches by four.
const VIEW_MM: (f32, f32) = (15.24, 10.16);
/// Grid lines closer than this, in mm of the view box, are thinned to every
/// n-th line so a fine step still reads as a grid.
const MIN_LINE_GAP_MM: f64 = 0.5;
/// Height of the drawing, in captions.
const HEIGHT_EM: f32 = 11.0;
/// Width of the preview card, in body-text steps.
const CARD_WIDTH_EM: f32 = 22.0;

/// Whether every 2.54 mm pin lands on a grid of `step_mm`.
pub(super) fn pins_on_grid(step_mm: f64) -> bool {
    if !(step_mm.is_finite() && step_mm > 0.0) {
        return false;
    }
    let ratio = PIN_PITCH_MM / step_mm;
    (ratio - ratio.round()).abs() < 1e-6
}

/// The pin ends of the drawn parts, in mm of the view box.
pub(super) const PINS_MM: [(f32, f32); 4] =
    [(2.54, 5.08), (10.16, 5.08), (2.54, 2.54), (10.16, 7.62)];

fn point(mm: (f32, f32)) -> Point {
    Point::new(mm.0, mm.1)
}

fn line(from: (f32, f32), to: (f32, f32), style: Style) -> Part {
    Part::new(
        Primitive::Line {
            from: point(from),
            to: point(to),
        },
        style,
    )
}

/// The grid lines of `step_mm` across the view box, thinned so neighbours
/// stay at least [`MIN_LINE_GAP_MM`] apart.
fn grid_parts(step_mm: f64) -> Vec<Part> {
    let every = (MIN_LINE_GAP_MM / step_mm).ceil().max(1.0);
    let gap = step_mm * every;
    let style = Style::outline(Ink::Rule, Weight::Hairline);
    let mut parts = Vec::new();
    let (w, h) = (f64::from(VIEW_MM.0), f64::from(VIEW_MM.1));
    let mut x = 0.0;
    while x <= w {
        parts.push(line((x as f32, 0.0), (x as f32, VIEW_MM.1), style));
        x += gap;
    }
    let mut y = 0.0;
    while y <= h {
        parts.push(line((0.0, y as f32), (VIEW_MM.0, y as f32), style));
        y += gap;
    }
    parts
}

/// The resistor between its two pins, the power port above the left pin and
/// the ground below the right one, then a ring on every pin end.
fn part_parts(power: Paint, ring: Paint) -> Vec<Part> {
    let edge = Style::edge();
    let power_style = Style::outline(Ink::Secondary, Weight::Regular).with_stroke_paint(power);
    let mut parts = vec![
        // Resistor: leads and body.
        line((2.54, 5.08), (4.06, 5.08), edge),
        line((8.64, 5.08), (10.16, 5.08), edge),
        Part::new(
            Primitive::Rect {
                rect: Rectangle::new(Point::new(4.06, 4.32), iced::Size::new(4.58, 1.52)),
            },
            edge.with_fill(Paint::Ink(Ink::Secondary), Tint::Light),
        ),
        // Power port: a bar over a stem.
        line((2.54, 2.54), (2.54, 1.27), power_style),
        line((1.52, 1.27), (3.56, 1.27), power_style),
        line((2.54, 2.54), (2.54, 5.08), edge),
        // Ground: a stem and three shrinking bars.
        line((10.16, 5.08), (10.16, 7.62), edge),
        line((10.16, 7.62), (10.16, 8.38), power_style),
        line((9.14, 8.38), (11.18, 8.38), power_style),
        line((9.53, 8.89), (10.79, 8.89), power_style),
        line((9.91, 9.40), (10.41, 9.40), power_style),
    ];
    let ring_style = Style::outline(Ink::Secondary, Weight::Thin).with_stroke_paint(ring);
    parts.extend(PINS_MM.iter().map(|&pin| {
        Part::new(
            Primitive::Circle {
                center: point(pin),
                radius: 0.38,
            },
            ring_style,
        )
    }));
    parts
}

/// The pin ring ink on a grid of `step_mm`: the ok token on the pitch, the
/// warning token off it.
pub(super) fn ring_color(step_mm: f64, tokens: &ThemeTokens) -> iced::Color {
    if pins_on_grid(step_mm) {
        signex_widgets::theme_ext::success_color(tokens)
    } else {
        signex_widgets::theme_ext::warning_color(tokens)
    }
}

/// The preview's line art for `step_mm`.
pub(super) fn grid_illustration(step_mm: f64, tokens: &ThemeTokens, tid: ThemeId) -> Illustration {
    let power = signex_widgets::theme_ext::power_port(&signex_types::theme::canvas_colors(tid));
    let ring = ring_color(step_mm, tokens);
    Illustration::new(VIEW_MM.0, VIEW_MM.1)
        .height_em(HEIGHT_EM)
        .parts(grid_parts(step_mm))
        .parts(part_parts(Paint::Document(power), Paint::Document(ring)))
}

/// The preview card: its title and the step it shows, the drawing, and a
/// line saying whether the pins land on the grid.
pub(super) fn grid_preview_card<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    tid: ThemeId,
    step_mm: f64,
    step_name: String,
) -> Element<'a, Message> {
    use iced::widget::{Widget as _, column, container, row};
    let ink = crate::styles::ti(tokens.text);
    let muted = crate::styles::tokens::muted_text(tokens);
    let title = signex_widgets::text(i18n.message(menu_ids::CTX_GRID_PREVIEW, &[]))
        .size(TS.body)
        .color(ink)
        .font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..signex_widgets::ui_font::regular()
        });
    let caption = signex_widgets::text(i18n.message(
        menu_ids::CTX_GRID_PREVIEW_ACTIVE,
        &[("step", step_name.into())],
    ))
    .size(TS.caption)
    .color(muted);
    let help = if pins_on_grid(step_mm) {
        menu_ids::CTX_GRID_PREVIEW_ON_PITCH
    } else {
        menu_ids::CTX_GRID_PREVIEW_OFF_PITCH
    };
    let drawing = property_illustration(
        grid_illustration(step_mm, tokens, tid),
        &IllustrationHover::default(),
        tokens,
        TS.caption,
    )
    .map(|_| Message::Ui(UiMsg::Noop));
    let skin = crate::styles::tokens::resolve(tokens);
    let border = crate::styles::ti(tokens.border);
    container(
        column![
            row![
                title,
                iced::widget::Space::new().width(Length::Fill),
                caption
            ]
            .align_y(iced::Alignment::Center),
            drawing,
            signex_widgets::text(i18n.message(help, &[]))
                .size(TS.caption)
                .color(muted),
        ]
        .spacing(SP.s),
    )
    .width(Length::Fixed(TS.body * CARD_WIDTH_EM))
    .padding(SP.m)
    .style(move |_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(skin.raised)),
        border: iced::Border {
            width: 1.0,
            radius: crate::styles::Radii::GRAPHITE.card.into(),
            color: border,
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mil steps that divide 2.54 mm keep the pins on the grid; the
    /// metric steps do not.
    #[test]
    fn the_pin_pitch_lands_on_mil_steps_and_misses_metric_ones() {
        for on in [0.0254, 0.127, 0.254, 0.508, 0.635, 1.27, 2.54] {
            assert!(pins_on_grid(on), "{on}");
        }
        for off in [0.025, 0.1, 0.25, 0.5, 1.0, 2.5] {
            assert!(!pins_on_grid(off), "{off}");
        }
        assert!(!pins_on_grid(0.0));
        assert!(!pins_on_grid(f64::NAN));
    }

    /// Every drawn pin sits on the 2.54 mm pitch the rings test.
    #[test]
    fn every_pin_sits_on_the_pin_pitch() {
        for (x, y) in PINS_MM {
            for v in [x, y] {
                let n = f64::from(v) / PIN_PITCH_MM;
                assert!((n - n.round()).abs() < 1e-4, "{v}");
            }
        }
    }

    /// A fine step is thinned: the grid never draws more lines than fit at
    /// the minimum gap.
    #[test]
    fn a_fine_grid_is_thinned_to_the_minimum_gap() {
        let fine = grid_parts(0.0254).len();
        let most = ((f64::from(VIEW_MM.0) + f64::from(VIEW_MM.1)) / MIN_LINE_GAP_MM) as usize + 2;
        assert!(fine <= most, "{fine} > {most}");
        assert!(grid_parts(2.54).len() >= 8);
    }

    /// The rings are green on a mil step and amber on a metric one, in both
    /// Graphite themes.
    #[test]
    fn the_rings_follow_the_step_in_both_themes() {
        for tid in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = signex_types::theme::theme_tokens(tid);
            assert_eq!(
                ring_color(1.27, &tokens),
                signex_widgets::theme_ext::success_color(&tokens),
                "{tid:?}"
            );
            assert_eq!(
                ring_color(1.0, &tokens),
                signex_widgets::theme_ext::warning_color(&tokens),
                "{tid:?}"
            );
        }
    }

    /// One ring per pin, drawn after the parts so it sits on top.
    #[test]
    fn every_pin_gets_a_ring_on_top() {
        let parts = part_parts(
            Paint::Document(iced::Color::BLACK),
            Paint::Document(iced::Color::WHITE),
        );
        let rings = parts
            .iter()
            .rev()
            .take_while(|p| matches!(p.primitive, Primitive::Circle { .. }))
            .count();
        assert_eq!(rings, PINS_MM.len());
    }
}
