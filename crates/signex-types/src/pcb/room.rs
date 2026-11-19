//! Rooms (#2457): named areas of a board that hold the parts of one
//! schematic sheet placement, or of one channel of a sheet placed several
//! times. The model, membership and the palette live here; the edits and
//! the planner are `signex_engine::pcb::rooms`, the rule
//! `signex_drc::rooms`. See `docs/PCB_ROOMS.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Footprint, Point};
use crate::format::UnknownFields;
use crate::schematic::InstancePath;

/// Room colours, assigned per sheet in creation order and stored in the
/// room, so a room keeps its colour when others come and go.
pub const ROOM_PALETTE: [&str; 8] = [
    "#2CA8B8", "#C8A02C", "#A058C8", "#409C58", "#C8585C", "#4C78C8", "#C8782C", "#8C8C3C",
];

/// The margin a new room starts with, mm.
pub const ROOM_DEFAULT_MARGIN_MM: f64 = 0.5;

/// The `index`-th colour of [`ROOM_PALETTE`], wrapping round.
pub fn palette_colour(index: usize) -> &'static str {
    ROOM_PALETTE[index % ROOM_PALETTE.len()]
}

/// Whether `text` is a `#RRGGBB` colour.
pub fn is_room_colour(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// A named area of the board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub uuid: Uuid,
    pub name: String,
    /// The sheet placement whose parts the room holds; `None` for a room
    /// drawn by hand that holds no parts yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<RoomSource>,
    #[serde(default)]
    pub layer: RoomLayer,
    pub outline: RoomOutline,
    /// `#RRGGBB`.
    pub colour: String,
    /// A member part outside the outline, or inside the margin, is a DRC
    /// finding (`builtin::part_outside_room`).
    #[serde(default)]
    pub keep_parts_inside: bool,
    /// How far inside the outline the members must stay, mm.
    #[serde(default)]
    pub margin_mm: f64,
    /// A locked room refuses a move.
    #[serde(default)]
    pub locked: bool,
    /// Keys of its `[[rooms]]` entry this build does not know.
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
}

/// The sheet placement a room was made for. A sheet placed several times
/// has one room per placement; `channel` numbers them from 1.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomSource {
    pub sheet: InstancePath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<u32>,
}

/// Which side of the board a room is for: the side view that shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomLayer {
    #[default]
    Top,
    Bottom,
    Both,
}

impl RoomLayer {
    pub const ALL: [RoomLayer; 3] = [RoomLayer::Top, RoomLayer::Bottom, RoomLayer::Both];

    /// The `.snxpcb` token.
    pub const fn token(self) -> &'static str {
        match self {
            RoomLayer::Top => "top",
            RoomLayer::Bottom => "bottom",
            RoomLayer::Both => "both",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.token() == token)
    }

    /// Whether the room shows when the board is seen from the bottom
    /// (`bottom_view`) or the top.
    pub const fn shows_on(self, bottom_view: bool) -> bool {
        match self {
            RoomLayer::Top => !bottom_view,
            RoomLayer::Bottom => bottom_view,
            RoomLayer::Both => true,
        }
    }
}

/// The kind of outline, as Properties offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoomShape {
    #[default]
    Rectangle,
    Polygon,
}

/// A room's outline, board mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RoomOutline {
    /// Its least corner and size.
    Rect {
        x_mm: f64,
        y_mm: f64,
        w_mm: f64,
        h_mm: f64,
    },
    /// Its corners in order; closed.
    Polygon(Vec<Point>),
}

/// The bounding box of an outline: least corner and size, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomBounds {
    pub x_mm: f64,
    pub y_mm: f64,
    pub w_mm: f64,
    pub h_mm: f64,
}

impl RoomOutline {
    /// The rectangle with corners `a` and `b`, in either order.
    pub fn rect_between(a: Point, b: Point) -> Self {
        RoomOutline::Rect {
            x_mm: a.x.min(b.x),
            y_mm: a.y.min(b.y),
            w_mm: (a.x - b.x).abs(),
            h_mm: (a.y - b.y).abs(),
        }
    }

    pub fn shape(&self) -> RoomShape {
        match self {
            RoomOutline::Rect { .. } => RoomShape::Rectangle,
            RoomOutline::Polygon(_) => RoomShape::Polygon,
        }
    }

    /// Its corners in order.
    pub fn corners(&self) -> Vec<Point> {
        match *self {
            RoomOutline::Rect {
                x_mm,
                y_mm,
                w_mm,
                h_mm,
            } => vec![
                Point::new(x_mm, y_mm),
                Point::new(x_mm + w_mm, y_mm),
                Point::new(x_mm + w_mm, y_mm + h_mm),
                Point::new(x_mm, y_mm + h_mm),
            ],
            RoomOutline::Polygon(ref points) => points.clone(),
        }
    }

    /// The box round it; `None` for a polygon with no corner.
    pub fn bounds(&self) -> Option<RoomBounds> {
        if let RoomOutline::Rect {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
        } = *self
        {
            return Some(RoomBounds {
                x_mm,
                y_mm,
                w_mm,
                h_mm,
            });
        }
        let corners = self.corners();
        let first = corners.first()?;
        let (mut lo, mut hi) = (*first, *first);
        for p in &corners {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        Some(RoomBounds {
            x_mm: lo.x,
            y_mm: lo.y,
            w_mm: hi.x - lo.x,
            h_mm: hi.y - lo.y,
        })
    }

    /// The outline moved by `(dx, dy)` mm.
    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        match *self {
            RoomOutline::Rect {
                x_mm,
                y_mm,
                w_mm,
                h_mm,
            } => RoomOutline::Rect {
                x_mm: x_mm + dx,
                y_mm: y_mm + dy,
                w_mm,
                h_mm,
            },
            RoomOutline::Polygon(ref points) => RoomOutline::Polygon(
                points
                    .iter()
                    .map(|p| Point::new(p.x + dx, p.y + dy))
                    .collect(),
            ),
        }
    }

    /// The outline moved so its box starts at `(x, y)`.
    pub fn moved_to(&self, x: f64, y: f64) -> Self {
        match self.bounds() {
            Some(b) => self.translated(x - b.x_mm, y - b.y_mm),
            None => self.clone(),
        }
    }

    /// The outline stretched from its box's least corner so the box is
    /// `w` by `h`. A polygon with a flat box keeps that side.
    pub fn resized(&self, w: f64, h: f64) -> Self {
        let Some(b) = self.bounds() else {
            return self.clone();
        };
        match self {
            RoomOutline::Rect { .. } => RoomOutline::Rect {
                x_mm: b.x_mm,
                y_mm: b.y_mm,
                w_mm: w,
                h_mm: h,
            },
            RoomOutline::Polygon(points) => {
                let sx = if b.w_mm > 0.0 { w / b.w_mm } else { 1.0 };
                let sy = if b.h_mm > 0.0 { h / b.h_mm } else { 1.0 };
                RoomOutline::Polygon(
                    points
                        .iter()
                        .map(|p| {
                            Point::new(b.x_mm + (p.x - b.x_mm) * sx, b.y_mm + (p.y - b.y_mm) * sy)
                        })
                        .collect(),
                )
            }
        }
    }

    /// The outline as `shape`: a rectangle becomes the polygon of its
    /// corners, a polygon the rectangle round it.
    pub fn as_shape(&self, shape: RoomShape) -> Self {
        match (self, shape) {
            (RoomOutline::Rect { .. }, RoomShape::Polygon) => RoomOutline::Polygon(self.corners()),
            (RoomOutline::Polygon(_), RoomShape::Rectangle) => match self.bounds() {
                Some(b) => RoomOutline::Rect {
                    x_mm: b.x_mm,
                    y_mm: b.y_mm,
                    w_mm: b.w_mm,
                    h_mm: b.h_mm,
                },
                None => self.clone(),
            },
            _ => self.clone(),
        }
    }

    /// Whether every number is finite.
    pub fn is_finite(&self) -> bool {
        self.corners()
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
    }

    /// Whether it encloses an area: a rectangle with both sides above zero,
    /// a polygon with three corners or more.
    pub fn has_area(&self) -> bool {
        match self {
            RoomOutline::Rect { w_mm, h_mm, .. } => *w_mm > 0.0 && *h_mm > 0.0,
            RoomOutline::Polygon(points) => points.len() >= 3,
        }
    }
}

impl Room {
    /// A room drawn by hand: no source sheet, the default margin.
    pub fn drawn(uuid: Uuid, name: String, colour: &str, outline: RoomOutline) -> Self {
        Room {
            uuid,
            name,
            source: None,
            layer: RoomLayer::Top,
            outline,
            colour: colour.to_string(),
            keep_parts_inside: false,
            margin_mm: ROOM_DEFAULT_MARGIN_MM,
            locked: false,
            unknown: UnknownFields::default(),
        }
    }

    /// Whether `footprint` is a member: linked to a part on the room's
    /// sheet placement or a sheet below it. A room without a source holds
    /// none.
    pub fn holds(&self, footprint: &Footprint) -> bool {
        let (Some(source), Some(link)) = (&self.source, &footprint.schematic_link) else {
            return false;
        };
        is_under(&link.sheet, &source.sheet)
    }

    /// The member footprints of `footprints`, in board order.
    pub fn members<'a>(
        &'a self,
        footprints: &'a [Footprint],
    ) -> impl Iterator<Item = &'a Footprint> + 'a {
        footprints.iter().filter(|fp| self.holds(fp))
    }
}

/// Whether `path` is `sheet` or a sheet below it; the same rule Update PCB
/// uses (`signex_engine::pcb::eco`).
pub fn is_under(path: &InstancePath, sheet: &InstancePath) -> bool {
    path.chain().starts_with(sheet.chain())
}

/// `Room <n>` with the lowest `n` from 1 no room of `rooms` is named.
pub fn next_room_name(rooms: &[Room], prefix: &str) -> String {
    (1..)
        .map(|n| format!("{prefix} {n}"))
        .find(|name| rooms.iter().all(|r| &r.name != name))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_between_two_corners_is_normalised() {
        let r = RoomOutline::rect_between(Point::new(10.0, 2.0), Point::new(4.0, 8.0));
        assert_eq!(
            r,
            RoomOutline::Rect {
                x_mm: 4.0,
                y_mm: 2.0,
                w_mm: 6.0,
                h_mm: 6.0
            }
        );
        assert!(r.has_area());
    }

    #[test]
    fn a_polygon_keeps_its_corners_through_a_shape_round_trip() {
        let rect = RoomOutline::Rect {
            x_mm: 1.0,
            y_mm: 2.0,
            w_mm: 3.0,
            h_mm: 4.0,
        };
        let poly = rect.as_shape(RoomShape::Polygon);
        assert_eq!(poly.shape(), RoomShape::Polygon);
        assert_eq!(poly.corners(), rect.corners());
        assert_eq!(poly.as_shape(RoomShape::Rectangle), rect);
    }

    #[test]
    fn a_resize_stretches_from_the_least_corner() {
        let poly = RoomOutline::Polygon(vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
        ]);
        let wide = poly.resized(8.0, 2.0);
        assert_eq!(wide.corners()[1], Point::new(8.0, 0.0));
        assert_eq!(wide.moved_to(1.0, 1.0).corners()[0], Point::new(1.0, 1.0));
    }

    #[test]
    fn room_names_and_colours_follow_creation_order() {
        let mut rooms = Vec::new();
        assert_eq!(next_room_name(&rooms, "Room"), "Room 1");
        rooms.push(Room::drawn(
            Uuid::nil(),
            "Room 1".into(),
            palette_colour(0),
            RoomOutline::rect_between(Point::ZERO, Point::new(1.0, 1.0)),
        ));
        assert_eq!(next_room_name(&rooms, "Room"), "Room 2");
        assert_eq!(palette_colour(2), "#A058C8");
        assert_eq!(palette_colour(ROOM_PALETTE.len()), ROOM_PALETTE[0]);
        assert!(is_room_colour("#A058C8"));
        assert!(!is_room_colour("teal"));
    }

    #[test]
    fn a_top_room_shows_on_the_top_view_only() {
        assert!(RoomLayer::Top.shows_on(false));
        assert!(!RoomLayer::Top.shows_on(true));
        assert!(RoomLayer::Bottom.shows_on(true));
        assert!(RoomLayer::Both.shows_on(true) && RoomLayer::Both.shows_on(false));
    }
}
