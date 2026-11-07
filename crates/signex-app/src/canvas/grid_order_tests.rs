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

use crate::canvas::grid::draw_grid;
use crate::render_config::GridStyle;

use super::tests::test_prefs;
use super::*;

/// A renderer that only builds wgpu frames, so the draw order is observable
/// without a GPU.
pub(super) struct MeshOnly;

impl CoreRenderer for MeshOnly {
    fn start_layer(&mut self, _bounds: Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _transformation: Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, _quad: Quad, _background: impl Into<Background>) {}
    fn reset(&mut self, _new_bounds: Rectangle) {}
    fn hint(&mut self, _scale: iced::advanced::renderer::Scale) {}
    fn scale(&self) -> Option<iced::advanced::renderer::Scale> {
        None
    }
    fn settings(&self) -> iced::advanced::renderer::Settings {
        iced::advanced::renderer::Settings::default()
    }
    fn allocate_image(
        &self,
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
    background_vertices(prefs, None, Size::new(800.0, 600.0))
        .into_iter()
        .map(|(_, color)| color)
        .collect()
}

/// Solid vertices (position, colour) of the background at `camera` in a
/// `size` canvas, in draw order.
fn background_vertices(
    prefs: CanvasViewPrefs<'_>,
    camera: Option<Camera>,
    size: Size,
) -> Vec<([f32; 2], [f32; 4])> {
    let slot = CanvasSlot::new();
    if let Some(camera) = camera {
        *slot.camera_mut() = camera;
    }
    let canvas = SchematicCanvas::new(&slot, prefs);
    let bounds = Rectangle::with_size(size);
    let mut frame = canvas::Frame::<MeshOnly>::new(&MeshOnly, bounds.size());
    canvas.paint_background(&mut frame, bounds);
    let Geometry::Live { sections } = frame.into_geometry() else {
        panic!("a fresh frame is live geometry");
    };
    sections
        .iter()
        .flat_map(|section| section.meshes.iter())
        .filter_map(|mesh| match mesh {
            Mesh::Solid { buffers, .. } => Some(buffers.vertices.iter()),
            Mesh::Gradient { .. } => None,
        })
        .flatten()
        .map(|v| (v.position, v.color.components()))
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

/// #1940 - with the clip gone, no grid line reaches past the paper edge. The
/// camera puts the paper edges on whole pixels, so a line on the edge (stop 0)
/// or a stroke's half width would show as a vertex outside it.
#[test]
fn grid_lines_stay_inside_the_paper() {
    let (overrides, filters) = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.grid_style = crate::render_config::GridStyle::Lines;
    let camera = Camera {
        offset: iced::Point::new(20.0, 20.0),
        scale: 8.0,
    };
    let (bg, paper, border) = (
        pack(prefs.theme_bg).components(),
        pack(prefs.theme_paper).components(),
        pack(prefs.theme_grid).components(),
    );
    // The default sheet is 297 x 210 mm: 2376 x 1680 px at scale 8.
    let size = Size::new(2600.0, 1900.0);
    let (left, top, right, bottom) = (20.0, 20.0, 20.0 + 297.0 * 8.0, 20.0 + 210.0 * 8.0);

    let mut lines = 0;
    for (pos, color) in background_vertices(prefs, Some(camera), size) {
        if color == bg || color == paper || color == border {
            continue;
        }
        lines += 1;
        let eps = 1e-3;
        assert!(
            pos[0] >= left - eps
                && pos[0] <= right + eps
                && pos[1] >= top - eps
                && pos[1] <= bottom + eps,
            "grid vertex {pos:?} lies outside the paper ({left}..{right}, {top}..{bottom})"
        );
    }
    assert!(lines > 100, "the line grid is drawn ({lines} vertices)");
}

/// #1993 - the background re-tessellates when the camera moves and not when
/// nothing did, whatever event announced the move.
#[test]
fn the_background_follows_the_camera_and_ignores_a_hover() {
    let (overrides, filters) = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    let renderer = crate::layout_probe::headless_renderer();
    let bounds = Rectangle::with_size(Size::new(800.0, 600.0));
    let slot = CanvasSlot::new();
    let tessellations = |slot: &CanvasSlot| {
        SchematicCanvas::new(slot, prefs).draw_background(&renderer, bounds);
        slot.bg_tessellations.get()
    };

    assert_eq!(tessellations(&slot), 1, "first draw tessellates");
    assert_eq!(
        tessellations(&slot),
        1,
        "an unchanged camera reuses the cache"
    );

    slot.camera_mut().scale *= 1.1;
    assert_eq!(tessellations(&slot), 2, "a zoom re-tessellates");
    slot.camera_mut().offset.x += 15.0;
    assert_eq!(tessellations(&slot), 3, "a pan re-tessellates");
    assert_eq!(tessellations(&slot), 3, "and then it is cached again");
}

/// Not a gate:`cargo test -p signex-app --lib grid_tessellation_cost -- --ignored --nocapture`
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

/// Every vertex the grid draws at `dpr`, as the real call sites emit them.
fn drawn_grid_vertices(style: GridStyle, dpr: f32) -> Vec<[f32; 2]> {
    let camera = Camera {
        offset: iced::Point::new(13.3, 9.7),
        scale: 2.37,
    };
    let bounds = Rectangle::with_size(Size::new(1000.0, 700.0));
    let mut frame = canvas::Frame::<MeshOnly>::new(&MeshOnly, bounds.size());
    draw_grid(
        &mut frame,
        &camera,
        2.54,
        bounds,
        Color::WHITE,
        297.0,
        210.0,
        style,
        dpr,
        false,
    );
    let Geometry::Live { sections } = frame.into_geometry() else {
        panic!("a fresh frame is live geometry");
    };
    sections
        .iter()
        .flat_map(|section| section.meshes.iter())
        .filter_map(|mesh| match mesh {
            Mesh::Solid { buffers, .. } => Some(buffers.vertices.iter()),
            Mesh::Gradient { .. } => None,
        })
        .flatten()
        .map(|v| v.position)
        .collect()
}

fn whole_device_pixel(coordinate: f32, dpr: f32) -> bool {
    let device = coordinate * dpr;
    (device - device.round()).abs() < 1e-3
}

/// At a fractional display scale the grid's lines have every edge on a whole
/// device pixel, and its dots are centred on a device pixel middle: the call
/// sites in `draw_grid_within` really snap, which the pure helpers alone do
/// not show.
#[test]
fn the_grid_lands_on_device_pixels_at_one_and_a_half_times() {
    let dpr = 1.5;
    let lines = drawn_grid_vertices(GridStyle::Lines, dpr);
    assert!(!lines.is_empty(), "the grid drew lines");
    for [x, y] in &lines {
        assert!(
            whole_device_pixel(*x, dpr) && whole_device_pixel(*y, dpr),
            "a line vertex off the device grid: ({x}, {y})"
        );
    }

    let dots = drawn_grid_vertices(GridStyle::Dots, dpr);
    assert!(
        dots.len() >= 4 && dots.len().is_multiple_of(4),
        "{} dot vertices",
        dots.len()
    );
    let mut dots_checked = 0;
    for square in dots.chunks(4) {
        let extent = |axis: usize| {
            let values = square.iter().map(|v| v[axis]);
            values.clone().fold(f32::NEG_INFINITY, f32::max) - values.fold(f32::INFINITY, f32::min)
        };
        // The major lines drawn over the dots are long; a dot is a few px.
        if extent(0) > 8.0 || extent(1) > 8.0 {
            continue;
        }
        dots_checked += 1;
        let mid = |axis: usize| {
            let lo = square.iter().map(|v| v[axis]).fold(f32::INFINITY, f32::min);
            let hi = square
                .iter()
                .map(|v| v[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            (lo + hi) / 2.0
        };
        for centre in [mid(0), mid(1)] {
            let device = centre * dpr;
            assert!(
                ((device - 0.5) - (device - 0.5).round()).abs() < 1e-3,
                "a dot centred off a device pixel middle: {centre}"
            );
        }
    }
    assert!(dots_checked > 10, "only {dots_checked} dots were found");
}
