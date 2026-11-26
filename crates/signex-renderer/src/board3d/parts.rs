//! A component in 3D: its model or its plain body, placed as the footprint
//! is (`pcb_geom::Placement`: mirror on the bottom, turn, move) and stood
//! on its face. A bottom part hangs below the bottom face, so mirroring x
//! and flipping z together turn it over rather than reflect it.

use pcb_geom::{Placement, PointNm, Shape, Side, footprint_body, nm_to_mm};
use signex_gfx::scene3d::Scene3d;
use signex_types::pcb::{Footprint, Point};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use super::{AttachedModel, Board3dInput, BodyExtrusion, ComponentModel, Face, Part3d, board_xy};

/// Height of a body whose footprint gives none, mm (the library 3D Body's
/// own default).
pub(super) const DEFAULT_BODY_HEIGHT_MM: f64 = 1.0;
/// Half the side of the body of a footprint with no pads and no courtyard.
const EMPTY_BODY_HALF_MM: f64 = 0.5;

/// Append the part of `footprint`; `None` when the user hid its face's 3D
/// Body layer (single-layer mode does not count) or its placement cannot be
/// read.
pub(super) fn append_part(
    scene: &mut Scene3d,
    footprint: &Footprint,
    input: &Board3dInput<'_>,
    thickness: f32,
) -> Option<Part3d> {
    let placement = Placement::of_footprint(footprint).ok()?;
    let (face, layer) = match placement.side {
        Side::Top => (Face::Top, TechnicalLayer::Body3dTop),
        Side::Bottom => (Face::Bottom, TechnicalLayer::Body3dBottom),
    };
    if input.view.is_hidden(PcbLayer::Technical(layer)) {
        return None;
    }
    let stand = Stand {
        placement,
        face,
        thickness,
        opacity: footprint.component.body_opacity.clamp(0.0, 1.0) as f32,
    };
    let start = scene.indices.len() as u32;
    let model = footprint
        .library_ref
        .as_ref()
        .and_then(|source| input.models.get(source));
    match model {
        Some(ComponentModel::Model(model)) => append_model(scene, &stand, model),
        Some(ComponentModel::Body(body)) => {
            append_body(scene, &stand, footprint, body, input.colors.body)
        }
        None => {
            let height = footprint.component.height_mm;
            let body = BodyExtrusion {
                outline: None,
                height_mm: if height > 0.0 {
                    height
                } else {
                    DEFAULT_BODY_HEIGHT_MM
                },
                offset_z_mm: 0.0,
                colors: None,
            };
            append_body(scene, &stand, footprint, &body, input.colors.body);
        }
    }
    let end = scene.indices.len() as u32;
    (start < end).then_some(Part3d {
        footprint: footprint.uuid,
        indices: start..end,
        face,
    })
}

/// Where a part stands: its placement, its face and how see-through it is.
struct Stand {
    placement: Placement,
    face: Face,
    thickness: f32,
    opacity: f32,
}

impl Stand {
    /// A footprint-frame point (mm) on the board, in the 3D frame.
    fn xy(&self, local: [f64; 2]) -> Option<[f32; 2]> {
        let local = PointNm::from_mm(Point::new(local[0], local[1]), "part.point").ok()?;
        let world = self.placement.to_world(local);
        Some(board_xy([
            nm_to_mm(world.x) as f32,
            nm_to_mm(world.y) as f32,
        ]))
    }

    /// Height `above` the face the part stands on, in the 3D frame.
    fn z(&self, above: f64) -> f32 {
        match self.face {
            Face::Top => self.thickness + above as f32,
            Face::Bottom => -(above as f32),
        }
    }

    fn tint(&self, mut color: [f32; 4]) -> [f32; 4] {
        color[3] *= self.opacity;
        color
    }
}

fn append_body(
    scene: &mut Scene3d,
    stand: &Stand,
    footprint: &Footprint,
    body: &BodyExtrusion,
    plain_color: [f32; 4],
) {
    let ring: Option<Vec<[f32; 2]>> = match body.outline.as_deref() {
        Some(outline) if outline.len() >= 3 => outline.iter().map(|&p| stand.xy(p)).collect(),
        _ => body_box(footprint, stand),
    };
    let Some(ring) = ring else {
        return;
    };
    let (top, sides) = body.colors.unwrap_or((plain_color, plain_color));
    let height = if body.height_mm.is_finite() && body.height_mm > 0.0 {
        body.height_mm
    } else {
        DEFAULT_BODY_HEIGHT_MM
    };
    let offset = if body.offset_z_mm.is_finite() {
        body.offset_z_mm
    } else {
        0.0
    };
    let (z0, z1) = (stand.z(offset), stand.z(offset + height));
    scene.append_prism(&ring, &[], (z0, z1), stand.tint(top), stand.tint(sides));
}

/// The footprint's body box (`pcb_geom::footprint_body`) as a ring in the
/// 3D frame; a small square at the origin for a footprint with nothing.
fn body_box(footprint: &Footprint, stand: &Stand) -> Option<Vec<[f32; 2]>> {
    match footprint_body(footprint).ok()? {
        Shape::Rect {
            center,
            half_w,
            half_h,
            rotation_deg,
        } => {
            let (sin, cos) = rotation_deg.to_radians().sin_cos();
            let (cx, cy) = (nm_to_mm(center.x), nm_to_mm(center.y));
            let (hw, hh) = (nm_to_mm(half_w), nm_to_mm(half_h));
            let ring = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
                .into_iter()
                .map(|(x, y)| {
                    let world = [cx + x * cos - y * sin, cy + x * sin + y * cos];
                    board_xy([world[0] as f32, world[1] as f32])
                })
                .collect();
            Some(ring)
        }
        _ => {
            let h = EMPTY_BODY_HALF_MM;
            [[-h, -h], [h, -h], [h, h], [-h, h]]
                .into_iter()
                .map(|p| stand.xy(p))
                .collect()
        }
    }
}

fn append_model(scene: &mut Scene3d, stand: &Stand, model: &AttachedModel) {
    for mesh in &model.meshes {
        let positions: Option<Vec<[f32; 3]>> = mesh
            .positions
            .iter()
            .map(|&p| {
                let [x, y, z] = model_point_on_footprint(model, p);
                let xy = stand.xy([x, y])?;
                Some([xy[0], xy[1], stand.z(z)])
            })
            .collect();
        if let Some(positions) = positions {
            scene.append(&positions, &mesh.triangles, stand.tint(mesh.color));
        }
    }
}

/// A point of `model`'s mesh in the footprint's frame (mm, y down the
/// board, z up from the face the part stands on), turned and offset by its
/// attachment: what the 3D view draws and DRC measures (#2939).
pub fn model_point_on_footprint(model: &AttachedModel, p: [f32; 3]) -> [f64; 3] {
    let [rx, ry, rz] = model.rotation_deg.map(f64::to_radians);
    let p = turn_xyz(
        [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])],
        rx,
        ry,
        rz,
    );
    // The model's y points up the board; the footprint frame's y points
    // down it.
    [
        p[0] + model.offset_mm[0],
        -(p[1] + model.offset_mm[1]),
        p[2] + model.offset_mm[2],
    ]
}

/// `p` turned about x by `rx`, then y by `ry`, then z by `rz` (radians,
/// counter-clockwise looking down each axis).
fn turn_xyz(p: [f64; 3], rx: f64, ry: f64, rz: f64) -> [f64; 3] {
    let (s, c) = rx.sin_cos();
    let p = [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c];
    let (s, c) = ry.sin_cos();
    let p = [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c];
    let (s, c) = rz.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
}
