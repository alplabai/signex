//! The PCB canvas on a real GPU: the 10,000-track board at 1600 x 1300, for
//! the exit criterion "10k-track board at 60 fps" (#2176). `#[ignore]`d: run
//! by hand, see `docs/architecture/pcb-render-bench.md`, "GPU frames".
//!
//! Mounts the widgets `app/view/mod.rs` mounts for a board: the `PcbCanvas`
//! canvas, the `SceneShaderProgram<PcbSurface>` shader when GPU render is on,
//! and the highlight layer, in the same stack. Pan and zoom are real mouse
//! events through `UserInterface::update`; a frame with events builds the
//! tree again, as the app's `view` does after the message. Then `draw`, and
//! `iced_wgpu::Renderer::screenshot` uploads, renders and reads back, so the
//! GPU work is inside the timing. Never tiny-skia.
//!
//! ```text
//! cargo test -p signex-app --release --lib pcb_frame_bench -- --ignored --nocapture --test-threads=1
//! ```

use std::time::{Duration, Instant};

use iced::advanced::graphics::Antialiasing;
use iced::advanced::renderer::Style;
use iced::widget::{canvas, shader, stack};
use iced::{Color, Element, Event, Length, Point, Rectangle, Size, Theme, mouse};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_renderer::pcb::{PcbLayerView, PcbSnapshot};

use super::PcbCanvas;
use crate::app::Message;
use crate::canvas::render_bench_tests::{Rig, WINDOW, rig};
use crate::scene_shader::{PcbSurface, SceneShaderProgram};

// The one generator the renderer's own 10k tests use, included, not copied.
#[allow(
    dead_code,
    reason = "this bench reads only some of the fixture's counts"
)]
#[path = "../../../signex-renderer/tests/integration/pcb_10k_track_fixture.rs"]
mod fixture;

/// A 60 fps frame budget, in ms.
const BUDGET_MS: f64 = 16.7;
/// Frames rendered before a scenario is timed: pipelines, atlas, buffers.
const WARM_UP: usize = 3;
const PAN_FRAMES: usize = 60;
/// Wheel notches in, then the same number out.
const ZOOM_NOTCHES: usize = 15;
const IDLE_FRAMES: usize = 60;
/// Cursor step per pan frame, px.
const PAN_STEP: (f32, f32) = (6.0, 3.0);
/// Side of the zoomed-in view, mm.
const DETAIL_MM: f32 = 20.0;

// --- the mount -------------------------------------------------------------

/// The board's stack as `app/view/mod.rs` builds it, without the active bar
/// and the layer strip (chrome, not the board).
fn view(pcb: &PcbCanvas) -> Element<'_, Message> {
    let base = canvas(pcb).width(Length::Fill).height(Length::Fill);
    let highlight = canvas(pcb.highlight_layer())
        .width(Length::Fill)
        .height(Length::Fill);
    if !pcb.gpu_render {
        return stack![base, highlight].into();
    }
    let scene = pcb.gpu_scene().unwrap_or_default();
    let generation = pcb.scene_generation();
    let (offset_x, offset_y, scale) = pcb.live_camera();
    let content = shader(SceneShaderProgram::<PcbSurface>::new(
        scene,
        Some(generation),
        [offset_x, offset_y],
        scale,
    ))
    .width(Length::Fill)
    .height(Length::Fill);
    stack![base, content, highlight].into()
}

// --- one frame -------------------------------------------------------------

#[derive(Default)]
struct Phases {
    build: Vec<Duration>,
    update: Vec<Duration>,
    draw: Vec<Duration>,
    present: Vec<Duration>,
    total: Vec<Duration>,
}

struct Frame {
    times: [Duration; 5],
    pixels: Vec<u8>,
}

impl Phases {
    fn push(&mut self, f: &Frame) {
        let [build, update, draw, present, total] = f.times;
        self.build.push(build);
        self.update.push(update);
        self.draw.push(draw);
        self.present.push(present);
        self.total.push(total);
    }
}

/// `(median, p95, max)` in ms.
fn stats(v: &[Duration]) -> (f64, f64, f64) {
    let mut v = v.to_vec();
    v.sort();
    let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize].as_secs_f64() * 1e3;
    (at(0.5), at(0.95), at(1.0))
}

fn frame(
    rig: &mut Rig,
    dpr: f32,
    pcb: &PcbCanvas,
    cache: &mut Option<Cache>,
    events: &[Event],
    cursor: mouse::Cursor,
) -> Frame {
    let whole = Instant::now();
    let size = Size::new(WINDOW.0, WINDOW.1);
    let t = Instant::now();
    let mut ui = UserInterface::build(
        view(pcb),
        size,
        cache.take().unwrap_or_default(),
        &mut rig.renderer,
    );
    let mut build = t.elapsed();

    let t = Instant::now();
    if !events.is_empty() {
        let mut bus = iced::advanced::shell::Bus::new();
        let waker = iced::advanced::shell::Waker::noop();
        std::hint::black_box(ui.update(
            &iced::window::Headless,
            &waker,
            events,
            cursor,
            &mut rig.renderer,
            &mut bus,
        ));
    }
    let update = t.elapsed();

    // The canvas published a message and moved the camera: the app's `view`
    // runs again before the frame is drawn.
    if !events.is_empty() {
        let t = Instant::now();
        let kept = ui.into_cache();
        ui = UserInterface::build(view(pcb), size, kept, &mut rig.renderer);
        build += t.elapsed();
    }

    let t = Instant::now();
    ui.draw(
        &mut rig.renderer,
        &Theme::Dark,
        &Style {
            text_color: Color::WHITE,
        },
        cursor,
    );
    let draw = t.elapsed();
    *cache = Some(ui.into_cache());

    let t = Instant::now();
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(
        Size::new((WINDOW.0 * dpr) as u32, (WINDOW.1 * dpr) as u32),
        iced::advanced::renderer::Scale {
            window: dpr,
            application: 1.0,
        },
    );
    let iced::Renderer::Primary(r) = &mut rig.renderer else {
        unreachable!("built as Primary")
    };
    let pixels = std::hint::black_box(r.screenshot(&viewport, Color::BLACK));
    let present = t.elapsed();
    Frame {
        times: [build, update, draw, present, whole.elapsed()],
        pixels,
    }
}

// --- scenarios -------------------------------------------------------------

fn centre() -> Point {
    Point::new(WINDOW.0 / 2.0, WINDOW.1 / 2.0)
}

fn moved(p: Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: p })
}

fn board_rect() -> Rectangle {
    let mm = |nm: i64| nm as f32 * 1.0e-6;
    Rectangle::new(
        Point::ORIGIN,
        Size::new(mm(fixture::BOARD_WIDTH_NM), mm(fixture::BOARD_HEIGHT_NM)),
    )
}

/// A `DETAIL_MM` square in the middle of the board.
fn detail_rect() -> Rectangle {
    let board = board_rect();
    let c = board.center();
    Rectangle::new(
        Point::new(c.x - DETAIL_MM / 2.0, c.y - DETAIL_MM / 2.0),
        Size::new(DETAIL_MM, DETAIL_MM),
    )
}

struct Bench<'a> {
    rig: &'a mut Rig,
    dpr: f32,
    pcb: &'a PcbCanvas,
    cache: Option<Cache>,
}

impl Bench<'_> {
    fn frame(&mut self, events: &[Event], cursor: mouse::Cursor) -> Frame {
        frame(
            self.rig,
            self.dpr,
            self.pcb,
            &mut self.cache,
            events,
            cursor,
        )
    }

    /// Fit `world` into the window the way the app fits a board: the next
    /// canvas event takes `pending_fit`. Then warm up and check the frame.
    fn settle(&mut self, world: Rectangle) {
        self.pcb.pending_fit.set(Some(world));
        let c = centre();
        self.frame(&[moved(c)], mouse::Cursor::Available(c));
        assert!(
            self.pcb.pending_fit.get().is_none(),
            "the canvas took the fit"
        );
        let mut last = None;
        for _ in 0..WARM_UP {
            last = Some(self.frame(&[], mouse::Cursor::Unavailable));
        }
        let pixels = last.expect("warm-up frames").pixels;
        let expected = (WINDOW.0 * self.dpr) as usize * (WINDOW.1 * self.dpr) as usize * 4;
        assert_eq!(pixels.len(), expected, "the readback covers the window");
        let colours: std::collections::HashSet<&[u8]> = pixels.chunks_exact(4).collect();
        assert!(
            colours.len() >= 3,
            "the frame shows the board, not a flat fill: {} colours",
            colours.len()
        );
    }

    fn timed(
        &mut self,
        steps: usize,
        mut step: impl FnMut(usize) -> (Vec<Event>, Point),
    ) -> Phases {
        let mut phases = Phases::default();
        for i in 0..steps {
            let (events, at) = step(i);
            let f = self.frame(&events, mouse::Cursor::Available(at));
            phases.push(&f);
        }
        assert_eq!(phases.total.len(), steps, "one sample per frame");
        phases
    }

    /// A right-button drag over `PAN_FRAMES` frames; the press and release
    /// frames are not timed. Returns the timing and the camera's travel.
    fn pan(&mut self) -> (Phases, (f32, f32)) {
        let start = centre();
        let before = self.pcb.live_camera();
        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
        self.frame(&[moved(start), press], mouse::Cursor::Available(start));
        let mut p = start;
        let phases = self.timed(PAN_FRAMES, |_| {
            p = Point::new(p.x + PAN_STEP.0, p.y + PAN_STEP.1);
            (vec![moved(p)], p)
        });
        let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right));
        self.frame(&[release], mouse::Cursor::Available(p));
        let after = self.pcb.live_camera();
        (phases, (after.0 - before.0, after.1 - before.1))
    }
}

fn wheel(notches: f32) -> Event {
    Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y: notches },
    })
}

fn report(path: &str, aa: &str, scenario: &str, p: &Phases, note: &str) {
    let cell = |v: &[Duration]| {
        let (median, p95, _) = stats(v);
        format!("{median:6.2}/{p95:6.2}")
    };
    let (median, p95, max) = stats(&p.total);
    eprintln!(
        "BENCH pcb {path:<3} aa={aa:<5} {scenario:<9} frames {:>3} median {median:6.2} ms p95 \
         {p95:6.2} ms max {max:6.2} ms fps {:6.1} (p95 {:6.1}) {} | build {} update {} draw {} \
         present {} | {note}",
        p.total.len(),
        1e3 / median,
        1e3 / p95,
        if p95 <= BUDGET_MS { "ok" } else { "OVER" },
        cell(&p.build),
        cell(&p.update),
        cell(&p.draw),
        cell(&p.present),
    );
}

fn camera_note(pcb: &PcbCanvas) -> String {
    let (x, y, scale) = pcb.live_camera();
    format!(
        "camera offset ({x:.1}, {y:.1}) px, {scale:.2} px/mm, view {:.1} x {:.1} mm",
        WINDOW.0 / scale,
        WINDOW.1 / scale
    )
}

/// The four scenarios on one canvas, `gpu` choosing the mount.
fn measure(rig: &mut Rig, dpr: f32, snapshot: &PcbSnapshot, gpu: bool, aa: &str) {
    let mut pcb = PcbCanvas::new();
    pcb.gpu_render = gpu;
    pcb.set_renderer_snapshot(Some(snapshot.clone()));
    let path = if gpu { "gpu" } else { "cpu" };
    let mut bench = Bench {
        rig,
        dpr,
        pcb: &pcb,
        cache: None,
    };

    // 1. Idle: the whole board, nothing moves.
    bench.settle(board_rect());
    let idle = bench.timed(IDLE_FRAMES, |_| (vec![], centre()));
    report(path, aa, "idle", &idle, &camera_note(&pcb));

    // 2. Pan: a right-button drag across the whole board.
    let (pan, travel) = bench.pan();
    let want = (
        PAN_STEP.0 * PAN_FRAMES as f32,
        PAN_STEP.1 * PAN_FRAMES as f32,
    );
    assert!(
        (travel.0 - want.0).abs() < 0.5 && (travel.1 - want.1).abs() < 0.5,
        "the drag moved the view by {travel:?}, wanted {want:?}"
    );
    report(path, aa, "pan", &pan, &camera_note(&pcb));

    // 3. Zoom: wheel notches in about the centre, then as many out.
    bench.settle(board_rect());
    let start = pcb.live_camera().2;
    let mut peak = start;
    let zoom = bench.timed(ZOOM_NOTCHES * 2, |i| {
        if i == ZOOM_NOTCHES {
            peak = pcb.live_camera().2;
        }
        let notch = if i < ZOOM_NOTCHES { 1.0 } else { -1.0 };
        (vec![wheel(notch)], centre())
    });
    let end = pcb.live_camera().2;
    assert!(peak > start * 2.0, "zoomed in: {start} -> {peak} px/mm");
    assert!(
        (end - start).abs() < start * 1e-3,
        "zoomed back out: {start} -> {end} px/mm"
    );
    let note = format!("{start:.2} -> {peak:.2} -> {end:.2} px/mm");
    report(path, aa, "zoom", &zoom, &note);

    // 4. Zoomed in on a 20 x 20 mm patch, then a drag pan there.
    bench.settle(detail_rect());
    let detail_scale = pcb.live_camera().2;
    let (detail, travel) = bench.pan();
    assert!(
        (travel.0 - want.0).abs() < 0.5 && (travel.1 - want.1).abs() < 0.5,
        "the detail drag moved the view by {travel:?}, wanted {want:?}"
    );
    let note = format!(
        "fit {DETAIL_MM} mm -> {detail_scale:.2} px/mm (the zoom limit may clamp it); {}",
        camera_note(&pcb)
    );
    report(path, aa, "zoom-pan", &detail, &note);
}

#[test]
#[ignore = "benchmark: needs a GPU and a release build"]
fn pcb_frame_bench() {
    let dpr: f32 = std::env::var("SIGNEX_BENCH_DPR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let list = |var: &str, default: &str| -> Vec<String> {
        std::env::var(var)
            .unwrap_or_else(|_| default.into())
            .split(',')
            .map(str::to_owned)
            .collect()
    };
    let paths = list("SIGNEX_BENCH_PATHS", "cpu,gpu");
    let aas = list("SIGNEX_BENCH_AA", "off,msaa4");

    let board = fixture::board_10k_tracks();
    let t = Instant::now();
    let snapshot = PcbSnapshot::from_board_with_view(&board, &PcbLayerView::default());
    let lowered = t.elapsed();
    assert_eq!(snapshot.traces.len(), fixture::TRACK_COUNT);
    assert_eq!(snapshot.vias.len(), fixture::VIA_COUNT);
    assert_eq!(snapshot.pads.len(), fixture::PAD_COUNT);
    assert_eq!(snapshot.zones.len(), fixture::ZONE_COUNT);
    eprintln!(
        "BENCH pcb board: {} tracks, {} vias, {} pads, {} zones; snapshot {:.2} ms; \
         window {}x{} logical at dpr {dpr}; budget {BUDGET_MS} ms (60 fps) on p95",
        snapshot.traces.len(),
        snapshot.vias.len(),
        snapshot.pads.len(),
        snapshot.zones.len(),
        lowered.as_secs_f64() * 1e3,
        WINDOW.0,
        WINDOW.1,
    );

    let mut ran = 0;
    for aa_name in aas.iter().map(String::as_str) {
        let aa = (aa_name == "msaa4").then_some(Antialiasing::MSAAx4);
        for (gpu, name) in [(false, "cpu"), (true, "gpu")] {
            if !paths.iter().any(|p| p == name) {
                continue;
            }
            let mut rig = rig(aa, dpr);
            eprintln!("BENCH adapter (name, backend): {}", rig.adapter);
            measure(&mut rig, dpr, &snapshot, gpu, aa_name);
            ran += 1;
        }
    }
    assert!(
        ran > 0,
        "SIGNEX_BENCH_PATHS / SIGNEX_BENCH_AA picked nothing"
    );
}
