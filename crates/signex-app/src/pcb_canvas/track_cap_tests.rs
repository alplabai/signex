//! A track draws as a capsule on the CPU mount (#2391): a bend of 45 or 90
//! degrees has no notch on its outer side and a run's end is round. Read off
//! bare wgpu frames, so no adapter is needed.

use iced::advanced::graphics::Mesh;
use iced::{Point, Rectangle, Size};
use iced_wgpu::Geometry;
use signex_gfx::primitive::line::LineSegment;

use super::{Item, tessellate};
use crate::canvas::grid_order_tests::MeshOnly;
use crate::pcb_canvas::BoardCamera;

const WIDTH_MM: f32 = 1.0;
const VIEWPORT: Size = Size {
    width: 1000.0,
    height: 1000.0,
};

fn track(p0: [f32; 2], p1: [f32; 2]) -> LineSegment {
    LineSegment {
        p0,
        p1,
        width: WIDTH_MM,
        color: [1.0, 1.0, 1.0, 1.0],
        style: 0,
        _pad: 0,
    }
}

/// The triangles `tracks` tessellate to, in screen pixels, and the camera
/// that drew them: the board area from (-1, -1) to (19, 22) fitted.
fn triangles(tracks: &[LineSegment]) -> (Vec<[[f32; 2]; 3]>, BoardCamera) {
    let mut camera = BoardCamera::default();
    let viewport = Rectangle::with_size(VIEWPORT);
    camera.fit_rect(
        Rectangle::new(Point::new(-1.0, -1.0), Size::new(20.0, 23.0)),
        viewport,
    );
    let items: Vec<Item<'_>> = tracks.iter().map(Item::Line).collect();
    let mut out = Vec::new();
    for layer in tessellate(&MeshOnly, viewport, &items, &camera, 1) {
        let Geometry::Live { sections } = layer else {
            panic!("a fresh frame is live geometry");
        };
        for mesh in sections.into_iter().flat_map(|section| section.meshes) {
            let Mesh::Solid { buffers, .. } = mesh else {
                panic!("a track is a solid colour");
            };
            for corner in buffers.indices.chunks_exact(3) {
                let at = |i: u32| buffers.vertices[i as usize].position;
                out.push([at(corner[0]), at(corner[1]), at(corner[2])]);
            }
        }
    }
    (out, camera)
}

/// Whether board point `world` lands strictly inside a drawn triangle.
fn drawn(triangles: &[[[f32; 2]; 3]], camera: &BoardCamera, world: [f32; 2]) -> bool {
    let p = camera.world_to_screen(Point::new(world[0], world[1]));
    let side =
        |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (p.y - a[1]) - (b[1] - a[1]) * (p.x - a[0]);
    triangles.iter().any(|&[a, b, c]| {
        let (d0, d1, d2) = (side(a, b), side(b, c), side(c, a));
        (d0 > 0.0 && d1 > 0.0 && d2 > 0.0) || (d0 < 0.0 && d1 < 0.0 && d2 < 0.0)
    })
}

#[test]
fn a_right_angle_bend_is_filled_on_its_outer_corner() {
    let (tris, camera) = triangles(&[
        track([0.0, 0.0], [10.0, 0.0]),
        track([10.0, 0.0], [10.0, 10.0]),
    ]);
    // 0.42 mm from the joint, past the end of the first leg and before the
    // start of the second: only a round end reaches it.
    assert!(drawn(&tris, &camera, [10.3, -0.3]));
}

#[test]
fn a_45_degree_bend_is_filled_on_its_outer_corner() {
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let (tris, camera) = triangles(&[
        track([0.0, 0.0], [10.0, 0.0]),
        track([10.0, 0.0], [10.0 + 10.0 * half, 10.0 * half]),
    ]);
    // 0.45 mm from the joint, between the two legs' outer edges.
    assert!(drawn(&tris, &camera, [10.172, -0.416]));
}

#[test]
fn a_track_end_is_round() {
    let (tris, camera) = triangles(&[track([0.0, 20.0], [10.0, 20.0])]);
    assert!(
        drawn(&tris, &camera, [10.4, 20.0]),
        "past the end, on its axis"
    );
    // A square end would reach this corner; a round one, 0.64 mm out, does not.
    assert!(!drawn(&tris, &camera, [10.45, 20.45]));
    assert!(drawn(&tris, &camera, [5.0, 20.4]), "the body");
}
