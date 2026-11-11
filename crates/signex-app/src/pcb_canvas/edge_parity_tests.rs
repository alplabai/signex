//! The board edge drawn by the overlay pass lands on the board's Edge Cuts
//! graphics at every zoom and after Fit (#2275). The thin line 0.6 mm inside
//! it on `mixed-tht-smd` is the GND pour on the bottom copper, whose outline
//! is pulled back 0.6 mm from the edge in the board data; its screen gap grows
//! with the zoom, which a second transform would not do.

use iced::{Point, Rectangle};
use signex_gfx::style::ColorSlot;
use signex_renderer::pcb::PcbSnapshot;
use signex_renderer::theme::ResolvedTheme;
use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;

use super::{PcbCanvas, world_to_screen};

/// The window the Graphite screens use for the PCB canvas, near enough.
const VIEWPORT: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 970.0,
    height: 540.0,
};
/// The pour's pull-back from the board edge in the corpus board, in mm.
const POUR_INSET_MM: f32 = 0.6;

fn mixed_tht_smd() -> PcbBoard {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real/mixed-tht-smd.snxpcb");
    let text = std::fs::read_to_string(path).expect("corpus file");
    SnxPcb::parse(&text).expect("board parses").board
}

fn fitted_canvas(board: &PcbBoard) -> PcbCanvas {
    let mut canvas = PcbCanvas::new();
    canvas.set_renderer_snapshot(Some(PcbSnapshot::from_board(board)));
    canvas.set_viewport(VIEWPORT);
    canvas.fit_to_board();
    // What `update` does with a pending fit on the next canvas event.
    let target = canvas.pending_fit.take().expect("the board has bounds");
    canvas.camera.borrow_mut().fit_rect(target, VIEWPORT);
    canvas
}

fn screen(canvas: &PcbCanvas, world: [f32; 2]) -> Point {
    world_to_screen(&canvas.camera.borrow(), VIEWPORT, world)
}

fn close(a: Point, b: Point, tolerance_px: f32) -> bool {
    (a.x - b.x).abs() <= tolerance_px && (a.y - b.y).abs() <= tolerance_px
}

/// The corner furthest along +x+y.
fn bottom_right(points: impl Iterator<Item = [f32; 2]>) -> [f32; 2] {
    points
        .max_by(|a, b| (a[0] + a[1]).total_cmp(&(b[0] + b[1])))
        .expect("points")
}

#[test]
fn board_edge_overlay_lands_on_the_edge_cuts_graphics_after_fit_and_zoom() {
    let board = mixed_tht_smd();
    let edge_cuts: Vec<[f32; 2]> = board
        .graphics
        .iter()
        .filter(|g| g.layer == "EdgeCuts")
        .flat_map(|g| [g.start, g.end])
        .flatten()
        .map(|p| [p.x as f32, p.y as f32])
        .collect();
    assert_eq!(edge_cuts.len(), 10, "four lines and an arc, two ends each");

    let canvas = fitted_canvas(&board);
    let scene = canvas.gpu_scene().expect("scene");
    let edge_color =
        ResolvedTheme::from_canvas_colors(canvas.canvas_colors).color(ColorSlot::BoardEdge);
    let overlay_edge: Vec<[f32; 2]> = scene
        .overlay_lines
        .iter()
        .filter(|l| l.color == edge_color)
        .flat_map(|l| [l.p0, l.p1])
        .collect();
    assert!(!overlay_edge.is_empty(), "the overlay draws the board edge");
    let pour = scene
        .polygons
        .iter()
        .max_by(|a, b| a.vertices.len().cmp(&b.vertices.len()))
        .expect("the GND pour is in the main polygon bucket");

    let fit_scale = canvas.live_camera().2;
    // Fit, two notches in, then three out: three different scales.
    let steps: [(bool, usize); 3] = [(true, 0), (true, 2), (false, 3)];
    let mut scales = Vec::new();
    for (zoom_in, notches) in steps {
        for _ in 0..notches {
            assert!(canvas.zoom_about_centre(zoom_in, canvas.zoom_step));
        }
        let scale = canvas.live_camera().2;
        scales.push(scale);

        for &end in &edge_cuts {
            let want = screen(&canvas, end);
            assert!(
                overlay_edge
                    .iter()
                    .any(|&p| close(screen(&canvas, p), want, 0.01)),
                "Edge Cuts end {end:?} at {want:?} has no board-edge vertex on it at {scale} px/mm"
            );
        }

        // The inner line is the pour, 0.6 mm in on both axes at every scale.
        let edge_corner = screen(&canvas, bottom_right(overlay_edge.iter().copied()));
        let pour_corner = screen(&canvas, bottom_right(pour.vertices.iter().copied()));
        let gap = POUR_INSET_MM * scale;
        assert!(
            close(
                pour_corner,
                Point::new(edge_corner.x - gap, edge_corner.y - gap),
                0.01,
            ),
            "pour corner {pour_corner:?} is not {gap} px inside the edge corner {edge_corner:?}"
        );
    }
    assert_eq!(scales[0], fit_scale);
    assert_ne!(scales[0], scales[1]);
    assert_ne!(scales[1], scales[2]);
}

#[test]
fn fit_shows_the_whole_board_edge() {
    let canvas = fitted_canvas(&mixed_tht_smd());
    for corner in [[0.0, 0.0], [50.0, 0.0], [50.0, 40.0], [0.0, 40.0]] {
        assert!(VIEWPORT.contains(screen(&canvas, corner)), "{corner:?}");
    }
}
