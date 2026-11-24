//! A read DXF drawing as plain edges for the board sketch import (mockup row
//! 179c): its layers, the units it can be read in, and every entity as a
//! line, arc or circle in board coordinates. Nothing here changes a board.

use signex_types::pcb::Point;
use signex_types::pcb_options::BoardOrigin;

use super::DxfError;
use super::parse::{DxfDrawing, DxfEntity, parse_dxf_raw};
use super::shapes::{bulge_arc, on_circle, polyline_segments};
use crate::pcb::graphics::is_storable_point;

/// The units an import reads the file's numbers in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxfUnits {
    /// What the file's header says; mm when it says nothing.
    FromFile,
    Millimetre,
    Mil,
    Inch,
}

impl DxfUnits {
    /// Every choice, in the order the import dialog lists them.
    pub const ALL: [Self; 4] = [Self::FromFile, Self::Millimetre, Self::Mil, Self::Inch];

    /// Millimetres per file unit; `file_to_mm` is the header's factor.
    pub fn to_mm(self, file_to_mm: f64) -> f64 {
        match self {
            Self::FromFile => file_to_mm,
            Self::Millimetre => 1.0,
            Self::Mil => 0.0254,
            Self::Inch => 25.4,
        }
    }
}

/// A DXF file read for the import dialog: its drawing in file units.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfSource {
    pub drawing: DxfDrawing,
    /// Millimetres per file unit, from the header.
    pub file_to_mm: f64,
}

impl DxfSource {
    /// Read `text`; a file with nothing to import is refused.
    pub fn read(text: &str) -> Result<Self, DxfError> {
        let (drawing, file_to_mm) = parse_dxf_raw(text)?;
        if drawing.entities.is_empty() {
            return Err(DxfError::Empty);
        }
        Ok(Self {
            drawing,
            file_to_mm,
        })
    }
}

/// One DXF layer and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxfLayerRow {
    pub name: String,
    /// Entities the import can draw.
    pub drawable: usize,
    /// How many of `drawable` are circles.
    pub circles: usize,
    /// Entities of a kind the import does not read.
    pub skipped: usize,
}

/// The layers of `drawing`, in the order the file first names them.
pub fn dxf_layer_rows(drawing: &DxfDrawing) -> Vec<DxfLayerRow> {
    let mut rows: Vec<DxfLayerRow> = Vec::new();
    for (entity, layer) in drawing.entities.iter().zip(&drawing.layers) {
        let i = row_of(&mut rows, layer);
        rows[i].drawable += 1;
        rows[i].circles += usize::from(matches!(entity, DxfEntity::Circle { .. }));
    }
    for layer in &drawing.skipped_layers {
        let i = row_of(&mut rows, layer);
        rows[i].skipped += 1;
    }
    rows
}

/// The index of the row of layer `name`, added when it is new.
fn row_of(rows: &mut Vec<DxfLayerRow>, name: &str) -> usize {
    if let Some(i) = rows.iter().position(|r| r.name == name) {
        return i;
    }
    rows.push(DxfLayerRow {
        name: name.to_string(),
        drawable: 0,
        circles: 0,
        skipped: 0,
    });
    rows.len() - 1
}

/// One edge in board coordinates. An arc runs the way of increasing angle
/// from `start` to `end`, as a board arc does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DxfEdgeShape {
    Line {
        a: Point,
        b: Point,
    },
    Arc {
        center: Point,
        start: Point,
        end: Point,
    },
    Circle {
        center: Point,
        radius: f64,
    },
}

/// An edge and the index of its layer in [`dxf_layer_rows`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DxfEdge {
    pub shape: DxfEdgeShape,
    pub layer: usize,
}

/// Every entity of `drawing`, scaled by `factor` (mm per file unit) about
/// the file's origin, in board coordinates. Refused when a point leaves the
/// range a board file holds.
pub fn dxf_edges(
    drawing: &DxfDrawing,
    origin: BoardOrigin,
    factor: f64,
) -> Result<Vec<DxfEdge>, DxfError> {
    let rows = dxf_layer_rows(drawing);
    let mut edges = Vec::new();
    for (entity, name) in drawing.entities.iter().zip(&drawing.layers) {
        let layer = rows.iter().position(|r| &r.name == name).unwrap_or(0);
        for shape in entity_shapes(entity, origin, factor) {
            if !shape_is_storable(&shape) {
                return Err(DxfError::OutOfRange);
            }
            edges.push(DxfEdge { shape, layer });
        }
    }
    Ok(edges)
}

fn entity_shapes(entity: &DxfEntity, origin: BoardOrigin, factor: f64) -> Vec<DxfEdgeShape> {
    let scaled = |p: Point| Point::new(p.x * factor, p.y * factor);
    let at = |p: Point| Point::new(origin.board_x(p.x), origin.board_y(p.y));
    // A counter-clockwise arc of the file turns clockwise in board axes,
    // so its ends swap to run the way of increasing angle.
    let arc = |center: Point, from: Point, to: Point| DxfEdgeShape::Arc {
        center: at(center),
        start: at(to),
        end: at(from),
    };
    match entity {
        DxfEntity::Line { a, b } => vec![DxfEdgeShape::Line {
            a: at(scaled(*a)),
            b: at(scaled(*b)),
        }],
        DxfEntity::Circle { center, radius } => vec![DxfEdgeShape::Circle {
            center: at(scaled(*center)),
            radius: radius * factor,
        }],
        DxfEntity::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let (c, r) = (scaled(*center), radius * factor);
            if (end_deg - start_deg).rem_euclid(360.0) == 0.0 {
                return vec![DxfEdgeShape::Circle {
                    center: at(c),
                    radius: r,
                }];
            }
            vec![arc(
                c,
                on_circle(c, r, *start_deg),
                on_circle(c, r, *end_deg),
            )]
        }
        DxfEntity::Polyline { vertices, closed } => {
            let vertices: Vec<(Point, f64)> =
                vertices.iter().map(|(p, b)| (scaled(*p), *b)).collect();
            polyline_segments(&vertices, *closed)
                .into_iter()
                .map(|(a, b, bulge)| match bulge_arc(a, b, bulge) {
                    Some((center, _, from, to)) => arc(center, from, to),
                    None => DxfEdgeShape::Line { a: at(a), b: at(b) },
                })
                .collect()
        }
    }
}

fn shape_is_storable(shape: &DxfEdgeShape) -> bool {
    match shape {
        DxfEdgeShape::Line { a, b } => is_storable_point(a) && is_storable_point(b),
        DxfEdgeShape::Arc { center, start, end } => {
            is_storable_point(center) && is_storable_point(start) && is_storable_point(end)
        }
        DxfEdgeShape::Circle { center, radius } => {
            *radius > 0.0
                && is_storable_point(center)
                && is_storable_point(&Point::new(center.x + radius, center.y))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two lines on BOARD, a circle on HOLES and a text on NOTES, in inches.
    const FILE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n1\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n0\n11\n1\n21\n0\n\
0\nLINE\n8\nBOARD\n10\n1\n20\n0\n11\n1\n21\n1\n\
0\nCIRCLE\n8\nHOLES\n10\n0.5\n20\n0.5\n40\n0.1\n\
0\nTEXT\n8\nNOTES\n10\n0\n20\n0\n\
0\nENDSEC\n0\nEOF\n";

    #[test]
    fn the_layers_are_listed_with_what_each_holds() {
        let source = DxfSource::read(FILE).unwrap();
        assert_eq!(source.file_to_mm, 25.4);
        let rows = dxf_layer_rows(&source.drawing);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["BOARD", "HOLES", "NOTES"]);
        assert_eq!((rows[0].drawable, rows[0].circles), (2, 0));
        assert_eq!((rows[1].drawable, rows[1].circles), (1, 1));
        assert_eq!((rows[2].drawable, rows[2].skipped), (0, 1));
    }

    #[test]
    fn the_chosen_units_and_scale_size_the_edges() {
        let source = DxfSource::read(FILE).unwrap();
        let origin = BoardOrigin::default();
        let factor = DxfUnits::Millimetre.to_mm(source.file_to_mm) * 2.0;
        let edges = dxf_edges(&source.drawing, origin, factor).unwrap();
        let DxfEdgeShape::Line { b, .. } = edges[0].shape else {
            panic!("{:?}", edges[0]);
        };
        assert_eq!(b.x, origin.board_x(2.0));
        assert_eq!(edges[2].layer, 1);
    }

    #[test]
    fn a_scale_past_the_file_range_is_refused() {
        let source = DxfSource::read(FILE).unwrap();
        let far = dxf_edges(&source.drawing, BoardOrigin::default(), 1.0e30);
        assert_eq!(far, Err(DxfError::OutOfRange));
    }
}
