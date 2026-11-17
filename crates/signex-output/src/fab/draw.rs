//! Board and footprint graphics (lines, rectangles, circles, arcs and
//! polygons) as Gerber marks. Text is not drawn here: it needs the stroke
//! font that board text drawing (#2387) brings, and is counted instead.

use pcb_geom::{GeomError, PointNm, mm_to_nm};
use signex_types::pcb::{BoardGraphic, FpGraphic, Point};

use super::marks::{ApertureShape, Attributes, Function, Mark, Step, turns_ccw};

/// One graphic, read the same way whether it is a footprint's or the
/// board's own.
pub(crate) struct Drawing<'a> {
    pub kind: &'a str,
    pub width: f64,
    pub start: Option<Point>,
    pub end: Option<Point>,
    pub center: Option<Point>,
    pub mid: Option<Point>,
    pub radius: f64,
    pub points: &'a [Point],
    pub filled: bool,
}

impl<'a> From<&'a FpGraphic> for Drawing<'a> {
    fn from(g: &'a FpGraphic) -> Self {
        Drawing {
            kind: &g.graphic_type,
            width: g.width,
            start: g.start,
            end: g.end,
            center: g.center,
            mid: g.mid,
            radius: g.radius,
            points: &g.points,
            filled: g.fill == "solid",
        }
    }
}

impl<'a> From<&'a BoardGraphic> for Drawing<'a> {
    fn from(g: &'a BoardGraphic) -> Self {
        Drawing {
            kind: &g.graphic_type,
            width: g.width,
            start: g.start,
            end: g.end,
            center: g.center,
            mid: None,
            radius: g.radius,
            points: &g.points,
            filled: false,
        }
    }
}

/// What drawing a graphic gave.
pub(crate) enum Drawn {
    Marks(Vec<Mark>),
    /// A text, left for the stroke font (#2387).
    Text,
    /// A kind this build does not know, or one missing its points.
    Skipped,
}

/// How a graphic is placed and drawn.
pub(crate) struct Pen<'a> {
    /// A model point (footprint-local or board) in the fabrication frame.
    pub to_fab: &'a dyn Fn(Point) -> Result<PointNm, GeomError>,
    /// The turn of an arc that names no middle point, in the fabrication
    /// frame: an arc runs counter-clockwise in its own model frame, which
    /// the frame's y flip reverses and a bottom-side mirror reverses again.
    pub arc_ccw_without_mid: bool,
    pub function: Function,
    /// A pen width that replaces the graphic's own (the profile's).
    pub width_override: Option<i64>,
}

pub(crate) fn draw(drawing: &Drawing, pen: &Pen) -> Result<Drawn, GeomError> {
    let width = match pen.width_override {
        Some(width) => width,
        None => mm_to_nm(drawing.width, "graphic.width")?.abs(),
    };
    let at =
        |p: Option<Point>| -> Result<Option<PointNm>, GeomError> { p.map(pen.to_fab).transpose() };
    let stroke = |start: PointNm, steps: Vec<Step>| Mark::Stroke {
        width,
        function: pen.function,
        start,
        steps,
        attributes: Attributes::default(),
    };
    let marks = match drawing.kind {
        "line" => {
            let (Some(a), Some(b)) = (at(drawing.start)?, at(drawing.end)?) else {
                return Ok(Drawn::Skipped);
            };
            vec![stroke(a, vec![Step::Line(b)])]
        }
        // A text frame's box (#2424); its text is counted by the caller.
        "rect" | "text_frame" => {
            let (Some(a), Some(b)) = (drawing.start, drawing.end) else {
                return Ok(Drawn::Skipped);
            };
            let corners = [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)].map(pen.to_fab);
            let corners = corners.into_iter().collect::<Result<Vec<_>, _>>()?;
            polygon(&corners, drawing.filled, width, pen.function)
        }
        "circle" => {
            let Some(center) = at(drawing.center)? else {
                return Ok(Drawn::Skipped);
            };
            let radius = mm_to_nm(drawing.radius, "graphic.radius")?.abs();
            if drawing.filled {
                vec![Mark::Flash {
                    shape: ApertureShape::Circle {
                        diameter: 2 * radius + width,
                    },
                    function: pen.function,
                    at: center,
                    attributes: Attributes::default(),
                }]
            } else {
                vec![Mark::circle(center, radius, width, pen.function)]
            }
        }
        "arc" => {
            let (Some(center), Some(a), Some(b)) =
                (at(drawing.center)?, at(drawing.start)?, at(drawing.end)?)
            else {
                return Ok(Drawn::Skipped);
            };
            let ccw = match at(drawing.mid)? {
                Some(mid) => turns_ccw(a, mid, b),
                None => pen.arc_ccw_without_mid,
            };
            vec![stroke(a, vec![Step::Arc { to: b, center, ccw }])]
        }
        "poly" | "polygon" => {
            let points = drawing
                .points
                .iter()
                .map(|p| (pen.to_fab)(*p))
                .collect::<Result<Vec<_>, _>>()?;
            if points.len() < 2 {
                return Ok(Drawn::Skipped);
            }
            polygon(&points, drawing.filled, width, pen.function)
        }
        "text" => return Ok(Drawn::Text),
        _ => return Ok(Drawn::Skipped),
    };
    Ok(Drawn::Marks(marks))
}

/// A closed polygon: filled as a region when `filled`, outlined when the
/// pen has a width or the polygon is not filled.
fn polygon(points: &[PointNm], filled: bool, width: i64, function: Function) -> Vec<Mark> {
    let mut marks = Vec::new();
    if filled && points.len() >= 3 {
        marks.push(Mark::Region {
            outline: points.to_vec(),
            holes: Vec::new(),
            attributes: Attributes::default(),
        });
    }
    if !filled || width > 0 {
        marks.extend(Mark::closed_path(points, width, function));
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fab::frame::FabFrame;

    fn board_to_fab() -> impl Fn(Point) -> Result<PointNm, GeomError> {
        let frame = FabFrame {
            origin: PointNm::new(0, 0),
        };
        move |p| frame.of_mm(p, "test")
    }

    fn graphic(kind: &str) -> BoardGraphic {
        BoardGraphic {
            uuid: uuid::Uuid::nil(),
            graphic_type: kind.into(),
            layer: "TopSilkscreen".into(),
            width: 0.2,
            start: Some(Point::new(1.0, 1.0)),
            end: Some(Point::new(3.0, 1.0)),
            center: Some(Point::new(2.0, 1.0)),
            radius: 1.0,
            points: Vec::new(),
            text: String::new(),
            font_size: 0.0,
        }
    }

    /// #2424: a text frame's box is a closed outline, like a rectangle; its
    /// text is left to the stroke font.
    #[test]
    fn a_text_frames_box_is_drawn_as_its_rectangle() {
        let mut frame = graphic("text_frame");
        frame.end = Some(Point::new(3.0, 2.0));
        frame.text = "REV A".into();
        let mut rect = frame.clone();
        rect.graphic_type = "rect".into();
        assert_eq!(drawn(&frame), drawn(&rect));
        assert!(!drawn(&frame).is_empty());
    }

    fn drawn(g: &BoardGraphic) -> Vec<Mark> {
        let to_fab = board_to_fab();
        let pen = Pen {
            to_fab: &to_fab,
            arc_ccw_without_mid: false,
            function: Function::Graphic,
            width_override: None,
        };
        match draw(&Drawing::from(g), &pen).unwrap() {
            Drawn::Marks(marks) => marks,
            _ => panic!("marks"),
        }
    }

    #[test]
    fn a_line_is_one_stroke_in_the_fab_frame() {
        let marks = drawn(&graphic("line"));
        assert_eq!(
            marks,
            vec![Mark::Stroke {
                width: 200_000,
                function: Function::Graphic,
                start: PointNm::new(1_000_000, -1_000_000),
                steps: vec![Step::Line(PointNm::new(3_000_000, -1_000_000))],
                attributes: Attributes::default(),
            }]
        );
    }

    #[test]
    fn a_board_arc_without_a_middle_takes_the_pens_turn() {
        let marks = drawn(&graphic("arc"));
        let Mark::Stroke { steps, .. } = &marks[0] else {
            panic!("a stroke");
        };
        assert!(matches!(steps[0], Step::Arc { ccw: false, .. }));
    }

    #[test]
    fn a_circle_is_a_full_arc_back_to_its_start() {
        let marks = drawn(&graphic("circle"));
        let Mark::Stroke { start, steps, .. } = &marks[0] else {
            panic!("a stroke");
        };
        assert_eq!(*start, PointNm::new(3_000_000, -1_000_000));
        assert!(matches!(steps[0], Step::Arc { to, .. } if to == *start));
    }

    #[test]
    fn text_and_unknown_kinds_draw_nothing() {
        let to_fab = board_to_fab();
        let pen = Pen {
            to_fab: &to_fab,
            arc_ccw_without_mid: false,
            function: Function::Graphic,
            width_override: None,
        };
        let text = graphic("text");
        assert!(matches!(draw(&Drawing::from(&text), &pen), Ok(Drawn::Text)));
        let other = graphic("spline");
        assert!(matches!(
            draw(&Drawing::from(&other), &pen),
            Ok(Drawn::Skipped)
        ));
    }
}
