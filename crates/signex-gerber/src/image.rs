//! One read image: a Gerber or Excellon file as objects in integer nm.
//!
//! Frame: the file's own, x to the right and y up from the file origin,
//! which for a Signex file is the board origin (the exporter's frame,
//! `signex_output` `fab::frame`). Every object keeps the line it was read
//! from, and an object whose shape cannot be built keeps empty `shapes`
//! so the viewer draws a marker there instead of dropping it.

use std::collections::BTreeMap;
use std::sync::Arc;

use pcb_geom::{PointNm, RectNm, Shape};

use crate::file_function::FileFunction;
use crate::problem::{ReadProblem, Severity};
use crate::source::{FileKind, SourceLine};

/// The unit a file's numbers are written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    Millimetres,
    Inches,
}

/// X2 `.FilePolarity`: whether the image draws material or openings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePolarity {
    Positive,
    Negative,
}

/// Load polarity (`%LPD*%` / `%LPC*%`) an object was drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    Dark,
    Clear,
}

impl Polarity {
    pub(crate) fn inverted(self) -> Polarity {
        match self {
            Polarity::Dark => Polarity::Clear,
            Polarity::Clear => Polarity::Dark,
        }
    }
}

/// An aperture (Gerber D code) or drill tool as the file defines it.
#[derive(Debug, Clone, PartialEq)]
pub struct ApertureDef {
    /// The D code, or the tool number of a drill file.
    pub d_code: u32,
    /// The definition verbatim: `%ADD10R,1X1.2*%`, `T1C0.300`.
    pub definition: String,
    /// The X2 `.AperFunction` value in force when it was defined.
    pub function: Option<String>,
    pub at: SourceLine,
}

/// X2 object attributes (`%TO.N`, `.P`, `.C`), `\uXXXX` escapes decoded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectAttributes {
    pub net: Option<String>,
    /// Designator and pad number.
    pub pin: Option<(String, String)>,
    pub component: Option<String>,
}

/// What an object is, in the file frame.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectKind {
    Flash {
        at: PointNm,
    },
    Line {
        from: PointNm,
        to: PointNm,
    },
    Arc {
        from: PointNm,
        to: PointNm,
        center: PointNm,
        ccw: bool,
    },
    /// A region contour, arcs flattened. Each vertex once; the closing edge
    /// from the last vertex back to the first is implied.
    Region {
        outline: Vec<PointNm>,
    },
    /// A drilled hole, or a routed slot from `at` to `slot_to`. No
    /// `diameter` when its tool has no size.
    Hole {
        at: PointNm,
        diameter: Option<i64>,
        tool: u32,
        slot_to: Option<PointNm>,
    },
}

/// One drawn object.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub kind: ObjectKind,
    pub polarity: Polarity,
    /// The D code of a flash or draw, the tool of a hole; `None` for a region.
    pub aperture: Option<u32>,
    pub attributes: ObjectAttributes,
    pub at: SourceLine,
    /// The material it covers. Empty when it could not be built (an
    /// unknown macro, a tool without a size): drawn as a marker, never
    /// dropped. A clear object is not subtracted here.
    pub shapes: Vec<Shape>,
}

impl Object {
    /// The same object moved by `by` (step and repeat, block flashes).
    pub(crate) fn translated(&self, by: PointNm) -> Object {
        let kind = match self.kind {
            ObjectKind::Flash { at } => ObjectKind::Flash { at: at.plus(by) },
            ObjectKind::Line { from, to } => ObjectKind::Line {
                from: from.plus(by),
                to: to.plus(by),
            },
            ObjectKind::Arc {
                from,
                to,
                center,
                ccw,
            } => ObjectKind::Arc {
                from: from.plus(by),
                to: to.plus(by),
                center: center.plus(by),
                ccw,
            },
            ObjectKind::Region { ref outline } => ObjectKind::Region {
                outline: outline.iter().map(|p| p.plus(by)).collect(),
            },
            ObjectKind::Hole {
                at,
                diameter,
                tool,
                slot_to,
            } => ObjectKind::Hole {
                at: at.plus(by),
                diameter,
                tool,
                slot_to: slot_to.map(|p| p.plus(by)),
            },
        };
        Object {
            kind,
            shapes: self
                .shapes
                .iter()
                .map(|s| s.clone().translated(by))
                .collect(),
            ..self.clone()
        }
    }

    /// The points that place the object, for bounds when it has no shape.
    fn anchors(&self) -> Vec<PointNm> {
        match self.kind {
            ObjectKind::Flash { at } => vec![at],
            ObjectKind::Line { from, to } | ObjectKind::Arc { from, to, .. } => vec![from, to],
            ObjectKind::Region { ref outline } => outline.clone(),
            ObjectKind::Hole { at, slot_to, .. } => std::iter::once(at).chain(slot_to).collect(),
        }
    }
}

/// One Gerber or Excellon file as read.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub file: String,
    /// `Gerber` or `Excellon`.
    pub kind: FileKind,
    pub function: Option<FileFunction>,
    pub polarity: FilePolarity,
    pub generation_software: Option<String>,
    pub project_id: Option<String>,
    pub units: Option<Units>,
    pub apertures: BTreeMap<u32, ApertureDef>,
    /// In file order.
    pub objects: Vec<Object>,
    /// By line.
    pub problems: Vec<ReadProblem>,
}

impl Image {
    pub(crate) fn empty(file: &str, kind: FileKind) -> Image {
        Image {
            file: file.to_string(),
            kind,
            function: None,
            polarity: FilePolarity::Positive,
            generation_software: None,
            project_id: None,
            units: None,
            apertures: BTreeMap::new(),
            objects: Vec::new(),
            problems: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        self.problems
            .iter()
            .any(|p| p.severity() == Severity::Error)
    }

    /// The smallest box holding every object (its shapes, or its anchor
    /// points when it has none), as (min, max); `None` for no objects.
    pub fn bounds(&self) -> Option<(PointNm, PointNm)> {
        self.objects
            .iter()
            .flat_map(|object| {
                if object.shapes.is_empty() {
                    object.anchors().into_iter().map(RectNm::point).collect()
                } else {
                    object.shapes.iter().map(Shape::aabb).collect::<Vec<_>>()
                }
            })
            .reduce(|a, b| a.union(&b))
            .map(|r| (r.min, r.max))
    }
}

/// A polygon shape through `points`: repeated and closing vertices
/// dropped, turned counter-clockwise as `Shape::Polygon` wants; `None`
/// for fewer than three vertices.
pub(crate) fn polygon_shape(points: &[PointNm]) -> Option<Shape> {
    let mut ring = points.to_vec();
    ring.dedup();
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < 3 {
        return None;
    }
    let n = ring.len();
    let doubled_area: i128 = (0..n).map(|i| ring[i].cross(ring[(i + 1) % n])).sum();
    if doubled_area < 0 {
        ring.reverse();
    }
    Some(Shape::Polygon {
        ring: Arc::from(ring),
        radius: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> PointNm {
        PointNm::new(x, y)
    }

    #[test]
    fn a_polygon_is_turned_counter_clockwise_and_opened() {
        let cw = [p(0, 0), p(0, 10), p(10, 10), p(10, 0), p(0, 0)];
        let Some(Shape::Polygon { ring, radius }) = polygon_shape(&cw) else {
            panic!("a polygon");
        };
        assert_eq!(radius, 0);
        assert_eq!(&*ring, &[p(10, 0), p(10, 10), p(0, 10), p(0, 0)]);
        assert_eq!(polygon_shape(&[p(0, 0), p(1, 1), p(0, 0)]), None);
    }

    #[test]
    fn bounds_cover_shapes_and_the_anchors_of_unbuilt_objects() {
        let mut image = Image::empty("x.gbr", FileKind::Gerber);
        assert_eq!(image.bounds(), None);
        let object = |kind, shapes| Object {
            kind,
            polarity: Polarity::Dark,
            aperture: None,
            attributes: ObjectAttributes::default(),
            at: SourceLine {
                file: "x.gbr".into(),
                line: 1,
            },
            shapes,
        };
        image.objects.push(object(
            ObjectKind::Flash { at: p(0, 0) },
            vec![Shape::Disc {
                center: p(0, 0),
                radius: 5,
            }],
        ));
        image
            .objects
            .push(object(ObjectKind::Flash { at: p(100, -20) }, Vec::new()));
        assert_eq!(image.bounds(), Some((p(-5, -20), p(100, 5))));
        let moved = image.objects[0].translated(p(1, 2));
        assert_eq!(moved.kind, ObjectKind::Flash { at: p(1, 2) });
        assert_eq!(
            moved.shapes,
            vec![Shape::Disc {
                center: p(1, 2),
                radius: 5
            }]
        );
    }
}
