//! What every [`SchDrawing`] kind answers alike: its id, its lock, the
//! format token it needs and the annotation kinds' point map.

use uuid::Uuid;

use super::{Point, SchDrawing};

impl SchDrawing {
    /// The drawing's own id, whichever shape it is.
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Line { uuid, .. }
            | Self::Rect { uuid, .. }
            | Self::Circle { uuid, .. }
            | Self::Arc { uuid, .. }
            | Self::Polyline { uuid, .. } => *uuid,
            Self::Bezier(b) => b.uuid,
            Self::Dimension(d) => d.uuid,
            Self::Table(t) => t.uuid,
            Self::Image(i) => i.uuid,
            Self::Curve(c) => c.uuid,
        }
    }

    /// Give the drawing a new id, whichever shape it is.
    pub fn set_uuid(&mut self, id: Uuid) {
        match self {
            Self::Line { uuid, .. }
            | Self::Rect { uuid, .. }
            | Self::Circle { uuid, .. }
            | Self::Arc { uuid, .. }
            | Self::Polyline { uuid, .. } => *uuid = id,
            Self::Bezier(b) => b.uuid = id,
            Self::Dimension(d) => d.uuid = id,
            Self::Table(t) => t.uuid = id,
            Self::Image(i) => i.uuid = id,
            Self::Curve(c) => c.uuid = id,
        }
    }

    /// Whether the drawing is locked (#1573), whichever shape it is.
    pub fn is_locked(&self) -> bool {
        match self {
            Self::Line { locked, .. }
            | Self::Rect { locked, .. }
            | Self::Circle { locked, .. }
            | Self::Arc { locked, .. }
            | Self::Polyline { locked, .. } => *locked,
            Self::Bezier(b) => b.locked,
            Self::Dimension(d) => d.locked,
            Self::Table(t) => t.locked,
            Self::Image(i) => i.locked,
            Self::Curve(c) => c.locked,
        }
    }

    /// Set the lock, whichever shape the drawing is.
    pub fn set_locked(&mut self, value: bool) {
        match self {
            Self::Line { locked, .. }
            | Self::Rect { locked, .. }
            | Self::Circle { locked, .. }
            | Self::Arc { locked, .. }
            | Self::Polyline { locked, .. } => *locked = value,
            Self::Bezier(b) => b.locked = value,
            Self::Dimension(d) => d.locked = value,
            Self::Table(t) => t.locked = value,
            Self::Image(i) => i.locked = value,
            Self::Curve(c) => c.locked = value,
        }
    }

    /// Whether a sheet holding this drawing needs format `snxsch/1.3`: the
    /// Bezier, dimension and table kinds are new in it (#1267, #1300). An
    /// image needs no new token: its rows are a key an older build keeps.
    pub fn needs_format_1_3(&self) -> bool {
        matches!(self, Self::Bezier(_) | Self::Dimension(_) | Self::Table(_))
    }

    /// Whether a sheet holding this drawing needs format `snxsch/1.5`: the
    /// ellipse, elliptical arc and round rectangle are new in it (#1267).
    pub fn needs_curve_format(&self) -> bool {
        matches!(self, Self::Curve(_))
    }

    /// Map an annotation kind's points through `f` (move, rotate, mirror);
    /// `false` for the classic shapes, which their callers move point by
    /// point.
    pub fn map_annotation_points(&mut self, f: impl Fn(Point) -> Point) -> bool {
        match self {
            Self::Bezier(b) => b.map_points(f),
            Self::Dimension(d) => d.map_points(f),
            Self::Table(t) => t.map_points(f),
            Self::Image(i) => i.map_points(f),
            Self::Curve(c) => c.map_points(f),
            Self::Line { .. }
            | Self::Rect { .. }
            | Self::Circle { .. }
            | Self::Arc { .. }
            | Self::Polyline { .. } => return false,
        }
        true
    }
}
