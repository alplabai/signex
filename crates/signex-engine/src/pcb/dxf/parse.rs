//! Read the drawing entities of an ASCII DXF file: lines, arcs, circles and
//! polylines (light and classic), in millimetres. Design note:
//! `docs/BOARD_IMPORT_AND_REUSE.md`, "DXF import".

use signex_types::pcb::Point;

use super::DxfError;

/// One drawing entity, in millimetres in the file's own axes (y up).
#[derive(Debug, Clone, PartialEq)]
pub enum DxfEntity {
    Line {
        a: Point,
        b: Point,
    },
    /// Counter-clockwise from `start_deg` to `end_deg`.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    /// Vertices with the bulge of the segment leaving each one (0 straight;
    /// the tangent of a quarter of the included angle, positive
    /// counter-clockwise).
    Polyline {
        vertices: Vec<(Point, f64)>,
        closed: bool,
    },
}

/// The entities of a file and how many of another kind it skipped.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DxfDrawing {
    pub entities: Vec<DxfEntity>,
    /// Entities of a kind this reader does not draw (text, blocks, hatches).
    pub skipped: usize,
}

/// One group: a code and its value line.
struct Group<'a> {
    code: i32,
    value: &'a str,
}

/// The drawing in `text`, scaled to mm by the header's `$INSUNITS`.
pub fn parse_dxf(text: &str) -> Result<DxfDrawing, DxfError> {
    let groups = groups(text)?;
    let scale = units_to_mm(&groups)?;
    let mut drawing = DxfDrawing::default();
    let mut i = match entities_start(&groups) {
        Some(i) => i,
        None => return Err(DxfError::NoEntities),
    };
    while i < groups.len() {
        let g = &groups[i];
        if g.code != 0 {
            i += 1;
            continue;
        }
        if g.value == "ENDSEC" || g.value == "EOF" {
            break;
        }
        let end = next_entity(&groups, i + 1);
        let fields = &groups[i + 1..end];
        match g.value {
            "LINE" => drawing.entities.push(line(fields, scale)?),
            "ARC" => drawing.entities.push(arc(fields, scale)?),
            "CIRCLE" => drawing.entities.push(circle(fields, scale)?),
            "LWPOLYLINE" => drawing.entities.push(lw_polyline(fields, scale)?),
            "POLYLINE" => {
                let (entity, after) = polyline(&groups, i, scale)?;
                drawing.entities.push(entity);
                i = after;
                continue;
            }
            _ => drawing.skipped += 1,
        }
        i = end;
    }
    Ok(drawing)
}

fn groups(text: &str) -> Result<Vec<Group<'_>>, DxfError> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    if lines.len() < 2 {
        return Err(DxfError::NotDxf);
    }
    lines
        .chunks(2)
        .enumerate()
        .filter(|(_, pair)| pair.len() == 2)
        .map(|(n, pair)| {
            let code = pair[0]
                .parse::<i32>()
                .map_err(|_| DxfError::BadGroupCode { line: n * 2 + 1 })?;
            Ok(Group {
                code,
                value: pair[1],
            })
        })
        .collect()
}

/// The factor from the file's `$INSUNITS` to mm; a file without one, or
/// unitless, is read as mm.
fn units_to_mm(groups: &[Group<'_>]) -> Result<f64, DxfError> {
    let Some(at) = groups
        .iter()
        .position(|g| g.code == 9 && g.value == "$INSUNITS")
    else {
        return Ok(1.0);
    };
    let Some(units) = groups.get(at + 1).filter(|g| g.code == 70) else {
        return Ok(1.0);
    };
    let code: i32 = units
        .value
        .parse()
        .map_err(|_| DxfError::UnsupportedUnits(units.value.to_string()))?;
    Ok(match code {
        0 | 4 => 1.0,
        1 => 25.4,
        2 => 304.8,
        5 => 10.0,
        6 => 1000.0,
        8 => 25.4e-6,
        9 => 0.0254,
        13 => 1.0e-3,
        14 => 100.0,
        _ => return Err(DxfError::UnsupportedUnits(code.to_string())),
    })
}

/// The index just past `0 / SECTION, 2 / ENTITIES`.
fn entities_start(groups: &[Group<'_>]) -> Option<usize> {
    groups
        .windows(2)
        .position(|w| w[0].code == 0 && w[0].value == "SECTION" && w[1].value == "ENTITIES")
        .map(|i| i + 2)
}

fn next_entity(groups: &[Group<'_>], from: usize) -> usize {
    groups[from..]
        .iter()
        .position(|g| g.code == 0)
        .map_or(groups.len(), |n| from + n)
}

fn number(fields: &[Group<'_>], code: i32) -> Result<Option<f64>, DxfError> {
    match fields.iter().find(|g| g.code == code) {
        None => Ok(None),
        Some(g) => finite(g).map(Some),
    }
}

fn finite(g: &Group<'_>) -> Result<f64, DxfError> {
    g.value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| DxfError::BadNumber(g.value.to_string()))
}

fn required(fields: &[Group<'_>], code: i32, entity: &'static str) -> Result<f64, DxfError> {
    number(fields, code)?.ok_or(DxfError::MissingValue { entity, code })
}

fn point(
    fields: &[Group<'_>],
    x: i32,
    entity: &'static str,
    scale: f64,
) -> Result<Point, DxfError> {
    Ok(Point::new(
        required(fields, x, entity)? * scale,
        required(fields, x + 10, entity)? * scale,
    ))
}

fn line(fields: &[Group<'_>], scale: f64) -> Result<DxfEntity, DxfError> {
    Ok(DxfEntity::Line {
        a: point(fields, 10, "LINE", scale)?,
        b: point(fields, 11, "LINE", scale)?,
    })
}

fn arc(fields: &[Group<'_>], scale: f64) -> Result<DxfEntity, DxfError> {
    Ok(DxfEntity::Arc {
        center: point(fields, 10, "ARC", scale)?,
        radius: required(fields, 40, "ARC")? * scale,
        start_deg: required(fields, 50, "ARC")?,
        end_deg: required(fields, 51, "ARC")?,
    })
}

fn circle(fields: &[Group<'_>], scale: f64) -> Result<DxfEntity, DxfError> {
    Ok(DxfEntity::Circle {
        center: point(fields, 10, "CIRCLE", scale)?,
        radius: required(fields, 40, "CIRCLE")? * scale,
    })
}

fn closed_flag(fields: &[Group<'_>]) -> bool {
    fields
        .iter()
        .find(|g| g.code == 70)
        .and_then(|g| g.value.parse::<i32>().ok())
        .is_some_and(|flags| flags & 1 == 1)
}

/// A light polyline: each `10` starts a vertex, its `20` and `42` follow.
fn lw_polyline(fields: &[Group<'_>], scale: f64) -> Result<DxfEntity, DxfError> {
    let mut vertices: Vec<(Point, f64)> = Vec::new();
    let mut x: Option<f64> = None;
    for g in fields {
        match g.code {
            10 => x = Some(finite(g)? * scale),
            20 => {
                let x = x.take().ok_or(DxfError::MissingValue {
                    entity: "LWPOLYLINE",
                    code: 10,
                })?;
                vertices.push((Point::new(x, finite(g)? * scale), 0.0));
            }
            42 => {
                if let Some(last) = vertices.last_mut() {
                    last.1 = finite(g)?;
                }
            }
            _ => {}
        }
    }
    Ok(DxfEntity::Polyline {
        vertices,
        closed: closed_flag(fields),
    })
}

/// A classic polyline: its `VERTEX` entities follow it up to `SEQEND`.
/// Returns the polyline and the index after its `SEQEND`.
fn polyline(groups: &[Group<'_>], at: usize, scale: f64) -> Result<(DxfEntity, usize), DxfError> {
    let head_end = next_entity(groups, at + 1);
    let closed = closed_flag(&groups[at + 1..head_end]);
    let mut vertices = Vec::new();
    let mut i = head_end;
    while i < groups.len() && groups[i].value == "VERTEX" {
        let end = next_entity(groups, i + 1);
        let fields = &groups[i + 1..end];
        let p = point(fields, 10, "VERTEX", scale)?;
        vertices.push((p, number(fields, 42)?.unwrap_or(0.0)));
        i = end;
    }
    if i < groups.len() && groups[i].value == "SEQEND" {
        i = next_entity(groups, i + 1);
    }
    Ok((DxfEntity::Polyline { vertices, closed }, i))
}
