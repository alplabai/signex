//! Board drawings in the selection index (#2424): each drawing is the
//! capsules of its stroke along its outline, placed by
//! [`board_graphic_placement`], so a click picks the stroke the board draws.
//! A text frame's box is also taken from inside.

use signex_types::pcb::BoardGraphic;
use signex_types::pcb_graphic::BoardGraphicKind;

use crate::graphic::{GraphicShape, board_graphic_placement};
use crate::index::{Area, IndexedItem, ItemRef, Rejected};
use crate::pick_filter::PickKind;
use crate::rect::RectNm;
use crate::region::Polygon;
use crate::shape::Shape;
use crate::units::{GeomError, PointNm};

/// The largest turn one indexed chord of an arc or circle spans, degrees.
const CHORD_DEG: f64 = 10.0;

/// The outline a drawing is picked on: its points, and whether the last
/// joins the first.
struct Outline {
    points: Vec<PointNm>,
    closed: bool,
}

/// Index `graphic` when it is a kind the Graphics tools draw. A drawing
/// whose identity another drawing already has, or whose geometry cannot
/// convert, is listed as refused.
pub(crate) fn add_graphic(
    graphic: &BoardGraphic,
    areas: &mut Vec<Area>,
    items: &mut Vec<IndexedItem>,
    rejected: &mut Vec<Rejected>,
) {
    let Some(kind) = graphic.kind() else {
        return;
    };
    let item = ItemRef::Graphic {
        graphic: graphic.uuid,
        kind: PickKind::of_graphic(kind),
    };
    let taken = areas
        .iter()
        .any(|a| matches!(a.item, ItemRef::Graphic { graphic: id, .. } if id == graphic.uuid));
    if taken {
        rejected.push(Rejected {
            item,
            error: GeomError::DuplicateItem,
        });
        return;
    }
    let entries = outline(graphic).and_then(|found| {
        let Some((outline, radius)) = found else {
            return Ok(None);
        };
        let pieces = pieces(&outline);
        let entries = pieces
            .into_iter()
            .map(|(a, b)| IndexedItem::new(item, Shape::Capsule { a, b, radius }))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some((outline, entries)))
    });
    match entries {
        Ok(Some((outline, entries))) => {
            let Some(bounds) = RectNm::bounding(outline.points.iter().copied()) else {
                return;
            };
            areas.push(Area {
                item,
                edges: entries.len(),
                inside: kind == BoardGraphicKind::TextFrame,
                region: Polygon::new(outline.points, Vec::new()),
                bounds,
            });
            items.extend(entries);
        }
        Ok(None) => {}
        Err(error) => rejected.push(Rejected { item, error }),
    }
}

/// The pieces of `outline`: each point to the next, and back to the first
/// when it is closed.
fn pieces(outline: &Outline) -> Vec<(PointNm, PointNm)> {
    let points = &outline.points;
    let mut pieces: Vec<(PointNm, PointNm)> = points.windows(2).map(|w| (w[0], w[1])).collect();
    if outline.closed && points.len() > 2 {
        pieces.push((points[points.len() - 1], points[0]));
    }
    if points.len() == 1 {
        pieces.push((points[0], points[0]));
    }
    pieces
}

/// The outline of `graphic` and its stroke's half width; `None` for a
/// drawing missing the points its kind needs.
fn outline(graphic: &BoardGraphic) -> Result<Option<(Outline, i64)>, GeomError> {
    let Some(placed) = board_graphic_placement(graphic)? else {
        return Ok(None);
    };
    let radius = placed.width / 2;
    let outline = match placed.shape {
        GraphicShape::Line { a, b } => Outline {
            points: vec![a, b],
            closed: false,
        },
        GraphicShape::Arc {
            center,
            radius: r,
            start_deg,
            sweep_deg,
        } => Outline {
            points: arc_points(center, r, start_deg, sweep_deg),
            closed: false,
        },
        GraphicShape::Circle { center, radius: r } => {
            let mut points = arc_points(center, r, 0.0, 360.0);
            points.pop();
            Outline {
                points,
                closed: true,
            }
        }
        GraphicShape::Polygon { points } => Outline {
            points,
            closed: true,
        },
        GraphicShape::Text(_) => return Ok(None),
    };
    Ok(Some((outline, radius)))
}

/// Points along an arc, at most [`CHORD_DEG`] apart, both ends included.
fn arc_points(center: PointNm, radius: i64, start_deg: f64, sweep_deg: f64) -> Vec<PointNm> {
    let chords = ((sweep_deg.abs() / CHORD_DEG).ceil() as usize).clamp(1, 72);
    (0..=chords)
        .map(|i| {
            let t = (start_deg + sweep_deg * i as f64 / chords as f64).to_radians();
            PointNm::new(
                center.x + (radius as f64 * t.cos()).round() as i64,
                center.y + (radius as f64 * t.sin()).round() as i64,
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "graphic_index_tests.rs"]
mod tests;
