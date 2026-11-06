//! Pan cost on a busy sheet (#2102). `#[ignore]`d: run by hand with
//! `SIGNEX_BENCH_SHEET=<file.snxsch>` and `--ignored --nocapture`; it prints
//! min / median / p95 per stage over a run of whole-pixel pans.

use std::time::{Duration, Instant};

use iced::advanced::renderer::Style;
use iced::widget::Widget as _;
use iced::{Color, Element, Size, Theme};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_types::schematic::SchematicSheet;

use super::gpu_mount_tests::{Gpu, gpu, slot_with};
use super::tests::test_prefs;
use super::*;

const FRAMES: usize = 120;
/// About 148% of the 96 dpi mm scale.
const SCALE: f32 = 5.6;
const WINDOW: (f32, f32) = (1600.0, 1300.0);

fn load_sheet() -> Option<SchematicSheet> {
    let Some(path) = std::env::var_os("SIGNEX_BENCH_SHEET") else {
        eprintln!("SIGNEX_BENCH_SHEET is not set: nothing to measure");
        return None;
    };
    let text = std::fs::read_to_string(&path).expect("read the bench sheet");
    Some(
        signex_types::format::SnxSchematic::parse(&text)
            .expect("parse the bench sheet")
            .sheet,
    )
}

/// Camera offset that puts the top-left of the populated area a margin in.
fn offset_over(sheet: &SchematicSheet) -> iced::Point {
    let min = |f: fn(&signex_types::schematic::Symbol) -> f64| {
        sheet.symbols.iter().map(f).fold(f64::MAX, f64::min)
    };
    let (x, y) = if sheet.symbols.is_empty() {
        (0.0, 0.0)
    } else {
        (min(|s| s.position.x), min(|s| s.position.y))
    };
    iced::Point::new(20.0 - x as f32 * SCALE, 20.0 - y as f32 * SCALE)
}

fn report(label: &str, samples: &mut [Duration]) {
    samples.sort();
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q) as usize];
    eprintln!(
        "BENCH {label:<22} min {:>8.3} ms  median {:>8.3} ms  p95 {:>8.3} ms",
        at(0.0).as_secs_f64() * 1e3,
        at(0.5).as_secs_f64() * 1e3,
        at(0.95).as_secs_f64() * 1e3,
    );
}

fn build_and_draw(gpu: &mut Gpu, element: Element<'_, crate::app::Message>) -> Duration {
    let start = Instant::now();
    let mut ui = UserInterface::build(
        element,
        Size::new(WINDOW.0, WINDOW.1),
        Cache::default(),
        &mut gpu.renderer,
    );
    ui.draw(
        &mut gpu.renderer,
        &Theme::Dark,
        &Style {
            text_color: Color::WHITE,
        },
        iced::mouse::Cursor::Unavailable,
    );
    start.elapsed()
}

#[test]
#[ignore = "benchmark: needs SIGNEX_BENCH_SHEET and a GPU"]
fn pan_bench_canvas() {
    let Some(sheet) = load_sheet() else { return };
    let mut gpu = gpu();
    let mut slot = slot_with(&sheet);
    // The frame and title block the app lowers for a sheet.
    slot.sheet_frame = signex_output::template::lower_sheet_frame(
        f64::from(slot.paper_width_mm),
        f64::from(slot.paper_height_mm),
        &signex_output::template::Frame::default(),
        Some(&signex_output::template::standard_title_block()),
        |text| text.to_string(),
    );
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.device_px_per_logical_px = 1.0;
    let mut offset = offset_over(&sheet);
    let bounds = Rectangle::new(iced::Point::ORIGIN, Size::new(WINDOW.0, WINDOW.1));
    eprintln!(
        "BENCH sheet: {} symbols, {} wires, {} labels",
        sheet.symbols.len(),
        sheet.wires.len(),
        sheet.labels.len()
    );

    let (mut surface, mut ui_time, mut present, mut total) = (vec![], vec![], vec![], vec![]);
    for frame in 0..=FRAMES {
        // What a ViewMoved does: the camera moves, overlay and backdrop go stale.
        offset.x += 3.0;
        offset.y += 2.0;
        *slot.camera_mut() = Camera {
            offset,
            scale: SCALE,
        };
        slot.clear_overlay_cache();
        slot.clear_bg_cache();
        let whole = Instant::now();

        let t = Instant::now();
        {
            let canvas = SchematicCanvas::new(&slot, prefs);
            let state = CanvasState::default();
            let renderer = &gpu.renderer;
            let layers =
                canvas.draw_surface(&state, renderer, bounds, mouse::Cursor::Unavailable, true);
            std::hint::black_box(layers.layers.len());
        }
        let a = t.elapsed();

        let element: Element<'_, crate::app::Message> =
            SchematicSurface::new(SchematicCanvas::new(&slot, prefs)).boxed();
        let b = build_and_draw(&mut gpu, element);

        let t = Instant::now();
        let viewport = iced::advanced::graphics::Viewport::with_physical_size(
            Size::new(WINDOW.0 as u32, WINDOW.1 as u32),
            iced::advanced::renderer::Scale::default(),
        );
        let iced::Renderer::Primary(r) = &mut gpu.renderer else {
            unreachable!("the harness builds a wgpu renderer")
        };
        std::hint::black_box(r.screenshot(&viewport, Color::BLACK));
        let c = t.elapsed();

        if frame > 0 {
            surface.push(a);
            ui_time.push(b);
            present.push(c);
            total.push(whole.elapsed());
        }
    }
    report("draw_surface", &mut surface);
    report("ui build+draw", &mut ui_time);
    report("present/screenshot", &mut present);
    report("total", &mut total);
    eprintln!(
        "BENCH content rebuilds over {} frames: {}",
        FRAMES + 1,
        slot.content_surface.borrow().rebuilds()
    );
}

#[test]
#[ignore = "benchmark: needs SIGNEX_BENCH_SHEET and a GPU"]
fn pan_bench_app_view() {
    use crate::app::{Signex, TabInfo, TabKind};
    let Some(sheet) = load_sheet() else { return };
    let mut gpu = gpu();
    let path = std::path::PathBuf::from("bench.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "bench".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    app.ui_state.window_size = WINDOW;

    let (mut view, mut full) = (vec![], vec![]);
    for _ in 0..FRAMES {
        let t = Instant::now();
        std::hint::black_box(app.view(window));
        view.push(t.elapsed());
        full.push(build_and_draw(&mut gpu, app.view(window)));
    }
    report("Signex::view (build)", &mut view);
    report("view+layout+draw", &mut full);
}
