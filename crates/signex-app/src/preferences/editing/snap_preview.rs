//! The drawing of the snap preview on Editing ▸ Grid (#147): the schematic
//! canvas with Fast grid 1 as dots and Fast grid 2 as rings, a wire snapped
//! to the grid, the pointer pulled to the nearest point, and a dimension
//! arrow per pitch. The pitch labels and handles sit over it
//! (`snap_labels`); both read their positions from [`SnapGeometry`].

use crate::preferences::PrefMsg;
use crate::preferences::appearance::grid_preview::grid_marks;
use crate::preferences::pages_view::PREVIEW_PX_PER_MM;
use crate::preferences::pages_view::controls::PreviewColours;
use crate::render_config::GridStyle;
use crate::styles;
use iced::widget::canvas;
use iced::{Color, Font, Pixels, Point, Rectangle, Size, Theme, mouse};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;

/// Mockup geometry, in drawing px: the grid area's margins, the rows of
/// the two dimension arrows, and how far a label sits past its arrow.
const LEFT: f32 = 40.0;
const TOP: f32 = 46.0;
const RIGHT_MARGIN: f32 = 20.0;
const BOTTOM_MARGIN: f32 = 34.0;
const DIM_1_Y: f32 = 30.0;
const DIM_2_FROM_BOTTOM: f32 = 16.0;
const LABEL_PAST_ARROW: f32 = 110.0;
/// A label's centre stays this far inside the drawing's side edges.
const LABEL_EDGE: f32 = 110.0;
/// The finest pitch drawn as it is; a finer grid draws every n-th point.
const MIN_DRAWN_PX: f32 = 6.0;
const RING_R: f32 = 3.2;
const WIRE_W: f32 = 1.8;
const WIRE_DOT_R: f32 = 2.6;
const CROSS_ARM: f32 = 9.0;
const ARROW_HEAD: f32 = 5.0;
const ARROW_TICK: f32 = 6.0;

/// Where everything on the snap preview goes, for a drawing of `size` and
/// the two pitches the fields hold (`None` when Save would refuse one).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SnapGeometry {
    pub size: Size,
    pub pitches: [Option<f32>; 2],
    /// Fast grid 1 in px (the default grid stands in when it is refused).
    s1: f32,
    /// Every n-th Fast grid 1 point is drawn.
    pub every: f32,
}

impl SnapGeometry {
    pub(super) fn new(size: Size, pitches: [Option<f32>; 2]) -> Self {
        let mm = pitches[0].unwrap_or(crate::canvas::grid::FAST_GRIDS_MM[0]);
        let s1 = mm * PREVIEW_PX_PER_MM;
        let every = (MIN_DRAWN_PX / s1).ceil().max(1.0);
        Self {
            size,
            pitches,
            s1,
            every,
        }
    }

    fn area(&self) -> Rectangle {
        Rectangle::new(
            Point::new(LEFT, TOP),
            Size::new(
                self.size.width - RIGHT_MARGIN - LEFT,
                self.size.height - BOTTOM_MARGIN - TOP,
            ),
        )
    }

    /// The drawn Fast grid 1 pitch, in px.
    fn drawn_step(&self) -> f32 {
        self.s1 * self.every
    }

    /// Point `(i, j)` of Fast grid 1.
    fn grid_point(&self, i: f32, j: f32) -> Point {
        Point::new(LEFT + i * self.s1, TOP + j * self.s1)
    }

    /// Fast grid 1 points across and down the area.
    fn grid_counts(&self) -> (f32, f32) {
        let area = self.area();
        (
            (area.width / self.s1).floor().max(4.0),
            (area.height / self.s1).floor().max(3.0),
        )
    }

    /// Ends of the Fast grid 1 arrow.
    fn arrow_1(&self) -> (f32, f32) {
        let start = LEFT + self.s1;
        (start, start + self.drawn_step())
    }

    /// End of the Fast grid 2 arrow, at most the area's right edge.
    fn arrow_2_end(&self) -> Option<f32> {
        let start = self.arrow_1().0;
        let right = self.size.width - RIGHT_MARGIN;
        self.pitches[1].map(|mm| (start + mm * PREVIEW_PX_PER_MM).min(right))
    }

    fn dim_2_y(&self) -> f32 {
        self.size.height - DIM_2_FROM_BOTTOM
    }

    fn label_x(&self, x: f32) -> f32 {
        x.min(self.size.width - LABEL_EDGE).max(LABEL_EDGE)
    }

    /// Centre of the pitch label of fast grid `index`.
    pub(super) fn label_at(&self, index: usize) -> Point {
        if index == 0 {
            Point::new(self.label_x(self.arrow_1().1 + LABEL_PAST_ARROW), DIM_1_Y)
        } else {
            let end = self.arrow_2_end().unwrap_or(self.arrow_1().0);
            Point::new(self.label_x(end + LABEL_PAST_ARROW), self.dim_2_y())
        }
    }

    /// Centre of the drag handle of fast grid `index`; none for a refused
    /// pitch or an arrow end past the drawing.
    pub(super) fn handle_at(&self, index: usize) -> Option<Point> {
        let right = self.size.width - RIGHT_MARGIN;
        if index == 0 {
            let end = self.arrow_1().1;
            (self.pitches[0].is_some() && end <= right).then_some(Point::new(end, DIM_1_Y))
        } else {
            let end = self.arrow_2_end()?;
            (end < right).then_some(Point::new(end, self.dim_2_y()))
        }
    }
}

/// The notes drawn on the preview, already in the UI language.
pub(super) struct SnapNotes {
    /// "1 point in n drawn", when Fast grid 1 is drawn sparser than it is.
    pub every: Option<String>,
    pub snaps_here: String,
    /// Shown while Fast grid 2 is refused.
    pub no_rings: String,
}

pub(super) struct SnapDrawing {
    pub geometry: SnapGeometry,
    pub colours: PreviewColours,
    pub notes: SnapNotes,
}

impl canvas::Program<PrefMsg> for SnapDrawing {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let (g, c) = (&self.geometry, &self.colours);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), c.paper);
        grid_marks(&mut frame, g.area(), g.drawn_step(), GridStyle::Dots, c);
        self.draw_rings(&mut frame);
        self.draw_wire_and_pointer(&mut frame);
        self.draw_dimensions(&mut frame);
        vec![frame.into_geometry()]
    }
}

impl SnapDrawing {
    /// Fast grid 2 as rings, when it is valid and coarse enough to read.
    fn draw_rings(&self, frame: &mut canvas::Frame) {
        let (g, c) = (&self.geometry, &self.colours);
        let Some(step) = g.pitches[1].map(|mm| mm * PREVIEW_PX_PER_MM) else {
            return;
        };
        if !step.is_finite() || step < MIN_DRAWN_PX {
            return;
        }
        let area = g.area();
        let (x1, y1) = (area.x + area.width, area.y + area.height);
        let rings = canvas::Path::new(|b| {
            let mut x = area.x;
            while x <= x1 {
                let mut y = area.y;
                while y <= y1 {
                    b.circle(Point::new(x, y), RING_R);
                    y += step;
                }
                x += step;
            }
        });
        frame.stroke(
            &rings,
            canvas::Stroke::default().with_color(c.ink3).with_width(1.0),
        );
    }

    /// A wire on Fast grid 1 and the pointer pulled to its nearest point.
    fn draw_wire_and_pointer(&self, frame: &mut canvas::Frame) {
        let (g, c) = (&self.geometry, &self.colours);
        let (nx, ny) = g.grid_counts();
        let start = g.grid_point((nx * 0.25).round(), (ny * 0.4).round());
        let end = g.grid_point((nx * 0.55).round(), (ny * 0.8).round());
        let wire = canvas::Path::new(|b| {
            b.move_to(start);
            b.line_to(Point::new(end.x, start.y));
            b.line_to(end);
        });
        frame.stroke(
            &wire,
            canvas::Stroke::default()
                .with_color(c.info)
                .with_width(WIRE_W),
        );
        frame.fill(&canvas::Path::circle(start, WIRE_DOT_R), c.info);
        let snap = g.grid_point((nx * 0.78).round(), (ny * 0.5).round());
        let pointer = Point::new(snap.x + g.s1 * 0.38, snap.y + g.s1 * 0.31);
        let mut pull = canvas::Stroke::default().with_color(c.ink4).with_width(1.0);
        pull.line_dash = canvas::LineDash {
            segments: &[3.0, 2.0],
            offset: 0,
        };
        frame.stroke(&canvas::Path::line(pointer, snap), pull);
        let arrow = canvas::Path::new(|b| {
            b.move_to(pointer);
            b.line_to(Point::new(pointer.x, pointer.y + 12.0));
            b.line_to(Point::new(pointer.x + 3.4, pointer.y + 8.8));
            b.line_to(Point::new(pointer.x + 7.8, pointer.y + 8.8));
            b.close();
        });
        frame.fill(&arrow, Color { a: 0.6, ..c.ink3 });
        let cross = canvas::Path::new(|b| {
            b.move_to(Point::new(snap.x - CROSS_ARM, snap.y));
            b.line_to(Point::new(snap.x + CROSS_ARM, snap.y));
            b.move_to(Point::new(snap.x, snap.y - CROSS_ARM));
            b.line_to(Point::new(snap.x, snap.y + CROSS_ARM));
        });
        frame.stroke(
            &cross,
            canvas::Stroke::default()
                .with_color(c.accent)
                .with_width(1.4),
        );
        note(
            frame,
            &self.notes.snaps_here,
            Point::new(snap.x + 12.0, snap.y - 8.0 - TS.caption),
            c.ink4,
        );
    }

    /// The dimension arrow of each pitch; a refused Fast grid 2 gets a note
    /// in the error colour instead.
    fn draw_dimensions(&self, frame: &mut canvas::Frame) {
        let (g, c) = (&self.geometry, &self.colours);
        let (a0, a1) = g.arrow_1();
        dimension_arrow(frame, a0, a1, DIM_1_Y, c.accent);
        if let Some(every) = &self.notes.every {
            let x = g.label_at(0).x + LABEL_PAST_ARROW;
            note(
                frame,
                every,
                Point::new(x, DIM_1_Y - TS.caption / 2.0),
                c.ink4,
            );
        }
        match g.arrow_2_end() {
            Some(end) => dimension_arrow(frame, a0, end, g.dim_2_y(), c.accent),
            None => {
                let x = g.label_at(1).x + LABEL_PAST_ARROW;
                let at = Point::new(x, g.dim_2_y() - TS.caption / 2.0);
                note(frame, &self.notes.no_rings, at, c.err);
            }
        }
    }
}

/// A horizontal dimension arrow from `x0` to `x1` at `y`, with end ticks.
fn dimension_arrow(frame: &mut canvas::Frame, x0: f32, x1: f32, y: f32, colour: Color) {
    let path = canvas::Path::new(|b| {
        b.move_to(Point::new(x0, y));
        b.line_to(Point::new(x1, y));
        for (tip, dir) in [(x0, 1.0), (x1, -1.0)] {
            b.move_to(Point::new(tip + dir * ARROW_HEAD, y - 3.0));
            b.line_to(Point::new(tip, y));
            b.line_to(Point::new(tip + dir * ARROW_HEAD, y + 3.0));
            b.move_to(Point::new(tip, y - ARROW_TICK));
            b.line_to(Point::new(tip, y + ARROW_TICK));
        }
    });
    frame.stroke(
        &path,
        canvas::Stroke::default().with_color(colour).with_width(1.2),
    );
}

fn note(frame: &mut canvas::Frame, content: &str, top_left: Point, colour: Color) {
    frame.fill_text(canvas::Text {
        content: content.to_owned(),
        position: top_left,
        color: colour,
        size: Pixels(TS.caption * 10.0 / 11.0),
        font: Font::MONOSPACE,
        ..canvas::Text::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Size = Size::new(700.0, 200.0);

    #[test]
    fn a_refused_fast_grid_has_no_handle_and_its_label_stays_on_the_drawing() {
        let g = SnapGeometry::new(SIZE, [Some(1.27), None]);
        assert!(g.handle_at(0).is_some());
        assert!(g.handle_at(1).is_none());
        let label = g.label_at(1);
        assert!(label.x >= LABEL_EDGE && label.x <= SIZE.width - LABEL_EDGE);
    }

    #[test]
    fn a_fine_fast_grid_is_drawn_with_every_nth_point() {
        let fine = SnapGeometry::new(SIZE, [Some(0.01), Some(2.54)]);
        assert!(fine.every > 1.0);
        assert!(fine.drawn_step() >= MIN_DRAWN_PX);
        let normal = SnapGeometry::new(SIZE, [Some(1.27), Some(2.54)]);
        assert_eq!(normal.every, 1.0);
    }

    #[test]
    fn a_huge_pitch_keeps_its_label_on_the_drawing_and_drops_its_handle() {
        let g = SnapGeometry::new(SIZE, [Some(1000.0), Some(1000.0)]);
        assert!(g.handle_at(0).is_none());
        assert!(g.handle_at(1).is_none());
        for index in 0..2 {
            let x = g.label_at(index).x;
            assert!(x <= SIZE.width - LABEL_EDGE, "{index}: {x}");
        }
    }
}
