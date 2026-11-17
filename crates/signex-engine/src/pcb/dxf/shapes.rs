//! DXF entities as board drawings or as a board outline. The file's y grows
//! up and its origin is the board origin, so a point lands where the
//! Properties panel would show it (`BoardOrigin::board_x` / `board_y`). The
//! flip turns a counter-clockwise arc of the file clockwise in the model,
//! where a board arc runs the way of increasing angle: its ends swap.

use signex_types::object_id::IdSource;
use signex_types::pcb::{BoardGraphic, PCB_DEFAULT_GRAPHIC_WIDTH_MM, Point};
use signex_types::pcb_options::BoardOrigin;

use super::parse::{DxfDrawing, DxfEntity};

/// Two file points closer than this (mm) are one: a chain of lines closes.
const JOIN_MM: f64 = 1.0e-4;
/// The largest turn one outline chord covers, as the board draws arcs.
const CHORD_DEG: f64 = 6.0;

/// Every entity of `drawing` as a drawing on `layer`, each under a new id
/// from `ids`.
pub fn board_graphics(
    drawing: &DxfDrawing,
    origin: BoardOrigin,
    layer: &str,
    ids: &mut dyn IdSource,
) -> Vec<BoardGraphic> {
    let at = |p: Point| Point::new(origin.board_x(p.x), origin.board_y(p.y));
    let base = || BoardGraphic {
        // The id is set once the drawing is made, below.
        uuid: uuid::Uuid::nil(),
        graphic_type: String::new(),
        layer: layer.to_string(),
        width: PCB_DEFAULT_GRAPHIC_WIDTH_MM,
        start: None,
        end: None,
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    };
    let line = |a: Point, b: Point| BoardGraphic {
        graphic_type: "line".into(),
        start: Some(at(a)),
        end: Some(at(b)),
        ..base()
    };
    // A counter-clockwise arc of the file from `from` to `to` about `center`.
    let arc = |center: Point, radius: f64, from: Point, to: Point| BoardGraphic {
        graphic_type: "arc".into(),
        center: Some(at(center)),
        radius,
        start: Some(at(to)),
        end: Some(at(from)),
        ..base()
    };
    let mut out = Vec::new();
    for entity in &drawing.entities {
        match entity {
            DxfEntity::Line { a, b } => out.push(line(*a, *b)),
            DxfEntity::Circle { center, radius } => out.push(BoardGraphic {
                graphic_type: "circle".into(),
                center: Some(at(*center)),
                radius: *radius,
                ..base()
            }),
            DxfEntity::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => out.push(arc(
                *center,
                *radius,
                on_circle(*center, *radius, *start_deg),
                on_circle(*center, *radius, *end_deg),
            )),
            DxfEntity::Polyline { vertices, closed } => {
                let straight = vertices.iter().all(|(_, bulge)| *bulge == 0.0);
                if *closed && straight && vertices.len() >= 3 {
                    out.push(BoardGraphic {
                        graphic_type: "poly".into(),
                        points: vertices.iter().map(|(p, _)| at(*p)).collect(),
                        ..base()
                    });
                    continue;
                }
                for (a, b, bulge) in polyline_segments(vertices, *closed) {
                    match bulge_arc(a, b, bulge) {
                        Some((center, radius, from, to)) => out.push(arc(center, radius, from, to)),
                        None => out.push(line(a, b)),
                    }
                }
            }
        }
    }
    for graphic in &mut out {
        graphic.uuid = ids.next_id();
    }
    out
}

/// The largest closed shape of `drawing` (by area), as board outline corners:
/// a closed polyline, a circle, or lines, arcs and open polylines that join
/// end to end. Arcs become chords. `None` when nothing closes.
pub fn board_outline(drawing: &DxfDrawing, origin: BoardOrigin) -> Option<Vec<Point>> {
    let mut closed: Vec<Vec<Point>> = Vec::new();
    let mut open: Vec<Vec<Point>> = Vec::new();
    for entity in &drawing.entities {
        match entity {
            DxfEntity::Line { a, b } => open.push(vec![*a, *b]),
            DxfEntity::Circle { center, radius } => {
                closed.push(arc_points(*center, *radius, 0.0, 360.0));
            }
            DxfEntity::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let sweep = (end_deg - start_deg).rem_euclid(360.0);
                let sweep = if sweep == 0.0 { 360.0 } else { sweep };
                open.push(arc_points(*center, *radius, *start_deg, sweep));
            }
            DxfEntity::Polyline {
                vertices,
                closed: c,
            } => {
                let path = flatten_polyline(vertices, *c);
                if *c {
                    closed.push(path);
                } else {
                    open.push(path);
                }
            }
        }
    }
    closed.extend(join_chains(open));
    let best = closed
        .into_iter()
        .map(|mut ring| {
            if ring.len() > 1 && same(ring[0], ring[ring.len() - 1]) {
                ring.pop();
            }
            ring
        })
        .filter(|ring| ring.len() >= 3)
        .max_by(|a, b| area(a).total_cmp(&area(b)))?;
    Some(
        best.into_iter()
            .map(|p| Point::new(origin.board_x(p.x), origin.board_y(p.y)))
            .collect(),
    )
}

fn on_circle(center: Point, radius: f64, deg: f64) -> Point {
    let r = deg.to_radians();
    Point::new(center.x + radius * r.cos(), center.y + radius * r.sin())
}

fn same(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= JOIN_MM && (a.y - b.y).abs() <= JOIN_MM
}

/// Each segment of a polyline: its ends and bulge, the closing one too.
fn polyline_segments(vertices: &[(Point, f64)], closed: bool) -> Vec<(Point, Point, f64)> {
    let n = vertices.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    (0..count)
        .map(|i| (vertices[i].0, vertices[(i + 1) % n].0, vertices[i].1))
        .filter(|(a, b, _)| !same(*a, *b))
        .collect()
}

/// The counter-clockwise arc a bulged segment from `a` to `b` is: its
/// centre, radius and ends in counter-clockwise order. `None` for a
/// straight segment.
fn bulge_arc(a: Point, b: Point, bulge: f64) -> Option<(Point, f64, Point, Point)> {
    if bulge == 0.0 {
        return None;
    }
    let included = 4.0 * bulge.atan();
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let chord = dx.hypot(dy);
    let half = chord / 2.0;
    // Signed distance from the chord's middle to the centre, to its left.
    let h = half / (included / 2.0).tan();
    let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let center = Point::new(mid.x - dy / chord * h, mid.y + dx / chord * h);
    let radius = (half / (included / 2.0).sin()).abs();
    if !center.x.is_finite() || !center.y.is_finite() || !radius.is_finite() {
        return None;
    }
    Some(if bulge > 0.0 {
        (center, radius, a, b)
    } else {
        (center, radius, b, a)
    })
}

/// Points along a counter-clockwise arc, both ends included.
fn arc_points(center: Point, radius: f64, start_deg: f64, sweep_deg: f64) -> Vec<Point> {
    let steps = ((sweep_deg.abs() / CHORD_DEG).ceil() as usize).max(1);
    (0..=steps)
        .map(|i| {
            on_circle(
                center,
                radius,
                start_deg + sweep_deg * i as f64 / steps as f64,
            )
        })
        .collect()
}

fn flatten_polyline(vertices: &[(Point, f64)], closed: bool) -> Vec<Point> {
    let mut path: Vec<Point> = Vec::new();
    for (a, b, bulge) in polyline_segments(vertices, closed) {
        let piece = match bulge_arc(a, b, bulge) {
            Some((center, radius, from, to)) => {
                let start = (from.y - center.y).atan2(from.x - center.x).to_degrees();
                let end = (to.y - center.y).atan2(to.x - center.x).to_degrees();
                let mut points = arc_points(center, radius, start, (end - start).rem_euclid(360.0));
                if bulge < 0.0 {
                    points.reverse();
                }
                points
            }
            None => vec![a, b],
        };
        append(&mut path, piece);
    }
    if path.is_empty() {
        path = vertices.iter().map(|(p, _)| *p).collect();
    }
    path
}

/// Add `piece` to `path`, dropping its first point when it repeats the end.
fn append(path: &mut Vec<Point>, piece: Vec<Point>) {
    let skip = path
        .last()
        .zip(piece.first())
        .is_some_and(|(l, f)| same(*l, *f));
    path.extend(piece.into_iter().skip(usize::from(skip)));
}

/// Open paths joined end to end; each chain that comes back to its start is
/// a closed ring.
fn join_chains(mut open: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
    let mut rings = Vec::new();
    while let Some(mut chain) = open.pop() {
        while let (Some(&first), Some(&last)) = (chain.first(), chain.last()) {
            if chain.len() > 2 && same(first, last) {
                rings.push(chain);
                break;
            }
            let next = open.iter().position(|p| {
                p.first().is_some_and(|s| same(*s, last))
                    || p.last().is_some_and(|e| same(*e, last))
            });
            let Some(i) = next else { break };
            let mut piece = open.swap_remove(i);
            if !piece.first().is_some_and(|s| same(*s, last)) {
                piece.reverse();
            }
            append(&mut chain, piece);
        }
    }
    rings
}

/// The shoelace area of a ring, unsigned.
fn area(ring: &[Point]) -> f64 {
    let n = ring.len();
    let twice: f64 = (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    (twice / 2.0).abs()
}
