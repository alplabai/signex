//! The schematic background's draw order, read off a bare wgpu frame (#1993).
//!
//! No adapter needed: `iced_wgpu::geometry::Frame` tessellates on the CPU, and
//! its meshes come out in the order the renderer would draw them. The pixel
//! twin of this check is `gpu_mount_tests::the_backdrop_shows_the_grid_over_the_paper`.

use iced::advanced::graphics::Mesh;
use iced::advanced::graphics::color::pack;
use iced::advanced::graphics::geometry::Renderer as GeometryRenderer;
use iced::advanced::renderer::Quad;
use iced::advanced::{Renderer as CoreRenderer, image};
use iced::{Background, Rectangle, Size, Transformation};
use iced_wgpu::Geometry;

use super::tests::test_prefs;
use super::*;

/// A renderer that only builds wgpu frames, so the draw order is observable
/// without a GPU.
struct MeshOnly;

impl CoreRenderer for MeshOnly {
    fn start_layer(&mut self, _bounds: Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _transformation: Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, _quad: Quad, _background: impl Into<Background>) {}
    fn reset(&mut self, _new_bounds: Rectangle) {}
    fn allocate_image(
        &mut self,
        _handle: &image::Handle,
        _callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
    }
}

impl GeometryRenderer for MeshOnly {
    type Geometry = Geometry;
    type Frame = iced_wgpu::geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        iced_wgpu::geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, _geometry: Self::Geometry) {}
}

/// Solid vertex colours of the background, in draw order.
fn background_vertex_colors(prefs: CanvasViewPrefs<'_>) -> Vec<[f32; 4]> {
    let slot = CanvasSlot::new();
    let canvas = SchematicCanvas::new(&slot, prefs);
    let bounds = Rectangle::with_size(Size::new(800.0, 600.0));
    let mut frame = canvas::Frame::<MeshOnly>::new(&MeshOnly, bounds.size());
    canvas.paint_background(&mut frame, bounds);
    let Geometry::Live { meshes, .. } = frame.into_geometry() else {
        panic!("a fresh frame is live geometry");
    };
    meshes
        .iter()
        .filter_map(|mesh| match mesh {
            Mesh::Solid { buffers, .. } => Some(buffers.vertices.iter().map(|v| v.color)),
            Mesh::Gradient { .. } => None,
        })
        .flatten()
        .map(|c| c.components())
        .collect()
}

/// #1993 - the grid is painted after the paper fill, not under it. Trunk drew
/// the grid's meshes first (`Frame::with_clip`), so the opaque paper covered it.
#[test]
fn the_grid_is_painted_after_the_paper_fill() {
    let (overrides, filters) = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    assert!(prefs.grid_visible);
    let (bg, paper) = (
        pack(prefs.theme_bg).components(),
        pack(prefs.theme_paper).components(),
    );
    assert_ne!(bg, paper, "the test needs a visible paper");

    let colors = background_vertex_colors(prefs);
    let last_paper = colors
        .iter()
        .rposition(|c| *c == paper)
        .expect("the paper fill is in the frame");
    let drawn_before_paper = colors[..last_paper]
        .iter()
        .filter(|c| **c != bg && **c != paper)
        .count();
    assert_eq!(
        drawn_before_paper, 0,
        "{drawn_before_paper} vertices (the grid) are painted before the paper fill"
    );
    let grid_after = colors[last_paper..]
        .iter()
        .filter(|c| **c != bg && **c != paper)
        .count();
    assert!(
        grid_after > 100,
        "the grid dots follow the paper fill (found {grid_after} vertices)"
    );
}

/// The shown grid adds its dots on top of the hidden-grid background.
#[test]
fn a_hidden_grid_adds_no_vertices_after_the_paper() {
    let (overrides, filters) = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    let shown = background_vertex_colors(prefs).len();
    prefs.grid_visible = false;
    let hidden = background_vertex_colors(prefs).len();
    assert!(shown > hidden + 100, "shown {shown}, hidden {hidden}");
}

/// Not a gate: `cargo test -p signex-app --lib grid_tessellation_cost -- --ignored --nocapture`
/// prints the CPU time of one background tessellation (#1993). Debug builds
/// run it at opt-level 0, so only the before/after ratio means anything.
#[test]
#[ignore = "timing probe, not a gate"]
fn grid_tessellation_cost() {
    let (overrides, filters) = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    let runs = 20;
    let started = std::time::Instant::now();
    let mut vertices = 0;
    for _ in 0..runs {
        vertices = background_vertex_colors(prefs).len();
    }
    eprintln!(
        "background: {vertices} vertices, {:?} per tessellation",
        started.elapsed() / runs
    );
}
