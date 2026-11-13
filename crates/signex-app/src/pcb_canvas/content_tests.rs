//! The CPU board layer draws what the single-frame build drew (#2321): the
//! parallel runs give the same triangles in the same order, a pan reuses a
//! build only while it holds every primitive the live view shows, and the
//! moved triangles sit where a fresh build puts them. Read off bare wgpu
//! frames, so no adapter is needed; the pixel twin is
//! `pcb_canvas::content_pixel_tests`.

use iced::advanced::graphics::Mesh;
use iced::advanced::graphics::mesh::SolidVertex2D;
use iced::{Point, Rectangle, Size, Vector};
use iced_wgpu::Geometry;
use signex_renderer::pcb::{PcbLayerView, PcbSnapshot};

use super::content::{BuiltAt, Plan, RetainedBoard, cull_rect, plan};
use super::tessellate::{Item, tessellate, visible_items};
use super::{BoardCamera, PcbCanvas};
use crate::canvas::grid_order_tests::MeshOnly;
use crate::canvas::pan_reuse::clip_rect;

use super::frame_bench_tests::fixture;

const VIEWPORT: Size = Size {
    width: 1600.0,
    height: 1300.0,
};

/// The 10,000-track board fitted to the viewport and turned `degrees`.
fn board_canvas(degrees: f32) -> PcbCanvas {
    let board = fixture::board_10k_tracks();
    let mut canvas = PcbCanvas::new();
    canvas.set_renderer_snapshot(Some(PcbSnapshot::from_board_with_view(
        &board,
        &PcbLayerView::default(),
    )));
    let viewport = Rectangle::with_size(VIEWPORT);
    canvas.set_viewport(viewport);
    canvas.set_view_rotation(degrees);
    canvas.fit_to_board();
    let target = canvas.pending_fit.take().expect("the board has bounds");
    canvas.camera.borrow_mut().fit_rect(target, viewport);
    canvas
}

/// Every solid triangle corner of `geometry`, in draw order.
fn corners(geometry: Vec<Geometry>) -> Vec<SolidVertex2D> {
    let mut out = Vec::new();
    for layer in geometry {
        let Geometry::Live { sections } = layer else {
            panic!("a fresh frame is live geometry");
        };
        for mesh in sections.into_iter().flat_map(|section| section.meshes) {
            let Mesh::Solid { buffers, .. } = mesh else {
                panic!("the board draws solid colours only");
            };
            out.extend(
                buffers
                    .indices
                    .iter()
                    .map(|&i| buffers.vertices[i as usize]),
            );
        }
    }
    out
}

/// The items a build at `camera` keeps for screen rectangle `screen`.
fn kept<'a>(
    scene: &'a signex_gfx::scene::Scene,
    extents: &'a super::cull::SceneBounds,
    camera: &BoardCamera,
    screen: Rectangle,
) -> (Vec<Item<'a>>, bool) {
    visible_items(scene, extents, cull_rect(camera, screen))
}

fn address(item: &Item<'_>) -> usize {
    match *item {
        Item::Line(line) => std::ptr::from_ref(line) as usize,
        Item::Circle(circle) => std::ptr::from_ref(circle) as usize,
        Item::Polygon(polygon) => std::ptr::from_ref(polygon) as usize,
    }
}

#[test]
fn parallel_runs_draw_the_same_triangles_in_the_same_order() {
    for degrees in [0.0, 37.0] {
        let canvas = board_canvas(degrees);
        let scene = canvas.gpu_scene().expect("a board");
        let extents = canvas.cull_bounds(&scene);
        let camera = canvas.camera_ref().clone();
        let screen = Rectangle::with_size(VIEWPORT);
        let (items, complete) = kept(&scene, &extents, &camera, screen);
        assert!(complete, "the fitted board is all in view");
        let one = corners(tessellate(&MeshOnly, screen, &items, &camera, 1));
        let split = tessellate(&MeshOnly, screen, &items, &camera, 8);
        assert_eq!(split.len(), 8, "the board is worth eight runs");
        assert!(one.len() > 100_000, "{} corners", one.len());
        assert!(
            one == corners(split),
            "{degrees}°: eight runs drew other triangles than one frame"
        );
    }
}

#[test]
fn a_small_view_is_one_run() {
    let mut canvas = board_canvas(0.0);
    // Far in: a few primitives on screen.
    canvas.camera.get_mut().zoom_by_factor(Point::ORIGIN, 40.0);
    let scene = canvas.gpu_scene().expect("a board");
    let extents = canvas.cull_bounds(&scene);
    let camera = canvas.camera_ref().clone();
    let screen = Rectangle::with_size(VIEWPORT);
    let (items, complete) = kept(&scene, &extents, &camera, screen);
    assert!(!complete && !items.is_empty());
    assert_eq!(tessellate(&MeshOnly, screen, &items, &camera, 8).len(), 1);
}

/// A whole-pixel pan moves the track triangles to where a build at the new
/// camera puts them, to f32 noise. Tracks only: a filled shape goes through
/// lyon's sweep, whose triangle order may follow f32 ties that a shift
/// breaks differently; the pixel test (`content_pixel_tests`) covers those.
#[test]
fn moved_triangles_land_where_a_fresh_build_puts_them() {
    for degrees in [0.0, 37.0] {
        let canvas = board_canvas(degrees);
        let scene = canvas.gpu_scene().expect("a board");
        let extents = canvas.cull_bounds(&scene);
        let before = canvas.camera_ref().clone();
        let shift = Vector::new(37.0, -12.0);
        let mut after = before.clone();
        after.pan(shift.x, shift.y);
        let clip = clip_rect(VIEWPORT, 1.0);
        let (items, _) = kept(&scene, &extents, &before, clip);
        let items: Vec<Item<'_>> = items
            .into_iter()
            .filter(|item| matches!(item, Item::Line(_)))
            .collect();
        assert!(items.len() >= fixture::TRACK_COUNT, "the tracks are kept");
        let moved = corners(tessellate(&MeshOnly, clip, &items, &before, 8));
        let fresh = corners(tessellate(&MeshOnly, clip, &items, &after, 8));
        assert_eq!(moved.len(), fresh.len(), "{degrees}°: same triangles");
        let worst = moved
            .iter()
            .zip(&fresh)
            .map(|(m, f)| {
                assert_eq!(m.color, f.color);
                let dx = m.position[0] + shift.x - f.position[0];
                let dy = m.position[1] + shift.y - f.position[1];
                dx.abs().max(dy.abs())
            })
            .fold(0.0_f32, f32::max);
        assert!(worst < 1e-3, "{degrees}°: a corner moved {worst} px off");
    }
}

/// After a build that reaches over the clip rectangle, every primitive the
/// live view needs is in it for any pan the cover allows.
#[test]
fn a_reach_build_holds_what_the_panned_view_shows() {
    for degrees in [0.0, 37.0] {
        let mut canvas = board_canvas(degrees);
        canvas
            .camera
            .get_mut()
            .zoom_by_factor(Point::new(800.0, 650.0), 6.0);
        let scene = canvas.gpu_scene().expect("a board");
        let extents = canvas.cull_bounds(&scene);
        let built = canvas.camera_ref().clone();
        let clip = clip_rect(VIEWPORT, 1.0);
        let (held, complete) = kept(&scene, &extents, &built, clip);
        assert!(!complete, "zoomed in, the clip rectangle culls");
        let held: std::collections::HashSet<usize> = held.iter().map(address).collect();
        for shift in [
            (0.0, 0.0),
            (1599.0, 0.0),
            (-1600.0, 1300.0),
            (-800.0, -1299.0),
        ] {
            let mut live = built.clone();
            live.pan(shift.0, shift.1);
            let screen = Rectangle::with_size(VIEWPORT);
            let (needed, _) = kept(&scene, &extents, &live, screen);
            assert!(!needed.is_empty());
            let missing = needed
                .iter()
                .filter(|i| !held.contains(&address(i)))
                .count();
            assert_eq!(missing, 0, "{degrees}° pan {shift:?}: {missing} missing");
        }
    }
}

fn built(offset: (f32, f32)) -> BuiltAt {
    BuiltAt {
        offset,
        scale: 5.0,
        degrees: 0.0,
        device_px_per_logical_px: 1.0,
        viewport: VIEWPORT,
        scene_generation: 7,
    }
}

fn retained(cover: Rectangle) -> RetainedBoard {
    RetainedBoard {
        built: built((10.0, 20.0)),
        cover,
        layers: Vec::new(),
    }
}

#[test]
fn the_plan_reuses_only_a_whole_pixel_pan_inside_the_cover() {
    let tight = retained(Rectangle::with_size(VIEWPORT));
    let reach = retained(clip_rect(VIEWPORT, 1.0));
    let tight_build = Plan::Build { reach: false };

    assert_eq!(plan(None, &built((10.0, 20.0))), tight_build);
    assert_eq!(
        plan(Some(&tight), &built((10.0, 20.0))),
        Plan::Reuse(Vector::ZERO)
    );
    // A tight build covers the viewport only: a pan builds over the reach.
    assert_eq!(
        plan(Some(&tight), &built((16.0, 23.0))),
        Plan::Build { reach: true }
    );
    assert_eq!(
        plan(Some(&reach), &built((16.0, 23.0))),
        Plan::Reuse(Vector::new(6.0, 3.0))
    );
    assert_eq!(
        plan(Some(&reach), &built((10.0 - 1600.0, 20.0 + 1300.0))),
        Plan::Reuse(Vector::new(-1600.0, 1300.0))
    );
    assert_eq!(
        plan(Some(&reach), &built((10.0 + 1601.0, 20.0))),
        Plan::Build { reach: true }
    );
    // Off the device pixel grid: a fresh build, as before.
    assert_eq!(plan(Some(&reach), &built((10.5, 20.0))), tight_build);

    let changed = [
        BuiltAt {
            scale: 5.5,
            ..built((10.0, 20.0))
        },
        BuiltAt {
            degrees: 37.0,
            ..built((10.0, 20.0))
        },
        BuiltAt {
            device_px_per_logical_px: 2.0,
            ..built((10.0, 20.0))
        },
        BuiltAt {
            viewport: Size::new(800.0, 600.0),
            ..built((10.0, 20.0))
        },
        BuiltAt {
            scene_generation: 8,
            ..built((10.0, 20.0))
        },
    ];
    for live in changed {
        assert_eq!(plan(Some(&reach), &live), tight_build, "{live:?}");
    }
}

/// Through the layer: the build stays put across a pan, and a zoom, a
/// turn or a board change builds again.
#[test]
fn the_layer_keeps_its_build_across_a_pan_only() {
    let renderer = crate::layout_probe::headless_renderer();
    let mut canvas = board_canvas(0.0);
    let built_at = |canvas: &PcbCanvas| canvas.content.borrow().as_ref().map(|r| r.built);
    let draw = |canvas: &PcbCanvas| canvas.board_content(&renderer, VIEWPORT, 1.0);

    let (layers, shift) = draw(&canvas);
    assert!(!layers.is_empty());
    assert_eq!(shift, Vector::ZERO);
    let first = built_at(&canvas).expect("built");

    canvas.camera.get_mut().pan(6.0, 3.0);
    let (layers, shift) = draw(&canvas);
    assert!(!layers.is_empty());
    assert_eq!(shift, Vector::new(6.0, 3.0));
    assert_eq!(built_at(&canvas), Some(first), "a pan reuses the build");

    canvas
        .camera
        .get_mut()
        .zoom_by_factor(Point::new(800.0, 650.0), 1.1);
    assert_eq!(draw(&canvas).1, Vector::ZERO);
    let zoomed = built_at(&canvas).expect("built");
    assert_ne!(zoomed, first, "a zoom builds again");

    canvas.clear_content_cache();
    assert!(
        built_at(&canvas).is_none(),
        "a board change drops the build"
    );
    draw(&canvas);
    assert_ne!(built_at(&canvas), Some(zoomed));

    canvas.set_view_rotation(37.0);
    assert!(built_at(&canvas).is_none(), "a turn drops the build");
}
