//! The orientation tile (mockup row 194): +X and +Y arrows from an origin
//! dot in the zoom pill's shell, three rows square, with an optional
//! caption row under the arrows. The arrows turn; their labels stay upright.

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{Column, container};
use iced::{Border, Color, Element, Length, Point, Rectangle, Renderer, Theme, Widget as _, mouse};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::axes::ScreenAxes;
use crate::canvas::zoom_pill::{SHELL_INSET, shell_style};
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, HAIRLINE, ti};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// The mockup's drawing box; the lengths below are in its units.
const BOX: f32 = 72.0;
/// How far the origin dot sits from the box centre, against both arrows.
const ORIGIN_PULL: f32 = 12.0;
/// Shaft end, arrowhead base and tip, from the origin.
const SHAFT: f32 = 28.0;
const HEAD_BASE: f32 = 27.0;
const TIP: f32 = 34.0;
/// Half the width of an arrowhead.
const HEAD_HALF: f32 = 3.5;
const SHAFT_WIDTH: f32 = 1.6;
const ORIGIN_RADIUS: f32 = 2.4;
/// A label sits this far back from its tip and this far to the side.
const LABEL_BACK: f32 = 4.0;
const LABEL_SIDE: f32 = 9.0;
/// The caption row: one caption line and its hairline.
const CAPTION_H: f32 = TS.caption * 1.4 + HAIRLINE;

/// Outer size of the tile: three rows square, plus the caption row when
/// there is one.
pub(super) fn tile_size(has_caption: bool) -> (f32, f32) {
    let side = ChromeMetrics::DEFAULT.row * 3.0;
    let caption = if has_caption { CAPTION_H } else { 0.0 };
    (side, side + caption)
}

/// The tile for `axes`, labelled `labels` (+X, +Y), with `caption` under
/// the arrows when the view is turned or flipped.
pub(super) fn view<'a, M: 'a>(
    tokens: &ThemeTokens,
    axes: ScreenAxes,
    labels: [String; 2],
    caption: Option<Localized>,
) -> Element<'a, M> {
    let (w, h) = tile_size(caption.is_some());
    let side = w - 2.0 * SHELL_INSET;
    let arrows = Arrows {
        axes,
        x_ink: ti(tokens.error),
        y_ink: ti(tokens.success),
        origin_ink: ti(tokens.text_secondary),
        labels,
    };
    let mut column = Column::<Element<'a, M>>::new()
        .width(Length::Fill)
        .align_x(iced::Alignment::Center)
        .push(
            iced::widget::canvas(arrows)
                .width(side)
                .height(side)
                .boxed(),
        );
    if let Some(caption) = caption {
        column = column.push(caption_row(tokens, caption));
    }
    container(column)
        .padding(SHELL_INSET - HAIRLINE)
        .width(w)
        .height(h)
        .style(shell_style(tokens))
        .boxed()
}

/// "90°", "Bottom" or "Bottom · 90°" under a hairline.
fn caption_row<'a, M: 'a>(tokens: &ThemeTokens, caption: Localized) -> Element<'a, M> {
    let ink = ti(tokens.text_secondary);
    let line = resolve(tokens).line_soft;
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    container(text(caption).size(TS.caption).font(mono).color(ink))
        .center_x(Length::Fill)
        .center_y(CAPTION_H)
        .style(move |_: &Theme| {
            let mut border = Border::default();
            border.top = iced::border::Side::default().width(HAIRLINE).color(line);
            container::Style {
                border,
                ..container::Style::default()
            }
        })
        .boxed()
}

/// The drawing: two arrows from the origin dot, with upright labels.
struct Arrows {
    axes: ScreenAxes,
    x_ink: Color,
    y_ink: Color,
    origin_ink: Color,
    labels: [String; 2],
}

/// `p + d * k`.
fn along(p: [f32; 2], d: [f32; 2], k: f32) -> [f32; 2] {
    [p[0] + d[0] * k, p[1] + d[1] * k]
}

/// Where the tile's origin dot sits: off the box centre, against the two
/// arrows, so both fit in the box at any turn.
pub(super) fn origin_of(axes: ScreenAxes) -> [f32; 2] {
    let centre = [BOX / 2.0, BOX / 2.0];
    let sum = [axes.x[0] + axes.y[0], axes.x[1] + axes.y[1]];
    along(centre, sum, -ORIGIN_PULL)
}

/// A label's centre beside the tip of the arrow along `dir`, on the side
/// that faces the box centre, so it stays inside the box.
fn label_spot(tip: [f32; 2], dir: [f32; 2]) -> [f32; 2] {
    let side = [-dir[1], dir[0]];
    let to_centre = [BOX / 2.0 - tip[0], BOX / 2.0 - tip[1]];
    let facing = side[0] * to_centre[0] + side[1] * to_centre[1];
    let side = if facing < 0.0 {
        [-side[0], -side[1]]
    } else {
        side
    };
    along(along(tip, dir, -LABEL_BACK), side, LABEL_SIDE)
}

impl<M> canvas::Program<M> for Arrows {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let scale = bounds.width.min(bounds.height) / BOX;
        let at = |[x, y]: [f32; 2]| Point::new(x * scale, y * scale);
        let origin = origin_of(self.axes);
        let font = iced::Font {
            weight: iced::font::Weight::Medium,
            ..crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
        };
        let arrows = [
            (self.axes.x, self.x_ink, &self.labels[0]),
            (self.axes.y, self.y_ink, &self.labels[1]),
        ];
        for (dir, ink, label) in arrows {
            let shaft = Path::line(at(origin), at(along(origin, dir, SHAFT)));
            frame.stroke(
                &shaft,
                Stroke::default()
                    .with_color(ink)
                    .with_width(SHAFT_WIDTH * scale)
                    .with_line_cap(canvas::LineCap::Round),
            );
            let tip = along(origin, dir, TIP);
            let base = along(origin, dir, HEAD_BASE);
            let side = [-dir[1], dir[0]];
            let head = Path::new(|p| {
                p.move_to(at(tip));
                p.line_to(at(along(base, side, HEAD_HALF)));
                p.line_to(at(along(base, side, -HEAD_HALF)));
                p.close();
            });
            frame.fill(&head, ink);
            frame.fill_text(Text {
                content: label.clone(),
                position: at(label_spot(tip, dir)),
                color: ink,
                size: iced::Pixels(TS.caption),
                align_x: iced::advanced::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                font,
                ..Text::default()
            });
        }
        frame.fill(
            &Path::circle(at(origin), ORIGIN_RADIUS * scale),
            self.origin_ink,
        );
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both arrows and both labels stay inside the drawing box at every
    /// quarter turn, flipped or not.
    #[test]
    fn the_arrows_fit_the_box_at_every_turn() {
        use signex_gfx::view_rotation::ViewRotation;
        for mirrored in [false, true] {
            for degrees in [0.0, 30.0, 90.0, 180.0, 270.0] {
                let rotation = ViewRotation::from_degrees(degrees).with_mirror(mirrored);
                let axes = ScreenAxes::of_board(rotation);
                let origin = origin_of(axes);
                for dir in [axes.x, axes.y] {
                    let tip = along(origin, dir, TIP);
                    for p in [tip, label_spot(tip, dir)] {
                        assert!(
                            (0.0..=BOX).contains(&p[0]) && (0.0..=BOX).contains(&p[1]),
                            "{degrees}° mirrored {mirrored}: {p:?} leaves the box"
                        );
                    }
                }
            }
        }
    }

    /// The mockup's three drawings: the origin dot at (24, 48), (24, 24)
    /// turned 90° and (48, 48) flipped.
    #[test]
    fn the_origin_dot_sits_where_the_mockup_draws_it() {
        use signex_gfx::view_rotation::ViewRotation;
        let at = |degrees: f32, mirrored: bool| {
            let rotation = ViewRotation::from_degrees(degrees).with_mirror(mirrored);
            origin_of(ScreenAxes::of_board(rotation))
        };
        assert_eq!(at(0.0, false), [24.0, 48.0]);
        assert_eq!(at(90.0, false), [24.0, 24.0]);
        assert_eq!(at(0.0, true), [48.0, 48.0]);
    }

    /// Three rows square, the caption row below it when there is one.
    #[test]
    fn the_tile_is_three_rows_square() {
        let row = ChromeMetrics::DEFAULT.row;
        assert_eq!(tile_size(false), (3.0 * row, 3.0 * row));
        assert_eq!(tile_size(true), (3.0 * row, 3.0 * row + CAPTION_H));
    }
}
