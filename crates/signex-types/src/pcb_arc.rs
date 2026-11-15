//! A copper arc track (#2373): a track bent along the circle through its
//! start, a point on it and its end. The model and its geometry only; the
//! board list, the `.snxpcb` rows, drawing, hit-testing, the design rule
//! check and the ratsnest are still to come (see docs/PCB_TOOLS.md, "Arc").

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::format::{UnknownColumns, parse_pcb_layer};
use crate::pcb_layer::PcbLayer;
use crate::schematic::{Point, circumcircle};

/// The most chords [`TrackArc::points`] cuts an arc into, whatever the
/// tolerance asked.
pub const ARC_MAX_CHORDS: usize = 360;

/// A copper track along a circular arc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackArc {
    pub uuid: Uuid,
    pub start: Point,
    /// A point on the arc between `start` and `end`: it picks which way the
    /// arc bends.
    pub mid: Point,
    pub end: Point,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub net: u32,
    /// TSV columns this build does not recognise, kept per row so a save
    /// re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

/// The circle an arc lies on and how far it turns along it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcSweep {
    pub center: Point,
    pub radius: f64,
    /// The angle of `start` about the centre, in radians.
    pub start_angle: f64,
    /// The turn from `start` through `mid` to `end`, in radians:
    /// counter-clockwise positive, in `-TAU..TAU`.
    pub sweep: f64,
}

impl TrackArc {
    /// The typed layer, or `None` for a name this build does not know.
    pub fn pcb_layer(&self) -> Option<PcbLayer> {
        parse_pcb_layer(&self.layer)
    }

    /// The circle and turn of the arc; `None` when its three points are on
    /// one line (a straight track) or two of them are the same.
    pub fn sweep(&self) -> Option<ArcSweep> {
        if self.start == self.end || self.start == self.mid || self.mid == self.end {
            return None;
        }
        let (cx, cy, radius) = circumcircle(self.start, self.mid, self.end)?;
        if !radius.is_finite() || radius <= 0.0 {
            return None;
        }
        let angle = |p: Point| (p.y - cy).atan2(p.x - cx);
        let start_angle = angle(self.start);
        let to_end = (angle(self.end) - start_angle).rem_euclid(TAU);
        let to_mid = (angle(self.mid) - start_angle).rem_euclid(TAU);
        let sweep = if to_mid <= to_end {
            to_end
        } else {
            to_end - TAU
        };
        Some(ArcSweep {
            center: Point::new(cx, cy),
            radius,
            start_angle,
            sweep,
        })
    }

    /// The length of the arc's centre line in mm; the straight distance
    /// when it has no circle.
    pub fn length_mm(&self) -> f64 {
        match self.sweep() {
            Some(arc) => arc.sweep.abs() * arc.radius,
            None => (self.end.x - self.start.x).hypot(self.end.y - self.start.y),
        }
    }

    /// The arc as a polyline from `start` to `end`, each chord within
    /// `max_error_mm` of the arc (at most [`ARC_MAX_CHORDS`] chords). The
    /// first and last points are `start` and `end` exactly; an arc with no
    /// circle is the one chord between them.
    pub fn points(&self, max_error_mm: f64) -> Vec<Point> {
        let Some(arc) = self.sweep() else {
            return vec![self.start, self.end];
        };
        let chords = chord_count(arc, max_error_mm);
        let inner = (1..chords).map(|i| {
            let a = arc.start_angle + arc.sweep * i as f64 / chords as f64;
            Point::new(
                arc.center.x + arc.radius * a.cos(),
                arc.center.y + arc.radius * a.sin(),
            )
        });
        std::iter::once(self.start)
            .chain(inner)
            .chain(std::iter::once(self.end))
            .collect()
    }
}

/// How many equal chords keep each within `max_error_mm` of the arc: a
/// chord turning `step` strays `r * (1 - cos(step / 2))` from it.
fn chord_count(arc: ArcSweep, max_error_mm: f64) -> usize {
    if !max_error_mm.is_finite() || max_error_mm <= 0.0 {
        return ARC_MAX_CHORDS;
    }
    let half_step = (1.0 - max_error_mm / arc.radius).clamp(-1.0, 1.0).acos();
    if half_step <= 0.0 {
        return ARC_MAX_CHORDS;
    }
    let chords = (arc.sweep.abs() / (2.0 * half_step)).ceil();
    if chords.is_finite() {
        (chords as usize).clamp(1, ARC_MAX_CHORDS)
    } else {
        ARC_MAX_CHORDS
    }
}

#[cfg(test)]
#[path = "pcb_arc_tests.rs"]
mod tests;
