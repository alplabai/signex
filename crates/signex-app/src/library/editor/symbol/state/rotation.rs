//! Rotation + graphic translation helpers for the symbol editor.

use super::*;

/// Rotate the selected entity by 90°.
///
/// Pins rotate in place around their own center (orientation only),
/// while graphics rotate around world origin to preserve the existing
/// symbol-body rotate behavior.
/// `clockwise = true` rotates CW, `false` rotates CCW.
pub fn rotate_selected(sym: &mut Symbol, sel: Option<SymbolSelection>, clockwise: bool) {
    rotate_selected_with_pivot(sym, sel, clockwise, GraphicRotationPivotMode::WorldOrigin);
}

/// Rotate the selected entity by 90° with explicit graphic pivot mode.
pub fn rotate_selected_with_pivot(
    sym: &mut Symbol,
    sel: Option<SymbolSelection>,
    clockwise: bool,
    graphic_pivot_mode: GraphicRotationPivotMode,
) {
    match sel {
        Some(SymbolSelection::Pin(idx)) => {
            if let Some(pin) = sym.pins.get_mut(idx) {
                // Rotate tip around the body-end so the symbol-body
                // attachment point stays fixed (B-type anchor behavior).
                let (dx, dy) = pin_body_delta(pin);
                let body_end = Vec2d::new(pin.position[0] + dx, pin.position[1] + dy);
                let tip_local = Vec2d::new(-dx, -dy);
                let delta = if clockwise {
                    -std::f64::consts::FRAC_PI_2
                } else {
                    std::f64::consts::FRAC_PI_2
                };
                let new_tip_local = rotate_vec(tip_local, delta);
                pin.position[0] = snap_axis_value(body_end.x + new_tip_local.x);
                pin.position[1] = snap_axis_value(body_end.y + new_tip_local.y);
                pin.orientation = rotate_pin_orientation_90(pin.orientation, clockwise);
            }
        }
        Some(SymbolSelection::Graphic(idx)) => {
            rotate_graphic_90(sym, idx, clockwise, graphic_pivot_mode)
        }
        Some(SymbolSelection::Field(_))
        | Some(SymbolSelection::All)
        | Some(SymbolSelection::Multiple { .. })
        | None => {}
    }
}

/// Convenience wrapper for geometry-center graphic rotation scenarios.
pub fn rotate_selected_about_geometry_center(
    sym: &mut Symbol,
    sel: Option<SymbolSelection>,
    clockwise: bool,
) {
    rotate_selected_with_pivot(
        sym,
        sel,
        clockwise,
        GraphicRotationPivotMode::GeometryCenter,
    );
}

fn rotate_graphic_90(
    sym: &mut Symbol,
    idx: usize,
    clockwise: bool,
    graphic_pivot_mode: GraphicRotationPivotMode,
) {
    let Some(g) = sym.graphics.get_mut(idx) else {
        return;
    };

    let geometry_center = graphic_geometry_center(&g.kind);
    let pivot = match graphic_pivot_mode {
        GraphicRotationPivotMode::WorldOrigin => RotationPivot::WorldOrigin,
        GraphicRotationPivotMode::GeometryCenter => RotationPivot::GeometryCenter,
    };

    match &mut g.kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            *from = rotate_graphic_point_90(
                *from,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
            *to = rotate_graphic_point_90(
                *to,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
        }
        SymbolGraphicKind::Circle { center, .. } => {
            *center = rotate_graphic_point_90(
                *center,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
        }
        SymbolGraphicKind::Arc {
            center,
            start_deg,
            end_deg,
            ..
        } => {
            *center = rotate_graphic_point_90(
                *center,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
            let delta = if clockwise {
                -std::f64::consts::FRAC_PI_2
            } else {
                std::f64::consts::FRAC_PI_2
            };
            *start_deg = normalize_angle_rad((*start_deg).to_radians() + delta)
                .to_degrees()
                .rem_euclid(360.0);
            *end_deg = normalize_angle_rad((*end_deg).to_radians() + delta)
                .to_degrees()
                .rem_euclid(360.0);
        }
        SymbolGraphicKind::Text { position, .. } => {
            *position = rotate_graphic_point_90(
                *position,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
        }
        SymbolGraphicKind::Polygon { vertices } => {
            for v in vertices.iter_mut() {
                *v = rotate_graphic_point_90(
                    *v,
                    geometry_center,
                    clockwise,
                    RotationSpace::World,
                    pivot,
                );
            }
        }
    }
}

fn graphic_geometry_center(kind: &SymbolGraphicKind) -> [f64; 2] {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            [(from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5]
        }
        SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => *center,
        SymbolGraphicKind::Text { position, .. } => *position,
        SymbolGraphicKind::Polygon { vertices } => super::polygon_centroid(vertices),
    }
}

fn rotate_graphic_point_90(
    p: [f64; 2],
    geometry_center_world: [f64; 2],
    clockwise: bool,
    space: RotationSpace,
    pivot: RotationPivot,
) -> [f64; 2] {
    let delta = if clockwise {
        -std::f64::consts::FRAC_PI_2
    } else {
        std::f64::consts::FRAC_PI_2
    };

    let mut point_object = GraphicPointObject::new(p, geometry_center_world);
    rotate_object(&mut point_object, space, pivot, delta);
    point_object.world_point()
}

fn snap_axis_value(v: f64) -> f64 {
    if v.abs() < 1e-12 {
        return 0.0;
    }

    let rounded = v.round();
    if (v - rounded).abs() < 1e-12 {
        rounded
    } else {
        v
    }
}

#[derive(Debug, Clone, Copy)]
struct GraphicPointObject {
    pose: Pose2d,
    geometry_center_local: Vec2d,
}

impl GraphicPointObject {
    fn new(world_point: [f64; 2], geometry_center_world: [f64; 2]) -> Self {
        let point_origin = Vec2d::new(world_point[0], world_point[1]);
        let center_local = Vec2d::new(
            geometry_center_world[0] - world_point[0],
            geometry_center_world[1] - world_point[1],
        );
        Self {
            pose: Pose2d::new(point_origin, 0.0),
            geometry_center_local: center_local,
        }
    }

    fn world_point(self) -> [f64; 2] {
        [
            snap_axis_value(self.pose.origin.x),
            snap_axis_value(self.pose.origin.y),
        ]
    }
}

impl Rotatable2d for GraphicPointObject {
    fn pose(&self) -> Pose2d {
        self.pose
    }

    fn geometry_center_local(&self) -> Vec2d {
        self.geometry_center_local
    }

    fn set_pose(&mut self, pose: Pose2d) {
        self.pose = pose;
    }
}

/// Returns the (dx, dy) vector from the tip (connection point) to the
/// body-end (symbol-body attachment point) based on orientation and length.
pub(super) fn pin_body_delta(pin: &SymbolPin) -> (f64, f64) {
    match pin.orientation {
        PinOrientation::Right => (pin.length, 0.0),
        PinOrientation::Up => (0.0, pin.length),
        PinOrientation::Left => (-pin.length, 0.0),
        PinOrientation::Down => (0.0, -pin.length),
        _ => (-pin.length, 0.0),
    }
}

fn rotate_pin_orientation_90(o: PinOrientation, clockwise: bool) -> PinOrientation {
    match (o, clockwise) {
        (PinOrientation::Up, true) => PinOrientation::Right,
        (PinOrientation::Right, true) => PinOrientation::Down,
        (PinOrientation::Down, true) => PinOrientation::Left,
        (PinOrientation::Left, true) => PinOrientation::Up,
        (PinOrientation::Up, false) => PinOrientation::Left,
        (PinOrientation::Left, false) => PinOrientation::Down,
        (PinOrientation::Down, false) => PinOrientation::Right,
        (PinOrientation::Right, false) => PinOrientation::Up,
        _ => o,
    }
}

/// Translate the graphic at `idx` so its primary anchor lands on
/// `(x, y)`. Anchors are picked to match the visual centre of mass
/// of each shape: rectangles + lines move by the delta between the
/// new point and `from`; circles + arcs use `center`; text uses
/// `position`. No-op when `idx` is out of range.
pub(super) fn translate_graphic_to(sym: &mut Symbol, idx: usize, x: f64, y: f64) {
    let Some(g) = sym.graphics.get_mut(idx) else {
        return;
    };
    match &mut g.kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            let dx = x - from[0];
            let dy = y - from[1];
            from[0] += dx;
            from[1] += dy;
            to[0] += dx;
            to[1] += dy;
        }
        SymbolGraphicKind::Line { from, to } => {
            let dx = x - from[0];
            let dy = y - from[1];
            from[0] += dx;
            from[1] += dy;
            to[0] += dx;
            to[1] += dy;
        }
        SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
            center[0] = x;
            center[1] = y;
        }
        SymbolGraphicKind::Text { position, .. } => {
            position[0] = x;
            position[1] = y;
        }
        SymbolGraphicKind::Polygon { vertices } => {
            // Anchor is the centroid — matches `selection_anchor` so
            // the drag offset captured on press stays correct.
            let c = super::polygon_centroid(vertices);
            let dx = x - c[0];
            let dy = y - c[1];
            for v in vertices.iter_mut() {
                v[0] += dx;
                v[1] += dy;
            }
        }
    }
}

/// Shift a single graphic kind by `(dx, dy)` mm in-place.
pub fn translate_graphic_by(kind: &mut SymbolGraphicKind, dx: f64, dy: f64) {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            from[0] += dx;
            from[1] += dy;
            to[0] += dx;
            to[1] += dy;
        }
        SymbolGraphicKind::Line { from, to } => {
            from[0] += dx;
            from[1] += dy;
            to[0] += dx;
            to[1] += dy;
        }
        SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
            center[0] += dx;
            center[1] += dy;
        }
        SymbolGraphicKind::Text { position, .. } => {
            position[0] += dx;
            position[1] += dy;
        }
        SymbolGraphicKind::Polygon { vertices } => {
            for v in vertices.iter_mut() {
                v[0] += dx;
                v[1] += dy;
            }
        }
    }
}
