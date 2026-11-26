//! The Elliptical Arc gesture: five clicks (center, X radius, Y radius,
//! start, end; DECISIONS 2026-11-22). Angles are geometric, the direction
//! from the center to the click.

use signex_library::{SymbolGraphicKind, normalize_arc_endpoints_deg};

use super::super::{CanvasAction, unwrap_angle};
use super::tools::arc_sweep_exceeds_full_turn;

/// The smallest radius a click sets, as for a circle.
const MIN_RADIUS_MM: f64 = 0.1;
/// End angle of the whole-ellipse ghost shown before the start click.
const WHOLE_ELLIPSE_END_DEG: f64 = 359.999;

/// The clicks collected so far.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EllipseGesture {
    center: Option<(f64, f64)>,
    radius_x: Option<f64>,
    radius_y: Option<f64>,
    start_deg: Option<f64>,
    /// Unwrapped, so a sweep past +/-180 degrees does not jump.
    end_deg: Option<f64>,
    cursor: Option<(f64, f64)>,
}

impl EllipseGesture {
    pub fn is_active(&self) -> bool {
        self.center.is_some()
    }

    /// Follow the cursor while the gesture is in flight.
    pub fn track(&mut self, x: f64, y: f64) {
        self.cursor = Some((x, y));
        if let (Some((cx, cy)), Some(_)) = (self.center, self.start_deg) {
            let raw = (y - cy).atan2(x - cx).to_degrees();
            self.end_deg = Some(self.end_deg.map_or(raw, |prev| unwrap_angle(prev, raw)));
        }
    }

    /// Take one click. The fifth returns the action to publish; a sweep of a
    /// full turn or more is refused and leaves the gesture in place.
    pub fn click(&mut self, x: f64, y: f64) -> Option<CanvasAction> {
        self.track(x, y);
        let Some((cx, cy)) = self.center else {
            self.center = Some((x, y));
            return None;
        };
        let Some(radius_x) = self.radius_x else {
            self.radius_x = Some((x - cx).abs().max(MIN_RADIUS_MM));
            return None;
        };
        let Some(radius_y) = self.radius_y else {
            self.radius_y = Some((y - cy).abs().max(MIN_RADIUS_MM));
            return None;
        };
        let Some(start_deg) = self.start_deg else {
            let start = (y - cy).atan2(x - cx).to_degrees();
            self.start_deg = Some(start);
            self.end_deg = Some(start);
            return None;
        };
        let end_deg = self.end_deg.unwrap_or(start_deg);
        if arc_sweep_exceeds_full_turn(start_deg, end_deg) {
            return Some(CanvasAction::ArcSweepRejected);
        }
        *self = Self::default();
        Some(CanvasAction::AddEllipticalArc {
            cx,
            cy,
            radius_x,
            radius_y,
            start_deg,
            end_deg,
        })
    }

    /// The shape the next click would lead to, for the preview.
    pub fn preview(&self) -> Option<SymbolGraphicKind> {
        let (cx, cy) = self.center?;
        let (x, y) = self.cursor?;
        let radius_x = self
            .radius_x
            .unwrap_or_else(|| (x - cx).abs().max(MIN_RADIUS_MM));
        let radius_y = match (self.radius_x, self.radius_y) {
            (_, Some(r)) => r,
            (Some(_), None) => (y - cy).abs().max(MIN_RADIUS_MM),
            (None, None) => radius_x,
        };
        let (start_deg, end_deg) = match (self.start_deg, self.end_deg) {
            (Some(start), Some(end)) => normalize_arc_endpoints_deg(start, end),
            _ => (0.0, WHOLE_ELLIPSE_END_DEG),
        };
        Some(SymbolGraphicKind::EllipticalArc {
            center: [cx, cy],
            radius_x,
            radius_y,
            start_deg,
            end_deg,
        })
    }
}
