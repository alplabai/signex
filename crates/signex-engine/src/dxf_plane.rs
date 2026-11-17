//! The plane a DXF entity is drawn in, read the same way by the schematic
//! Import Graphics reader and the board DXF reader.

/// The X sign of an entity's own plane from its extrusion Z (code 230):
/// -1 when it points down the Z axis, as a drawing mirrored in a CAD tool
/// often stores circles, arcs and polylines; else 1. Only the Z sign is
/// read: a tilted extrusion (codes 210/220) is not a flat drawing.
pub(crate) fn plane_x_sign(extrusion_z: Option<&str>) -> f64 {
    let down = extrusion_z
        .and_then(|v| v.trim().parse::<f64>().ok())
        .is_some_and(|z| z < 0.0);
    if down { -1.0 } else { 1.0 }
}
