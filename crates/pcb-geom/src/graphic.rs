//! Where drawings land on the board (#2387): a footprint's silkscreen, fab,
//! assembly and courtyard drawings, placed like its pads (mirror on the
//! bottom side, then turn, then move), and the board's own drawings and
//! texts, which are in board coordinates already. Rules:
//! `docs/RENDERING_RULES.md`, "pcb-graphics".

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{BoardGraphic, BoardText, Footprint, FpGraphic, Point};
use signex_types::pcb_layer::BoardSide;

use crate::text::{TextPlacement, place_text};
use crate::transform::{Placement, Side};
use crate::units::{GeomError, PointNm, mm_to_nm};

/// The `fill` a filled drawing carries.
const SOLID_FILL: &str = "solid";

/// A drawing's geometry on the board.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphicShape {
    Line {
        a: PointNm,
        b: PointNm,
    },
    /// From `start_deg` through `sweep_deg` about `center`; angles are
    /// degrees in the model's axes, the sense a pad turns. A negative sweep
    /// runs the other way.
    Arc {
        center: PointNm,
        radius: i64,
        start_deg: f64,
        sweep_deg: f64,
    },
    Circle {
        center: PointNm,
        radius: i64,
    },
    /// A closed outline: a rectangle's four corners or a polygon's points.
    Polygon {
        points: Vec<PointNm>,
    },
    Text(TextPlacement),
}

/// A drawing placed on the board.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGraphic {
    pub shape: GraphicShape,
    /// The stroke width as saved; 0 when it gives none.
    pub width: i64,
    /// Drawn filled: a circle as a disc, a rectangle or polygon as an area.
    pub filled: bool,
}

/// Where `graphic` of `footprint` lands. `Ok(None)` for a drawing of a kind
/// this build does not know or that lacks the points its kind needs.
/// Refuses a non-finite or out-of-range value.
pub fn footprint_graphic_placement(
    footprint: &Footprint,
    graphic: &FpGraphic,
) -> Result<Option<PlacedGraphic>, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    if graphic.graphic_type == "text" {
        let Some(position) = graphic.position else {
            return Ok(None);
        };
        if !graphic.rotation.is_finite() {
            return Err(GeomError::Unrepresentable("graphic.rotation"));
        }
        let local = PointNm::from_mm(position, "graphic.position")?;
        return Ok(Some(PlacedGraphic {
            shape: GraphicShape::Text(place_text(&placement, local, graphic.rotation)),
            width: 0,
            filled: false,
        }));
    }
    let source = Source {
        kind: &graphic.graphic_type,
        start: graphic.start,
        mid: graphic.mid,
        end: graphic.end,
        center: graphic.center,
        radius: graphic.radius,
        points: &graphic.points,
    };
    place(
        &placement,
        &source,
        graphic.width,
        graphic.fill == SOLID_FILL,
    )
}

/// Where the board drawing `graphic` lands: where it is saved.
pub fn board_graphic_placement(graphic: &BoardGraphic) -> Result<Option<PlacedGraphic>, GeomError> {
    let source = Source {
        kind: &graphic.graphic_type,
        start: graphic.start,
        mid: None,
        end: graphic.end,
        center: graphic.center,
        radius: graphic.radius,
        points: &graphic.points,
    };
    place(&BOARD_FRAME, &source, graphic.width, false)
}

/// Where the board text `text` lands: at its position, turned by its
/// rotation, mirrored on a bottom-side layer, which is read from below.
pub fn board_text_placement(text: &BoardText) -> Result<TextPlacement, GeomError> {
    if !text.rotation.is_finite() {
        return Err(GeomError::Unrepresentable("text.rotation"));
    }
    let side = parse_pcb_layer(&text.layer).and_then(|layer| layer.side());
    Ok(TextPlacement {
        anchor: PointNm::from_mm(text.position, "text.position")?,
        rotation_deg: text.rotation,
        mirrored: matches!(side, Some(BoardSide::Bottom)),
    })
}

/// The board's own frame: a board drawing is placed where it is saved.
const BOARD_FRAME: Placement = Placement {
    origin: PointNm::ORIGIN,
    rotation_deg: 0.0,
    side: Side::Top,
};

/// The fields one drawing kind reads, from a footprint or a board drawing.
struct Source<'a> {
    kind: &'a str,
    start: Option<Point>,
    mid: Option<Point>,
    end: Option<Point>,
    center: Option<Point>,
    radius: f64,
    points: &'a [Point],
}

fn place(
    placement: &Placement,
    source: &Source<'_>,
    width: f64,
    filled: bool,
) -> Result<Option<PlacedGraphic>, GeomError> {
    let world = |p: Point, field: &'static str| {
        PointNm::from_mm(p, field).map(|local| placement.to_world(local))
    };
    let shape = match source.kind {
        "line" => {
            let (Some(start), Some(end)) = (source.start, source.end) else {
                return Ok(None);
            };
            GraphicShape::Line {
                a: world(start, "graphic.start")?,
                b: world(end, "graphic.end")?,
            }
        }
        "rect" => {
            let (Some(s), Some(e)) = (source.start, source.end) else {
                return Ok(None);
            };
            let corners = [s, Point::new(e.x, s.y), e, Point::new(s.x, e.y)];
            GraphicShape::Polygon {
                points: corners
                    .into_iter()
                    .map(|p| world(p, "graphic.corner"))
                    .collect::<Result<Vec<_>, GeomError>>()?,
            }
        }
        "poly" => {
            if source.points.len() < 2 {
                return Ok(None);
            }
            GraphicShape::Polygon {
                points: source
                    .points
                    .iter()
                    .map(|&p| world(p, "graphic.points"))
                    .collect::<Result<Vec<_>, GeomError>>()?,
            }
        }
        "circle" => {
            let Some(center) = source.center else {
                return Ok(None);
            };
            let rim = source.end.or(source.start);
            let radius = radius_of(center, source.radius, rim)?;
            GraphicShape::Circle {
                center: world(center, "graphic.center")?,
                radius,
            }
        }
        "arc" => match arc(placement, source)? {
            Some(shape) => shape,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(Some(PlacedGraphic {
        shape,
        width: mm_to_nm(width, "graphic.width")?.abs(),
        filled,
    }))
}

/// An arc about its centre from its start to its end. With a middle point
/// it runs the way that passes the middle; without one it runs the way of
/// increasing angle. The mirror of a bottom-side footprint reverses it.
fn arc(placement: &Placement, source: &Source<'_>) -> Result<Option<GraphicShape>, GeomError> {
    let (Some(center), Some(start), Some(end)) = (source.center, source.start, source.end) else {
        return Ok(None);
    };
    let angle = |p: Point| (p.y - center.y).atan2(p.x - center.x).to_degrees();
    let (a0, a1) = (angle(start), angle(end));
    let forward = (a1 - a0).rem_euclid(360.0);
    let local_sweep = match source.mid {
        Some(mid) if (angle(mid) - a0).rem_euclid(360.0) > forward => forward - 360.0,
        _ if forward == 0.0 => 360.0,
        _ => forward,
    };
    if !local_sweep.is_finite() {
        return Err(GeomError::Unrepresentable("graphic.arc"));
    }
    let radius = radius_of(center, source.radius, Some(start))?;
    let world_center = placement.to_world(PointNm::from_mm(center, "graphic.center")?);
    let world_start = placement.to_world(PointNm::from_mm(start, "graphic.start")?);
    let from = world_start.minus(world_center);
    let sweep_deg = match placement.side {
        Side::Top => local_sweep,
        Side::Bottom => -local_sweep,
    };
    Ok(Some(GraphicShape::Arc {
        center: world_center,
        radius,
        start_deg: (from.y as f64).atan2(from.x as f64).to_degrees(),
        sweep_deg,
    }))
}

/// A circle's or arc's radius: the distance from `center` to a point on its
/// rim when it has one, else the saved radius.
fn radius_of(center: Point, saved: f64, rim: Option<Point>) -> Result<i64, GeomError> {
    match rim {
        Some(p) => mm_to_nm((p.x - center.x).hypot(p.y - center.y), "graphic.radius"),
        None => Ok(mm_to_nm(saved, "graphic.radius")?.abs()),
    }
}

#[cfg(test)]
#[path = "graphic_tests.rs"]
mod tests;
