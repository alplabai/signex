//! Where a block diagram's lines go: block outlines by shape, port
//! anchors, the ends and right-angle path of a connection, its arrow head
//! and the point its label hangs from. Diagram mm, y down.

use signex_types::block_diagram::{Block, BlockPort, BlockShape, PortSide};
use signex_types::schematic::Point;
use uuid::Uuid;

/// Segments per quarter turn of a rounded corner.
const CORNER_STEPS: usize = 4;
/// Segments around an ellipse.
const ELLIPSE_STEPS: usize = 48;
/// A rounded corner's radius as a share of the block's shorter side.
const CORNER_SHARE: f64 = 0.15;
/// The shortest arrow head, mm; a thick line gets a longer one.
const MIN_ARROW_MM: f64 = 2.0;
/// Points closer than this are one point.
const SAME_MM: f64 = 1e-9;

/// One end of a connection: where it touches its block, and whether it
/// leaves that block sideways (a left or right edge) or up or down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LinkEnd {
    pub at: Point,
    pub sideways: bool,
}

/// The block's outline as a closed ring, by its shape.
pub(super) fn block_outline(block: &Block) -> Vec<Point> {
    let (x, y) = (block.position.x, block.position.y);
    let (w, h) = (block.width_mm, block.height_mm);
    match block.shape {
        BlockShape::Rectangle => vec![
            Point::new(x, y),
            Point::new(x + w, y),
            Point::new(x + w, y + h),
            Point::new(x, y + h),
        ],
        BlockShape::RoundedRectangle => {
            let r = w.min(h) * CORNER_SHARE;
            // Corner centres clockwise from the top right, each with the
            // angle its arc starts at (y down, so -90° points up).
            let corners = [
                (x + w - r, y + r, -90.0_f64),
                (x + w - r, y + h - r, 0.0),
                (x + r, y + h - r, 90.0),
                (x + r, y + r, 180.0),
            ];
            corners
                .into_iter()
                .flat_map(|(cx, cy, start)| {
                    (0..=CORNER_STEPS).map(move |i| {
                        let t = (start + 90.0 * i as f64 / CORNER_STEPS as f64).to_radians();
                        Point::new(cx + r * t.cos(), cy + r * t.sin())
                    })
                })
                .collect()
        }
        BlockShape::Ellipse => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            (0..ELLIPSE_STEPS)
                .map(|i| {
                    let t = std::f64::consts::TAU * i as f64 / ELLIPSE_STEPS as f64;
                    Point::new(cx + w / 2.0 * t.cos(), cy + h / 2.0 * t.sin())
                })
                .collect()
        }
        BlockShape::Hexagon => {
            let k = (h / 2.0).min(w / 4.0);
            vec![
                Point::new(x, y + h / 2.0),
                Point::new(x + k, y),
                Point::new(x + w - k, y),
                Point::new(x + w, y + h / 2.0),
                Point::new(x + w - k, y + h),
                Point::new(x + k, y + h),
            ]
        }
    }
}

/// Where `port` sits on `block`'s edge, and the unit step from there into
/// the block. An offset past the edge's end sits at that end.
pub(super) fn port_anchor(block: &Block, port: &BlockPort) -> (Point, Point) {
    let (x, y) = (block.position.x, block.position.y);
    let (w, h) = (block.width_mm, block.height_mm);
    let along = |len: f64| port.offset_mm.clamp(0.0, len);
    match port.side {
        PortSide::Left => (Point::new(x, y + along(h)), Point::new(1.0, 0.0)),
        PortSide::Right => (Point::new(x + w, y + along(h)), Point::new(-1.0, 0.0)),
        PortSide::Top => (Point::new(x + along(w), y), Point::new(0.0, 1.0)),
        PortSide::Bottom => (Point::new(x + along(w), y + h), Point::new(0.0, -1.0)),
    }
}

fn centre(block: &Block) -> Point {
    Point::new(
        block.position.x + block.width_mm / 2.0,
        block.position.y + block.height_mm / 2.0,
    )
}

/// The end of a connection on `block`: its port when it names one the
/// block has, else the middle of the edge that faces `other`.
pub(super) fn end_point(block: &Block, port: Option<Uuid>, other: &Block) -> LinkEnd {
    if let Some(port) = port.and_then(|id| block.port(id)) {
        let (at, inward) = port_anchor(block, port);
        return LinkEnd {
            at,
            sideways: inward.y == 0.0,
        };
    }
    let (c, o) = (centre(block), centre(other));
    let (dx, dy) = (o.x - c.x, o.y - c.y);
    let (half_w, half_h) = (block.width_mm / 2.0, block.height_mm / 2.0);
    if dx.abs() >= dy.abs() {
        LinkEnd {
            at: Point::new(c.x + half_w.copysign(dx), c.y),
            sideways: true,
        }
    } else {
        LinkEnd {
            at: Point::new(c.x, c.y + half_h.copysign(dy)),
            sideways: false,
        }
    }
}

/// The right-angle path from `a` to `b`: it leaves and arrives along each
/// end's own direction, turning halfway when both ends point the same way.
pub(super) fn connection_path(a: LinkEnd, b: LinkEnd) -> Vec<Point> {
    let (p, q) = (a.at, b.at);
    let points = match (a.sideways, b.sideways) {
        (true, true) => {
            let mx = (p.x + q.x) / 2.0;
            vec![p, Point::new(mx, p.y), Point::new(mx, q.y), q]
        }
        (false, false) => {
            let my = (p.y + q.y) / 2.0;
            vec![p, Point::new(p.x, my), Point::new(q.x, my), q]
        }
        (true, false) => vec![p, Point::new(q.x, p.y), q],
        (false, true) => vec![p, Point::new(p.x, q.y), q],
    };
    let mut path: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if path
            .last()
            .is_none_or(|last| distance(*last, point) > SAME_MM)
        {
            path.push(point);
        }
    }
    path
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// The filled arrow head at the end of `path`, its tip on the last point;
/// `None` for a path with no length.
pub(super) fn arrow_head(path: &[Point], width_mm: f64) -> Option<Vec<Point>> {
    let [.., from, tip] = path else {
        return None;
    };
    let len = distance(*from, *tip);
    if len <= SAME_MM {
        return None;
    }
    let (ux, uy) = ((tip.x - from.x) / len, (tip.y - from.y) / len);
    let long = (width_mm * 4.0).max(MIN_ARROW_MM).min(len);
    let half = long * 0.4;
    let base = Point::new(tip.x - ux * long, tip.y - uy * long);
    Some(vec![
        *tip,
        Point::new(base.x - uy * half, base.y + ux * half),
        Point::new(base.x + uy * half, base.y - ux * half),
    ])
}

/// Where a connection's label hangs: the middle of its longest level
/// segment, else of its longest segment.
pub(super) fn label_point(path: &[Point]) -> Point {
    let segments = path.windows(2).map(|pair| (pair[0], pair[1]));
    let longest = |level_only: bool| {
        segments
            .clone()
            .filter(|(a, b)| !level_only || (a.y - b.y).abs() <= SAME_MM)
            .max_by(|(a, b), (c, d)| distance(*a, *b).total_cmp(&distance(*c, *d)))
    };
    match longest(true).or_else(|| longest(false)) {
        Some((a, b)) => Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
        None => path.first().copied().unwrap_or(Point::new(0.0, 0.0)),
    }
}
