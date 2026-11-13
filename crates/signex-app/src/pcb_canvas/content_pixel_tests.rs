//! The CPU board layer (#2321) renders what trunk's CPU mount rendered,
//! pixel for pixel, after pans it answers from retained geometry, on a canvas
//! docked off the window origin, turned or not, at whole and fractional
//! display scales.
//!
//! The reference is trunk's content path: the board tessellated in one frame
//! of the canvas size at the live camera, culled to the viewport. Rendered
//! through iced's wgpu renderer (the harness of `canvas::gpu_mount_tests`).

use iced::widget::{canvas, stack};
use iced::{Element, Length, Point, Rectangle, Size, mouse};
use signex_renderer::pcb::PcbSnapshot;

use super::content::cull_rect;
use super::edge_parity_tests::mixed_tht_smd;
use super::tessellate::{tessellate, visible_items};
use super::{BoardContentLayer, PcbCanvas};
use crate::app::Message;
use crate::canvas::gpu_mount_tests::{Frame, gpu};
use crate::canvas::pan_reuse_pixel_tests::{DOCKED, Mount, differing, max_channel_diff, render_in};

/// Trunk's board content: one frame at the live camera, never retained.
struct FreshBoard<'a>(&'a PcbCanvas);

impl canvas::Program<Message> for FreshBoard<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let pcb = self.0;
        let Some(scene) = pcb.gpu_scene() else {
            return Vec::new();
        };
        let extents = pcb.cull_bounds(&scene);
        let camera = pcb.camera_ref();
        let screen = Rectangle::with_size(bounds.size());
        let (items, _) = visible_items(&scene, &extents, cull_rect(&camera, screen));
        tessellate(renderer, screen, &items, &camera, 1)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Path {
    /// Trunk's content path.
    Trunk,
    /// The retained, parallel layer.
    Layer,
}

fn element(path: Path, pcb: &PcbCanvas, dpi: f32) -> Element<'_, Message> {
    let base = canvas(pcb).width(Length::Fill).height(Length::Fill);
    match path {
        Path::Trunk => {
            let content = canvas(FreshBoard(pcb))
                .width(Length::Fill)
                .height(Length::Fill);
            stack![base, content].into()
        }
        Path::Layer => stack![base, BoardContentLayer::new(pcb, dpi)].into(),
    }
}

/// The corpus board fitted to the canvas of `mount`, turned `degrees`, then
/// zoomed in so part of it is off screen and culled.
fn board_canvas(mount: Mount, degrees: f32) -> PcbCanvas {
    let viewport = Rectangle::with_size(Size::new(
        mount.window.0 as f32 - mount.pad.0,
        mount.window.1 as f32 - mount.pad.1,
    ));
    let mut pcb = PcbCanvas::new();
    pcb.set_renderer_snapshot(Some(PcbSnapshot::from_board(&mixed_tht_smd())));
    pcb.set_viewport(viewport);
    pcb.set_view_rotation(degrees);
    pcb.fit_to_board();
    let target = pcb.pending_fit.take().expect("the board has bounds");
    let camera = pcb.camera.get_mut();
    camera.fit_rect(target, viewport);
    camera.zoom_by_factor(viewport.center(), 2.5);
    pcb
}

/// Render the layer at the start camera and after every pan of `pans`
/// (logical px), then trunk's path at the last camera, and compare.
fn assert_matches_trunk(mount: Mount, degrees: f32, pans: &[(f32, f32)]) -> usize {
    let mut gpu = gpu();
    let pcb = board_canvas(mount, degrees);
    let mut layer = render_in(&mut gpu, mount, element(Path::Layer, &pcb, mount.dpi));
    let mut builds = vec![built_offset(&pcb)];
    for &(dx, dy) in pans {
        pcb.camera.borrow_mut().pan(dx, dy);
        layer = render_in(&mut gpu, mount, element(Path::Layer, &pcb, mount.dpi));
        builds.push(built_offset(&pcb));
    }
    let trunk = render_in(&mut gpu, mount, element(Path::Trunk, &pcb, mount.dpi));
    assert_drawn(&trunk);
    let off = differing(&layer, &trunk);
    eprintln!(
        "dpi {} at {degrees}°: {} pixels differ from trunk, max channel diff {}",
        mount.dpi,
        off.len(),
        max_channel_diff(&layer, &trunk)
    );
    assert!(
        off.is_empty(),
        "dpi {} at {degrees}°: {} pixels differ from trunk, first at {:?}",
        mount.dpi,
        off.len(),
        off.first()
    );
    builds.dedup();
    builds.len()
}

fn built_offset(pcb: &PcbCanvas) -> Option<(f32, f32)> {
    pcb.content
        .borrow()
        .as_ref()
        .map(|retained| retained.built.offset)
}

/// The frame shows the board, not a flat fill.
fn assert_drawn(frame: &Frame) {
    let colours: std::collections::HashSet<&[u8]> = frame.data.chunks_exact(4).collect();
    assert!(colours.len() >= 3, "{} colours", colours.len());
}

#[test]
fn the_layer_matches_trunk_before_any_pan() {
    for degrees in [0.0, 37.0] {
        assert_eq!(assert_matches_trunk(DOCKED, degrees, &[]), 1);
    }
}

/// The first pan builds over the reach; the rest are drawn from it.
#[test]
fn the_layer_matches_trunk_after_pans_it_reused() {
    let pans = [(8.0, 4.0), (8.0, 4.0), (8.0, 4.0), (300.0, -200.0)];
    for degrees in [0.0, 37.0] {
        let builds = assert_matches_trunk(DOCKED, degrees, &pans);
        assert_eq!(builds, 2, "{degrees}°: the start build and one pan build");
    }
}

/// Pans of whole device pixels at fractional scales reuse the build too.
#[test]
fn a_fractional_display_scale_matches_trunk() {
    for dpi in [1.25, 1.5] {
        let mount = Mount { dpi, ..DOCKED };
        let builds = assert_matches_trunk(mount, 0.0, &[(8.0, 4.0); 5]);
        assert_eq!(builds, 2, "dpi {dpi}");
    }
}

/// A pan off the device pixel grid builds again, and still matches.
#[test]
fn an_off_grid_pan_builds_again_and_matches_trunk() {
    let pans = [(8.0, 4.0), (0.5, 0.25)];
    assert_eq!(assert_matches_trunk(DOCKED, 0.0, &pans), 3);
}

/// The test board is big enough at this zoom for culling to matter.
#[test]
fn the_zoomed_board_is_partly_off_screen() {
    let pcb = board_canvas(DOCKED, 0.0);
    let scene = pcb.gpu_scene().expect("a board");
    let extents = pcb.cull_bounds(&scene);
    let camera = pcb.camera_ref();
    let screen = Rectangle::new(
        Point::ORIGIN,
        Size::new(
            DOCKED.window.0 as f32 - DOCKED.pad.0,
            DOCKED.window.1 as f32 - DOCKED.pad.1,
        ),
    );
    let (_, complete) = visible_items(&scene, &extents, cull_rect(&camera, screen));
    assert!(!complete, "zoomed in, the viewport culls part of the board");
}
