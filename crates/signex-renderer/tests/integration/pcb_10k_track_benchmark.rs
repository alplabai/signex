//! 10,000-track PCB render baseline (#2176): emit time and draw-call count.
//!
//! The time bounds are loose on purpose (a debug build on a loaded CI runner
//! must pass); the printed numbers are the baseline. How to run and read it:
//! `docs/architecture/pcb-render-bench.md`.

use super::pcb_10k_track_fixture::{
    BOARD_HEIGHT_NM, BOARD_WIDTH_NM, FOOTPRINT_COUNT, PAD_COUNT, TRACK_COUNT, VIA_COUNT,
    ZONE_COUNT, board_10k_tracks,
};
use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::TextItem;
use signex_gfx::scene::{
    DirtyFlags, GPU_SCENE_DRAW_ORDER, Scene, SceneBucket, SceneUploadTarget, TextUploadParams,
    UploadCulling, ViewportAabbMm, apply_dirty_uploads_with_culling,
};
use signex_renderer::pcb::{PcbRenderer, PcbSnapshot};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;
use signex_types::format::SnxPcb;
use std::time::{Duration, Instant};

/// Timed repetitions; the median is reported and bounded.
const RUNS: usize = 7;
/// Ceiling for one snapshot + full scene build of the board. A release build
/// takes a few milliseconds; this only catches an order-of-magnitude
/// regression such as a quadratic pass.
const EMIT_BOUND: Duration = Duration::from_secs(2);
/// Ceiling for one viewport-culled upload pass over the built scene.
const CULL_BOUND: Duration = Duration::from_secs(2);

const ALL_GEOMETRY: DirtyFlags = DirtyFlags::LINES
    .union(DirtyFlags::CIRCLES)
    .union(DirtyFlags::POLYGONS);

/// Overlay buckets drawn in the GPU path's second pass, after
/// [`GPU_SCENE_DRAW_ORDER`].
const GPU_OVERLAY_BUCKETS: [SceneBucket; 3] = [
    SceneBucket::OverlayLines,
    SceneBucket::OverlayCircles,
    SceneBucket::OverlayPolygons,
];

fn bucket_len(scene: &Scene, bucket: SceneBucket) -> usize {
    match bucket {
        SceneBucket::Lines => scene.lines.len(),
        SceneBucket::Circles => scene.circles.len(),
        SceneBucket::Arcs => scene.arcs.len(),
        SceneBucket::Polygons => scene.polygons.len(),
        SceneBucket::TopPolygons => scene.top_polygons.len(),
        SceneBucket::Drawings => scene.drawings.len(),
        SceneBucket::DrawingsFront => scene.drawings_front.len(),
        SceneBucket::Texts => scene.texts.len(),
        SceneBucket::OverlayLines => scene.overlay_lines.len(),
        SceneBucket::OverlayCircles => scene.overlay_circles.len(),
        SceneBucket::OverlayPolygons => scene.overlay_polygons.len(),
        SceneBucket::ErcMarkerLines => scene.erc_marker_lines.len(),
        SceneBucket::ErcMarkerCircles => scene.erc_marker_circles.len(),
        SceneBucket::ErcMarkerPolygons => scene.erc_marker_polygons.len(),
    }
}

/// Draw calls the GPU scene shader issues for `scene`: each non-empty
/// instanced bucket is one `draw`. The drawing buckets are drawn as several
/// runs, so the caller asserts they are empty before trusting this count.
fn gpu_draw_calls(scene: &Scene) -> usize {
    GPU_SCENE_DRAW_ORDER
        .iter()
        .chain(GPU_OVERLAY_BUCKETS.iter())
        .filter(|bucket| bucket_len(scene, **bucket) > 0)
        .count()
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn build_scene() -> Scene {
    let board = board_10k_tracks();
    let snapshot = PcbSnapshot::from_board(&board);
    let mut scene = Scene::default();
    PcbRenderer::build_scene(
        &snapshot,
        &ResolvedTheme::builtin_default(),
        ALL_GEOMETRY,
        &mut scene,
    );
    scene
}

#[test]
fn pcb_10k_fixture_has_the_counts_it_claims() {
    let board = board_10k_tracks();
    assert_eq!(board.segments.len(), TRACK_COUNT);
    assert_eq!(TRACK_COUNT, 10_000);
    assert_eq!(board.vias.len(), VIA_COUNT);
    assert_eq!(board.footprints.len(), FOOTPRINT_COUNT);
    let pads: usize = board.footprints.iter().map(|f| f.pads.len()).sum();
    assert_eq!(pads, PAD_COUNT);
    assert_eq!(board.zones.len(), ZONE_COUNT);

    let width_mm = BOARD_WIDTH_NM as f64 / 1_000_000.0;
    let height_mm = BOARD_HEIGHT_NM as f64 / 1_000_000.0;
    for segment in &board.segments {
        for p in [segment.start, segment.end] {
            assert!((0.0..=width_mm).contains(&p.x) && (0.0..=height_mm).contains(&p.y));
        }
    }
}

#[test]
fn pcb_10k_fixture_reopens_from_snxpcb_unchanged() {
    let board = board_10k_tracks();
    let started = Instant::now();
    let text = SnxPcb::new(board.clone())
        .write_string()
        .expect("the generated board saves");
    let written = started.elapsed();
    let started = Instant::now();
    let mut reopened = SnxPcb::parse(&text).expect("the saved board reopens").board;
    let parsed = started.elapsed();
    eprintln!(
        "pcb_10k snxpcb: {} bytes, write {written:?}, parse {parsed:?}",
        text.len()
    );

    assert!(
        reopened.take_load_repairs().is_empty(),
        "a generated board needs no repair on open"
    );
    assert_eq!(reopened.segments, board.segments);
    assert_eq!(reopened.vias, board.vias);
    assert_eq!(reopened.footprints, board.footprints);
    assert_eq!(reopened.zones, board.zones);
}

#[test]
fn pcb_10k_emit_counts_and_draw_calls() {
    let scene = build_scene();

    assert_eq!(scene.lines.len(), TRACK_COUNT);
    // Each via is its copper ring and its drill opening.
    assert_eq!(scene.circles.len(), 2 * VIA_COUNT);
    assert_eq!(scene.top_polygons.len(), PAD_COUNT);
    // The board area under the two pours.
    assert_eq!(scene.polygons.len(), ZONE_COUNT + 1);
    assert!(scene.arcs.is_empty() && scene.texts.is_empty());
    assert!(scene.drawings.is_empty() && scene.drawings_front.is_empty());

    // Instancing: zones, traces, vias and pads are one draw each, however
    // many tracks the board has.
    assert_eq!(gpu_draw_calls(&scene), 4);
}

#[test]
fn pcb_10k_emit_time_is_bounded() {
    let board = board_10k_tracks();
    let theme = ResolvedTheme::builtin_default();
    let mut samples = Vec::with_capacity(RUNS);
    let mut scene = Scene::default();
    for _ in 0..RUNS {
        let started = Instant::now();
        let snapshot = PcbSnapshot::from_board(&board);
        PcbRenderer::build_scene(&snapshot, &theme, ALL_GEOMETRY, &mut scene);
        samples.push(started.elapsed());
    }
    assert_eq!(scene.lines.len(), TRACK_COUNT);

    let emit = median(samples);
    eprintln!(
        "pcb_10k emit (snapshot + scene): median {emit:?} over {RUNS} runs, {} draw calls",
        gpu_draw_calls(&scene)
    );
    assert!(
        emit < EMIT_BOUND,
        "emit took {emit:?}, bound {EMIT_BOUND:?}"
    );
}

/// Records how many primitives each culled upload keeps.
#[derive(Default)]
struct KeptCounts {
    lines: usize,
    circles: usize,
    polygons: usize,
    top_polygons: usize,
}

impl SceneUploadTarget for KeptCounts {
    type TextError = ();
    fn upload_lines(&mut self, lines: &[LineSegment]) {
        self.lines = lines.len();
    }
    fn upload_circles(&mut self, circles: &[Circle]) {
        self.circles = circles.len();
    }
    fn upload_arcs(&mut self, _: &[Arc]) {}
    fn upload_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.polygons = polygons.len();
    }
    fn upload_top_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.top_polygons = polygons.len();
    }
    fn upload_drawings(&mut self, _: &[OrderedItem]) {}
    fn upload_drawings_front(&mut self, _: &[OrderedItem]) {}
    fn upload_texts(&mut self, _: &[TextItem], _: TextUploadParams) -> Result<(), ()> {
        Ok(())
    }
    fn refresh_grid(&mut self) {}
    fn upload_overlay_lines(&mut self, _: &[LineSegment]) {}
    fn upload_overlay_circles(&mut self, _: &[Circle]) {}
    fn upload_overlay_polygons(&mut self, _: &[GpuPolygon]) {}
    fn upload_erc_marker_lines(&mut self, _: &[LineSegment]) {}
    fn upload_erc_marker_circles(&mut self, _: &[Circle]) {}
    fn upload_erc_marker_polygons(&mut self, _: &[GpuPolygon]) {}
    fn refresh_theme(&mut self) {}
}

#[test]
fn pcb_10k_zoomed_viewport_culls_most_tracks() {
    let scene = build_scene();
    // A 20 x 20 mm window in the middle of the 205 x 130 mm board.
    let culling = UploadCulling::viewport(ViewportAabbMm::new([92.0, 55.0], [112.0, 75.0]));

    let mut samples = Vec::with_capacity(RUNS);
    let mut kept = KeptCounts::default();
    for _ in 0..RUNS {
        kept = KeptCounts::default();
        let started = Instant::now();
        apply_dirty_uploads_with_culling(
            &scene,
            ALL_GEOMETRY,
            &mut kept,
            TextUploadParams::default(),
            culling,
        )
        .expect("no text to upload");
        samples.push(started.elapsed());
    }
    let cull = median(samples);
    eprintln!(
        "pcb_10k cull (20 x 20 mm): median {cull:?}, kept {} lines, {} via circles (ring + drill), {} pads, {} zones",
        kept.lines, kept.circles, kept.top_polygons, kept.polygons
    );

    // About 17 rows of 11 to 12 segments cross the window.
    assert!(
        (100..=400).contains(&kept.lines),
        "kept {} lines",
        kept.lines
    );
    assert!(
        kept.circles <= 2 * (VIA_COUNT / 10),
        "kept {} via circles",
        kept.circles
    );
    // Pads sit at the board edges; both pours cover the window.
    assert_eq!(kept.top_polygons, 0);
    assert_eq!(
        kept.polygons,
        ZONE_COUNT + 1,
        "both pours and the board area"
    );
    assert!(
        cull < CULL_BOUND,
        "cull took {cull:?}, bound {CULL_BOUND:?}"
    );
}

/// Writes the board as a `.snxpcb` to `SIGNEX_PCB_BENCH_OUT`, to open it in the
/// app by hand. Ignored: it only runs when asked for.
#[test]
#[ignore = "writes a file; set SIGNEX_PCB_BENCH_OUT and run with --ignored"]
fn pcb_10k_write_snxpcb() {
    let Some(path) = std::env::var_os("SIGNEX_PCB_BENCH_OUT") else {
        eprintln!("SIGNEX_PCB_BENCH_OUT is not set; nothing written");
        return;
    };
    let text = SnxPcb::new(board_10k_tracks())
        .write_string()
        .expect("the generated board saves");
    let path = std::path::PathBuf::from(path);
    signex_types::atomic_io::atomic_write(&path, text.as_bytes()).expect("write the board");
    eprintln!("wrote {} ({} bytes)", path.display(), text.len());
}
