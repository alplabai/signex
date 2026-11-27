//! Where the parts of a block diagram sit: a block's box, a port's point on
//! its block's edge, a connection's drawn route and the point its label
//! sits at, a group's frame, and which object is under a point. Plain maths
//! over the model, in mm with y down, so the canvas and the exports draw
//! the same picture.

use uuid::Uuid;

use super::{Block, BlockDiagram, BlockPort, Connection, Endpoint, PortSide};
use crate::schematic::Point;

/// The margin a group's frame keeps around its blocks, mm.
pub const GROUP_MARGIN_MM: f64 = 5.0;

/// The gap a block placed without a position keeps from the blocks before
/// it, mm.
pub const PLACEMENT_GAP_MM: f64 = 10.0;

/// An axis-aligned box in mm: its top-left corner and its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiagramBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl DiagramBox {
    /// Whether `p` is inside the box or on its edge.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.x + self.width && p.y >= self.y && p.y <= self.y + self.height
    }

    /// The middle of the box.
    pub fn centre(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// The smallest box holding both.
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }

    /// The box grown by `margin` on every side.
    pub fn grown(self, margin: f64) -> Self {
        Self {
            x: self.x - margin,
            y: self.y - margin,
            width: self.width + 2.0 * margin,
            height: self.height + 2.0 * margin,
        }
    }
}

/// Which way a route leaves its end: across (left or right) or along
/// (up or down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

impl Block {
    /// The block's box.
    pub fn bounds(&self) -> DiagramBox {
        DiagramBox {
            x: self.position.x,
            y: self.position.y,
            width: self.width_mm,
            height: self.height_mm,
        }
    }

    /// Where `port` sits on the block's edge. An offset past the end of
    /// its edge is held at the end.
    pub fn port_point(&self, port: &BlockPort) -> Point {
        let b = self.bounds();
        let along = |length: f64| port.offset_mm.clamp(0.0, length);
        match port.side {
            PortSide::Left => Point::new(b.x, b.y + along(b.height)),
            PortSide::Right => Point::new(b.x + b.width, b.y + along(b.height)),
            PortSide::Top => Point::new(b.x + along(b.width), b.y),
            PortSide::Bottom => Point::new(b.x + along(b.width), b.y + b.height),
        }
    }

    /// The middle of the side of the block that faces `target`.
    pub fn facing_point(&self, target: Point) -> Point {
        let (side, _) = self.facing_side(target);
        let b = self.bounds();
        let c = b.centre();
        match side {
            PortSide::Left => Point::new(b.x, c.y),
            PortSide::Right => Point::new(b.x + b.width, c.y),
            PortSide::Top => Point::new(c.x, b.y),
            PortSide::Bottom => Point::new(c.x, b.y + b.height),
        }
    }

    /// The side facing `target` and the axis a route leaves it on. A
    /// target as far across as along, measured against the box's
    /// proportions, is across.
    fn facing_side(&self, target: Point) -> (PortSide, Axis) {
        let c = self.bounds().centre();
        let (dx, dy) = (target.x - c.x, target.y - c.y);
        if dx.abs() * self.height_mm >= dy.abs() * self.width_mm {
            let side = if dx < 0.0 {
                PortSide::Left
            } else {
                PortSide::Right
            };
            (side, Axis::Horizontal)
        } else {
            let side = if dy < 0.0 {
                PortSide::Top
            } else {
                PortSide::Bottom
            };
            (side, Axis::Vertical)
        }
    }

    /// The point and axis one end of a route starts on: the port's, or
    /// the side facing `toward`.
    fn end_point(&self, port: Option<&BlockPort>, toward: Point) -> (Point, Axis) {
        match port {
            Some(port) => (self.port_point(port), side_axis(port.side)),
            None => (self.facing_point(toward), self.facing_side(toward).1),
        }
    }
}

fn side_axis(side: PortSide) -> Axis {
    match side {
        PortSide::Left | PortSide::Right => Axis::Horizontal,
        PortSide::Top | PortSide::Bottom => Axis::Vertical,
    }
}

/// The distance from `p` to the segment `a`-`b`.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length2 = dx * dx + dy * dy;
    let t = if length2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (x, y) = (a.x + t * dx, a.y + t * dy);
    (p.x - x).hypot(p.y - y)
}

impl BlockDiagram {
    /// The block and port an endpoint names, when both are in the diagram
    /// (a port the endpoint does not name is `None`).
    fn end_parts(&self, end: Endpoint) -> Option<(&Block, Option<&BlockPort>)> {
        let block = self.block(end.block)?;
        let port = match end.port {
            Some(id) => Some(block.port(id)?),
            None => None,
        };
        Some((block, port))
    }

    /// The route connection `link` is drawn along: from its start to its
    /// end in right angles, leaving each end square to its side. `None`
    /// when an end is not in the diagram.
    pub fn route(&self, link: &Connection) -> Option<Vec<Point>> {
        let (from_block, from_port) = self.end_parts(link.from)?;
        let (to_block, to_port) = self.end_parts(link.to)?;
        let toward_to = to_port.map_or(to_block.bounds().centre(), |p| to_block.port_point(p));
        let toward_from =
            from_port.map_or(from_block.bounds().centre(), |p| from_block.port_point(p));
        let (a, axis) = from_block.end_point(from_port, toward_to);
        let (b, _) = to_block.end_point(to_port, toward_from);
        Some(match axis {
            Axis::Horizontal => {
                let mid = (a.x + b.x) / 2.0;
                vec![a, Point::new(mid, a.y), Point::new(mid, b.y), b]
            }
            Axis::Vertical => {
                let mid = (a.y + b.y) / 2.0;
                vec![a, Point::new(a.x, mid), Point::new(b.x, mid), b]
            }
        })
    }

    /// Where connection `link`'s label is drawn: the middle of its route.
    pub fn label_point(&self, link: &Connection) -> Option<Point> {
        let route = self.route(link)?;
        let (a, b) = (route[1], route[2]);
        Some(Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0))
    }

    /// The frame drawn around group `group`: its blocks' boxes grown by
    /// [`GROUP_MARGIN_MM`]. `None` for a group with no block.
    pub fn group_frame(&self, group: Uuid) -> Option<DiagramBox> {
        self.blocks
            .iter()
            .filter(|b| b.group == Some(group))
            .map(Block::bounds)
            .reduce(DiagramBox::union)
            .map(|frame| frame.grown(GROUP_MARGIN_MM))
    }

    /// The box holding every block and group frame; `None` when the
    /// diagram has no block.
    pub fn extent(&self) -> Option<DiagramBox> {
        let frames = self.groups.iter().filter_map(|g| self.group_frame(g.id));
        self.blocks
            .iter()
            .map(Block::bounds)
            .chain(frames)
            .reduce(DiagramBox::union)
    }

    /// The topmost block under `p`: the one drawn last.
    pub fn block_at(&self, p: Point) -> Option<Uuid> {
        self.blocks
            .iter()
            .rev()
            .find(|b| b.bounds().contains(p))
            .map(|b| b.id)
    }

    /// The port within `tolerance` mm of `p`, nearest first; a port on a
    /// block drawn later wins a tie.
    pub fn port_at(&self, p: Point, tolerance: f64) -> Option<(Uuid, Uuid)> {
        self.blocks
            .iter()
            .rev()
            .flat_map(|b| b.ports.iter().map(move |port| (b, port)))
            .map(|(b, port)| {
                let at = b.port_point(port);
                ((b.id, port.id), (p.x - at.x).hypot(p.y - at.y))
            })
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(ids, _)| ids)
    }

    /// The connection whose route passes within `tolerance` mm of `p`,
    /// nearest first; a later connection wins a tie.
    pub fn connection_at(&self, p: Point, tolerance: f64) -> Option<Uuid> {
        self.connections
            .iter()
            .rev()
            .filter_map(|link| {
                let route = self.route(link)?;
                let d = route
                    .windows(2)
                    .map(|w| distance_to_segment(p, w[0], w[1]))
                    .fold(f64::INFINITY, f64::min);
                Some((link.id, d))
            })
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(id, _)| id)
    }

    /// The group whose frame's edge passes within `tolerance` mm of `p`.
    pub fn group_at(&self, p: Point, tolerance: f64) -> Option<Uuid> {
        self.groups.iter().rev().find_map(|g| {
            let f = self.group_frame(g.id)?;
            let corners = [
                Point::new(f.x, f.y),
                Point::new(f.x + f.width, f.y),
                Point::new(f.x + f.width, f.y + f.height),
                Point::new(f.x, f.y + f.height),
            ];
            let near = (0..4)
                .any(|i| distance_to_segment(p, corners[i], corners[(i + 1) % 4]) <= tolerance);
            near.then_some(g.id)
        })
    }

    /// Where a block made without a position goes: right of every block,
    /// level with the first one; the origin in an empty diagram.
    pub fn next_free_position(&self) -> Point {
        let Some(first) = self.blocks.first() else {
            return Point::ZERO;
        };
        let right = self
            .blocks
            .iter()
            .map(|b| b.position.x + b.width_mm)
            .fold(f64::NEG_INFINITY, f64::max);
        Point::new(right + PLACEMENT_GAP_MM, first.position.y)
    }
}
