//! The graph column: lane segments, merge rings and commit dots on one
//! canvas beside the rows. Geometry is a pure function of the layout and
//! the text size; the canvas program only strokes and fills it.

use iced::widget::canvas::{self, Frame, LineCap, LineDash, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};

use super::lanes::{DotKind, Edge, EdgeStyle, GraphLayout};
use super::style::{
    COLUMN_EM, DOT_RADIUS_EM, HistoryGraphStyle, LANE_DASH_EM, LANE_STEP_EM, LANE_X0_EM, LINE_EM,
    RING_RADIUS_EM, RING_STROKE_EM, ROW_EM, WORKING_TREE_DASH_EM, WORKING_TREE_RADIUS_EM,
    WORKING_TREE_STROKE_EM,
};

/// Where the graph's shapes go, for one text size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphGeometry {
    pub row_height: f32,
    pub lane_x0: f32,
    pub lane_step: f32,
    pub min_width: f32,
}

/// A lane segment between two adjacent rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentShape {
    Line {
        from: Point,
        to: Point,
    },
    /// A cubic from one lane to another over one row: both handles at the
    /// span's middle height, each above its own end.
    Curve {
        from: Point,
        handle_from: Point,
        handle_to: Point,
        to: Point,
    },
}

impl GraphGeometry {
    pub fn new(text_size: f32) -> Self {
        Self {
            row_height: text_size * ROW_EM,
            lane_x0: text_size * LANE_X0_EM,
            lane_step: text_size * LANE_STEP_EM,
            min_width: text_size * COLUMN_EM,
        }
    }

    /// Width of the column: the mockup's two-lane width, wider only when
    /// more lanes are drawn (the same margin each side).
    pub fn width(&self, lane_count: usize) -> f32 {
        let lanes = lane_count.saturating_sub(1) as f32;
        self.min_width
            .max(2.0 * self.lane_x0 + self.lane_step * lanes)
    }

    pub fn height(&self, rows: usize) -> f32 {
        self.row_height * rows as f32
    }

    /// The centre of the dot of `row` in `lane`.
    pub fn dot(&self, row: usize, lane: usize) -> Point {
        Point::new(
            self.lane_x0 + self.lane_step * lane as f32,
            self.row_height * (row as f32 + 0.5),
        )
    }

    /// The segment `edge` draws from `row` to the row below.
    pub fn segment(&self, row: usize, edge: &Edge) -> SegmentShape {
        let from = self.dot(row, edge.from_lane);
        let to = self.dot(row + 1, edge.to_lane);
        if edge.from_lane == edge.to_lane {
            return SegmentShape::Line { from, to };
        }
        let mid = (from.y + to.y) / 2.0;
        SegmentShape::Curve {
            from,
            handle_from: Point::new(from.x, mid),
            handle_to: Point::new(to.x, mid),
            to,
        }
    }
}

impl SegmentShape {
    fn path(&self) -> Path {
        Path::new(|b| match *self {
            Self::Line { from, to } => {
                b.move_to(from);
                b.line_to(to);
            }
            Self::Curve {
                from,
                handle_from,
                handle_to,
                to,
            } => {
                b.move_to(from);
                b.bezier_curve_to(handle_from, handle_to, to);
            }
        })
    }
}

/// Draws a [`GraphLayout`]; holds no state of its own.
pub(super) struct GraphProgram {
    pub layout: GraphLayout,
    pub style: HistoryGraphStyle,
}

impl GraphProgram {
    fn stroke<'d>(&self, color: Color, width_em: f32, dash: Option<&'d [f32; 2]>) -> Stroke<'d> {
        let mut stroke = Stroke::default()
            .with_color(color)
            .with_width(self.style.em(width_em))
            .with_line_cap(LineCap::Round);
        if let Some(segments) = dash {
            stroke.line_cap = LineCap::Butt;
            stroke.line_dash = LineDash {
                segments: segments.as_slice(),
                offset: 0,
            };
        }
        stroke
    }

    fn paint(&self, frame: &mut Frame) {
        let g = GraphGeometry::new(self.style.text_size);
        let s = &self.style;
        let lane_dash = [s.em(LANE_DASH_EM); 2];
        let ring_dash = [s.em(WORKING_TREE_DASH_EM); 2];
        for (row, layout) in self.layout.rows.iter().enumerate() {
            for edge in &layout.edges {
                let dash = (edge.style == EdgeStyle::Dashed).then_some(&lane_dash);
                let stroke = self.stroke(s.lane_color(edge.color_lane), LINE_EM, dash);
                frame.stroke(&g.segment(row, edge).path(), stroke);
            }
        }
        for (row, layout) in self.layout.rows.iter().enumerate() {
            let centre = g.dot(row, layout.lane);
            let color = s.lane_color(layout.lane);
            match layout.dot {
                DotKind::WorkingTree => {
                    let ring = Path::circle(centre, s.em(WORKING_TREE_RADIUS_EM));
                    frame.fill(&ring, s.panel);
                    let stroke = self.stroke(s.ink4, WORKING_TREE_STROKE_EM, Some(&ring_dash));
                    frame.stroke(&ring, stroke);
                }
                DotKind::Merge => {
                    let ring = Path::circle(centre, s.em(RING_RADIUS_EM));
                    frame.fill(&ring, s.panel);
                    frame.stroke(&ring, self.stroke(color, RING_STROKE_EM, None));
                    frame.fill(&Path::circle(centre, s.em(DOT_RADIUS_EM)), color);
                }
                DotKind::Commit => {
                    frame.fill(&Path::circle(centre, s.em(DOT_RADIUS_EM)), color);
                }
            }
        }
    }
}

impl<M> canvas::Program<M> for GraphProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        self.paint(&mut frame);
        vec![frame.into_geometry()]
    }
}
