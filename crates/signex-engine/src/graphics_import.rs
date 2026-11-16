//! Drawings from a DXF file for Import Graphics (#1667).
//!
//! The ASCII form only: LINE, CIRCLE, ARC, LWPOLYLINE and POLYLINE (with its
//! VERTEX rows). A polyline's bulges are read as straight segments. Every
//! other entity is counted as skipped, so the caller can say so. Lengths are
//! scaled from the header's `$INSUNITS` to mm (no units: mm) and Y is turned
//! down, as the sheet's Y grows down the page. A circle, arc or polyline whose
//! plane faces down the Z axis is mirrored back into the drawing's X.

use signex_types::schematic::{FillType, Point, SchDrawing};
use uuid::Uuid;

/// More drawings than this is not a schematic graphic; the import is refused.
pub const MAX_IMPORTED_DRAWINGS: usize = 50_000;

/// Why a DXF file gave no drawings.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GraphicsImportError {
    #[error("line {line} is not a DXF group code and value pair")]
    Malformed { line: usize },
    #[error("a {entity} entity is missing its {what}")]
    Incomplete { entity: String, what: &'static str },
    #[error("the drawing units (code {0}) are not supported")]
    UnsupportedUnits(i64),
    #[error("the file has no lines, circles, arcs or polylines")]
    NothingToImport,
    #[error("the file has more than {} drawings", MAX_IMPORTED_DRAWINGS)]
    TooManyDrawings,
}

/// What an import found.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedGraphics {
    /// In sheet coordinates (mm, Y down), each with a fresh id.
    pub drawings: Vec<SchDrawing>,
    /// Entities of a kind the import does not read (text, blocks, hatches,
    /// splines, ...) or with no extent (a zero radius, a one-point polyline).
    pub skipped: usize,
}

/// One group code and its value.
type Pair<'a> = (i64, &'a str);

/// The drawings of the DXF text `text`, drawn `width_mm` wide.
pub fn drawings_from_dxf(
    text: &str,
    width_mm: f64,
) -> Result<ImportedGraphics, GraphicsImportError> {
    let pairs = pairs(text)?;
    let mut reader = EntityReader {
        scale: unit_scale(&pairs)?,
        width_mm,
        drawings: Vec::new(),
        skipped: 0,
        polyline: None,
    };
    for entity in entities(&pairs) {
        reader.read(entity)?;
        if reader.drawings.len() > MAX_IMPORTED_DRAWINGS {
            return Err(GraphicsImportError::TooManyDrawings);
        }
    }
    // A POLYLINE with no SEQEND still keeps the points it had.
    reader.finish_polyline();
    if reader.drawings.is_empty() {
        return Err(GraphicsImportError::NothingToImport);
    }
    Ok(ImportedGraphics {
        drawings: reader.drawings,
        skipped: reader.skipped,
    })
}

/// The file as group code / value pairs, one per two lines.
fn pairs(text: &str) -> Result<Vec<Pair<'_>>, GraphicsImportError> {
    let mut out = Vec::new();
    let mut lines = text.lines().enumerate();
    while let Some((index, code)) = lines.next() {
        let code = code.trim();
        let Some((_, value)) = lines.next() else {
            // A blank line after the last pair ends the file.
            if code.is_empty() {
                break;
            }
            return Err(GraphicsImportError::Malformed { line: index + 1 });
        };
        let code = code
            .parse::<i64>()
            .map_err(|_| GraphicsImportError::Malformed { line: index + 1 })?;
        out.push((code, value.trim()));
    }
    Ok(out)
}

/// Millimetres per drawing unit, from the header's `$INSUNITS`.
fn unit_scale(pairs: &[Pair<'_>]) -> Result<f64, GraphicsImportError> {
    let units = pairs
        .windows(2)
        .find(|w| w[0] == (9, "$INSUNITS"))
        .and_then(|w| {
            if w[1].0 == 70 {
                w[1].1.parse::<i64>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    match units {
        // No units given: read as millimetres.
        0 | 4 => Ok(1.0),
        1 => Ok(25.4),
        2 => Ok(304.8),
        5 => Ok(10.0),
        6 => Ok(1000.0),
        9 => Ok(0.0254),
        13 => Ok(0.001),
        14 => Ok(100.0),
        other => Err(GraphicsImportError::UnsupportedUnits(other)),
    }
}

/// The entities of the ENTITIES section, each starting at its code-0 pair.
fn entities<'p, 'a>(pairs: &'p [Pair<'a>]) -> Vec<&'p [Pair<'a>]> {
    let Some(start) = pairs
        .windows(2)
        .position(|w| w[0] == (0, "SECTION") && w[1] == (2, "ENTITIES"))
    else {
        return Vec::new();
    };
    let body = &pairs[start + 2..];
    let end = body
        .iter()
        .position(|p| *p == (0, "ENDSEC"))
        .unwrap_or(body.len());
    let body = &body[..end];
    let mut out = Vec::new();
    let mut from = 0;
    for (i, pair) in body.iter().enumerate().skip(1) {
        if pair.0 == 0 {
            out.push(&body[from..i]);
            from = i;
        }
    }
    if from < body.len() {
        out.push(&body[from..]);
    }
    out
}

struct EntityReader {
    scale: f64,
    width_mm: f64,
    drawings: Vec<SchDrawing>,
    skipped: usize,
    /// A POLYLINE being filled by its VERTEX rows, whether it is closed, and
    /// its plane's X sign (see [`Fields::plane_x`]).
    polyline: Option<(Vec<Point>, bool, f64)>,
}

impl EntityReader {
    fn read(&mut self, entity: &[Pair<'_>]) -> Result<(), GraphicsImportError> {
        let Some(&(0, kind)) = entity.first() else {
            return Ok(());
        };
        let fields = Fields {
            kind,
            pairs: entity,
        };
        match kind {
            "LINE" => {
                let start = self.point(fields.number(10)?, fields.number(20)?);
                let end = self.point(fields.number(11)?, fields.number(21)?);
                self.drawings.push(SchDrawing::Line {
                    uuid: Uuid::now_v7(),
                    start,
                    end,
                    width: self.width_mm,
                    stroke_color: None,
                    locked: false,
                });
            }
            "CIRCLE" => {
                let sign = fields.plane_x();
                let center = self.point(sign * fields.number(10)?, fields.number(20)?);
                let radius = fields.number(40)? * self.scale;
                self.push_circle(center, radius);
            }
            "ARC" => self.read_arc(&fields)?,
            "LWPOLYLINE" => {
                let sign = fields.plane_x();
                let xs = fields.all(10)?;
                let ys = fields.all(20)?;
                let points: Vec<Point> = xs
                    .iter()
                    .zip(&ys)
                    .map(|(x, y)| self.point(sign * *x, *y))
                    .collect();
                self.push_polyline(points, fields.flags() & 1 == 1);
            }
            // A POLYLINE's points come in the VERTEX rows that follow it.
            "POLYLINE" => {
                self.finish_polyline();
                self.polyline = Some((Vec::new(), fields.flags() & 1 == 1, fields.plane_x()));
            }
            "VERTEX" => {
                let (x, y) = (fields.number(10)?, fields.number(20)?);
                let sign = self.polyline.as_ref().map_or(1.0, |(_, _, sign)| *sign);
                let at = self.point(sign * x, y);
                match self.polyline.as_mut() {
                    Some((points, _, _)) => points.push(at),
                    None => self.skipped += 1,
                }
            }
            "SEQEND" => self.finish_polyline(),
            _ => self.skipped += 1,
        }
        Ok(())
    }

    /// DXF arcs run counter-clockwise from the start angle to the end angle.
    fn read_arc(&mut self, fields: &Fields<'_, '_>) -> Result<(), GraphicsImportError> {
        let sign = fields.plane_x();
        let (cx, cy) = (fields.number(10)?, fields.number(20)?);
        let radius = fields.number(40)?;
        let from = fields.number(50)?;
        let sweep = (fields.number(51)? - from).rem_euclid(360.0);
        if radius <= 0.0 {
            self.skipped += 1;
            return Ok(());
        }
        if sweep == 0.0 {
            let center = self.point(sign * cx, cy);
            self.push_circle(center, radius * self.scale);
            return Ok(());
        }
        // Points on the arc in its own plane, then into the drawing's X.
        let on_arc = |degrees: f64| {
            let a = degrees.to_radians();
            self.point(sign * (cx + radius * a.cos()), cy + radius * a.sin())
        };
        let start = on_arc(from);
        let mid = on_arc(from + sweep / 2.0);
        let end = on_arc(from + sweep);
        self.drawings.push(SchDrawing::Arc {
            uuid: Uuid::now_v7(),
            start,
            mid,
            end,
            width: self.width_mm,
            fill: FillType::None,
            stroke_color: None,
            locked: false,
        });
        Ok(())
    }

    fn push_circle(&mut self, center: Point, radius: f64) {
        if radius <= 0.0 {
            self.skipped += 1;
            return;
        }
        self.drawings.push(SchDrawing::Circle {
            uuid: Uuid::now_v7(),
            center,
            radius,
            width: self.width_mm,
            fill: FillType::None,
            stroke_color: None,
            locked: false,
        });
    }

    fn push_polyline(&mut self, mut points: Vec<Point>, closed: bool) {
        if points.len() < 2 {
            self.skipped += 1;
            return;
        }
        if closed {
            points.push(points[0]);
        }
        self.drawings.push(SchDrawing::Polyline {
            uuid: Uuid::now_v7(),
            points,
            width: self.width_mm,
            fill: FillType::None,
            stroke_color: None,
            locked: false,
        });
    }

    /// Add the POLYLINE its VERTEX rows filled, if one is open.
    fn finish_polyline(&mut self) {
        if let Some((points, closed, _)) = self.polyline.take() {
            self.push_polyline(points, closed);
        }
    }

    /// A DXF point in sheet millimetres, Y down.
    fn point(&self, x: f64, y: f64) -> Point {
        Point::new(x * self.scale, -y * self.scale)
    }
}

/// The group codes of one entity.
struct Fields<'e, 'a> {
    kind: &'a str,
    pairs: &'e [Pair<'a>],
}

impl Fields<'_, '_> {
    fn number(&self, code: i64) -> Result<f64, GraphicsImportError> {
        let value = self
            .pairs
            .iter()
            .find(|(c, _)| *c == code)
            .ok_or_else(|| self.missing(code))?
            .1;
        self.parse(code, value)
    }

    fn all(&self, code: i64) -> Result<Vec<f64>, GraphicsImportError> {
        self.pairs
            .iter()
            .filter(|(c, _)| *c == code)
            .map(|(_, value)| self.parse(code, value))
            .collect()
    }

    fn parse(&self, code: i64, value: &str) -> Result<f64, GraphicsImportError> {
        value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| self.missing(code))
    }

    /// The X sign of the entity's own plane: -1 when its extrusion points
    /// down the Z axis (code 230 negative), as a drawing mirrored in a CAD
    /// tool often stores circles, arcs and polylines; else 1.
    fn plane_x(&self) -> f64 {
        let down = self
            .pairs
            .iter()
            .find(|(c, _)| *c == 230)
            .and_then(|(_, v)| v.parse::<f64>().ok())
            .is_some_and(|z| z < 0.0);
        if down { -1.0 } else { 1.0 }
    }

    /// The entity's flags (code 70); bit 1 is "closed" on a polyline.
    fn flags(&self) -> i64 {
        self.pairs
            .iter()
            .find(|(c, _)| *c == 70)
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0)
    }

    fn missing(&self, code: i64) -> GraphicsImportError {
        GraphicsImportError::Incomplete {
            entity: self.kind.to_string(),
            what: match code {
                10 | 20 => "first point",
                11 | 21 => "second point",
                40 => "radius",
                50 => "start angle",
                51 => "end angle",
                _ => "values",
            },
        }
    }
}

#[cfg(test)]
#[path = "graphics_import_tests.rs"]
mod tests;
