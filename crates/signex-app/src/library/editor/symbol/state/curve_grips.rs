//! Grips of the Elliptical Arc and Round Rectangle (#2838). A round
//! rectangle has the Rectangle's eight; an elliptical arc has the Arc's two
//! plus one per radius.

use super::*;

/// Floor of a dragged radius, as for a circle.
const MIN_RADIUS_MM: f64 = 0.1;

/// The grips of `kind`, `None` for the kinds `hit_test` handles itself.
pub(super) fn curve_handles(kind: &SymbolGraphicKind) -> Option<Vec<(GraphicHandle, [f64; 2])>> {
    match kind {
        SymbolGraphicKind::RoundRectangle { from, to, .. } => {
            let (mx, my) = ((from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5);
            Some(vec![
                (GraphicHandle::RectCorner(0), [from[0], to[1]]),
                (GraphicHandle::RectCorner(1), [to[0], to[1]]),
                (GraphicHandle::RectCorner(2), [to[0], from[1]]),
                (GraphicHandle::RectCorner(3), [from[0], from[1]]),
                (GraphicHandle::RectEdge(0), [mx, to[1]]),
                (GraphicHandle::RectEdge(1), [to[0], my]),
                (GraphicHandle::RectEdge(2), [mx, from[1]]),
                (GraphicHandle::RectEdge(3), [from[0], my]),
            ])
        }
        SymbolGraphicKind::EllipticalArc {
            center,
            radius_x,
            radius_y,
            start_deg,
            end_deg,
        } => {
            let at = |deg| SymbolGraphicKind::point_on_ellipse(*center, *radius_x, *radius_y, deg);
            Some(vec![
                (GraphicHandle::ArcStart, at(*start_deg)),
                (GraphicHandle::ArcEnd, at(*end_deg)),
                (
                    GraphicHandle::EllipseRadius(0),
                    [center[0] + radius_x, center[1]],
                ),
                (
                    GraphicHandle::EllipseRadius(1),
                    [center[0], center[1] + radius_y],
                ),
            ])
        }
        _ => None,
    }
}

/// Move a grip of `kind` to `(x, y)`; `false` when `kind` is not one of
/// these two. An angle grip keeps the radii, a radius grip keeps the angles.
pub(super) fn move_curve_handle(
    kind: &mut SymbolGraphicKind,
    handle: GraphicHandle,
    x: f64,
    y: f64,
) -> bool {
    match kind {
        SymbolGraphicKind::RoundRectangle { from, to, .. } => {
            move_rect_handle(from, to, handle, x, y);
            true
        }
        SymbolGraphicKind::EllipticalArc {
            center,
            radius_x,
            radius_y,
            start_deg,
            end_deg,
        } => {
            let angle = (y - center[1])
                .atan2(x - center[0])
                .to_degrees()
                .rem_euclid(360.0);
            match handle {
                GraphicHandle::ArcStart => *start_deg = angle,
                GraphicHandle::ArcEnd => *end_deg = angle,
                GraphicHandle::EllipseRadius(0) => {
                    *radius_x = (x - center[0]).abs().max(MIN_RADIUS_MM);
                }
                GraphicHandle::EllipseRadius(1) => {
                    *radius_y = (y - center[1]).abs().max(MIN_RADIUS_MM);
                }
                _ => {}
            }
            true
        }
        _ => false,
    }
}

fn move_rect_handle(from: &mut [f64; 2], to: &mut [f64; 2], handle: GraphicHandle, x: f64, y: f64) {
    match handle {
        GraphicHandle::RectCorner(0) => (from[0], to[1]) = (x, y),
        GraphicHandle::RectCorner(1) => *to = [x, y],
        GraphicHandle::RectCorner(2) => (to[0], from[1]) = (x, y),
        GraphicHandle::RectCorner(3) => *from = [x, y],
        GraphicHandle::RectEdge(0) => to[1] = y,
        GraphicHandle::RectEdge(1) => to[0] = x,
        GraphicHandle::RectEdge(2) => from[1] = y,
        GraphicHandle::RectEdge(3) => from[0] = x,
        _ => {}
    }
}
