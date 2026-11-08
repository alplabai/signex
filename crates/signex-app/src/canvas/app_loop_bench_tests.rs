//! The whole app loop, measured per frame: widget `update` (real events),
//! `Signex::update` for every message they publish plus the idle-cursor
//! message the window subscription sends, `Signex::view`, widget `draw`,
//! and the pixel readback. `#[ignore]`d: run by hand with
//!
//! ```text
//! SIGNEX_BENCH_SIZE=medium cargo test -p signex-app --profile fast-release --lib \
//!     app_loop_bench -- --ignored --nocapture --test-threads=1
//! ```
//!
//! `render_bench_tests` drives the canvas widget alone; the work `update` does
//! per move message (hover hit test, ghost refresh, cache clears) is not in it.

use std::time::{Duration, Instant};

use iced::advanced::renderer::Style;
use iced::{Color, Event, Size, Theme};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;

use super::render_bench_tests::{Generated, Variant, WINDOW, WORK_SCALE, generate, rig};
use super::*;
use crate::app::{Message, Signex, TabInfo, TabKind, UiMsg};

struct Frame {
    view: Duration,
    ui_update: Duration,
    app_update: Duration,
    draw: Duration,
    present: Duration,
}

impl Frame {
    fn total(&self) -> Duration {
        self.view + self.ui_update + self.app_update + self.draw + self.present
    }
}

fn app_with(g: &Generated, keep_wires: bool) -> (Signex, iced::window::Id) {
    let path = std::path::PathBuf::from("bench.snxsch");
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.move_keeps_wires = keep_wires;
    app.ui_state.wheel_mode = WheelMode::Scroll;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(g.sheet.clone()).expect("engine"),
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
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&g.sheet, 1),
    ));
    canvas.paper_width_mm = g.paper.0;
    canvas.paper_height_mm = g.paper.1;
    let (hx, hy) = g.handle;
    *canvas.camera_mut() = Camera {
        offset: iced::Point::new(
            WINDOW.0 / 2.0 - hx as f32 * WORK_SCALE,
            WINDOW.1 / 2.0 - hy as f32 * WORK_SCALE,
        ),
        scale: WORK_SCALE,
    };
    (app, window)
}

/// One frame: events through the widget tree, every published message (plus
/// the subscription's idle-cursor message) through `Signex::update`, then draw.
fn frame(
    app: &mut Signex,
    window: iced::window::Id,
    rig: &mut super::render_bench_tests::Rig,
    cache: &mut Option<Cache>,
    events: &[Event],
    cursor: iced::mouse::Cursor,
) -> Frame {
    let size = Size::new(WINDOW.0, WINDOW.1);
    let t = Instant::now();
    let element = app.view(window);
    let view = t.elapsed();
    let mut ui = UserInterface::build(
        element,
        size,
        cache.take().unwrap_or_default(),
        &mut rig.renderer,
    );

    let t = Instant::now();
    let mut bus = iced::advanced::shell::Bus::new();
    if !events.is_empty() {
        let waker = iced::advanced::shell::Waker::noop();
        std::hint::black_box(ui.update(
            &iced::window::Headless,
            &waker,
            events,
            cursor,
            &mut rig.renderer,
            &mut bus,
        ));
        // The runtime ends a frame with the redraw request, then draws.
        let redraw = [Event::Window(iced::window::Event::RedrawRequested(
            Instant::now(),
        ))];
        std::hint::black_box(ui.update(
            &iced::window::Headless,
            &waker,
            &redraw,
            cursor,
            &mut rig.renderer,
            &mut bus,
        ));
    }
    let ui_update = t.elapsed();
    // Messages hold no borrow of the tree; end it before `update` mutates the app.
    let mut messages: Vec<Message> = bus.into_iter().collect();
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

    for event in events {
        if let Event::Mouse(iced::mouse::Event::CursorMoved { position }) = event {
            messages.push(Message::Ui(UiMsg::DragMove(position.x, position.y)));
        }
    }
    let t = Instant::now();
    for message in messages {
        let _ = std::hint::black_box(app.update(message));
    }
    let app_update = t.elapsed();

    let t = Instant::now();
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(
        Size::new(WINDOW.0 as u32, WINDOW.1 as u32),
        iced::advanced::renderer::Scale {
            window: 1.0,
            application: 1.0,
        },
    );
    let iced::Renderer::Primary(r) = &mut rig.renderer else {
        unreachable!("built as Primary")
    };
    std::hint::black_box(r.screenshot(&viewport, Color::BLACK));
    Frame {
        view,
        ui_update,
        app_update,
        draw,
        present: t.elapsed(),
    }
}

fn median(mut v: Vec<Duration>) -> f64 {
    v.sort();
    v[v.len() / 2].as_secs_f64() * 1e3
}

fn report(name: &str, frames: &[Frame], app: &Signex) {
    let col = |f: fn(&Frame) -> Duration| median(frames.iter().map(f).collect());
    let sum: f64 = frames.iter().map(|f| f.total().as_secs_f64() * 1e3).sum();
    let canvas = app.interaction_state.active_canvas();
    eprintln!(
        "APPBENCH {name:<10} per frame median ms: view {:6.2} ui.update {:6.2} app.update {:6.2} \
         draw {:6.2} present {:6.2} | TOTAL {:6.2} | {:.1} ms per 100 frames | \
         content_rebuilds {} drag_static_builds {}",
        col(|f| f.view),
        col(|f| f.ui_update),
        col(|f| f.app_update),
        col(|f| f.draw),
        col(|f| f.present),
        median(frames.iter().map(Frame::total).collect()),
        sum / frames.len() as f64 * 100.0,
        canvas.content_surface.borrow().rebuilds(),
        canvas.drag_layers.borrow().builds(),
    );
}

fn press(button: iced::mouse::Button) -> Event {
    Event::Mouse(iced::mouse::Event::ButtonPressed(button))
}

fn moved(p: iced::Point) -> Event {
    Event::Mouse(iced::mouse::Event::CursorMoved { position: p })
}

/// Where the canvas sits in the window: the sheet point under the first
/// hover tells (`CursorAt` publishes the world position).
fn canvas_origin(
    app: &mut Signex,
    window: iced::window::Id,
    rig: &mut super::render_bench_tests::Rig,
) -> iced::Point {
    let probe = iced::Point::new(WINDOW.0 / 2.0, WINDOW.1 / 2.0);
    let mut cache = None;
    let _ = frame(
        app,
        window,
        rig,
        &mut cache,
        &[],
        iced::mouse::Cursor::Unavailable,
    );
    let (x, y) = (app.ui_state.cursor_x, app.ui_state.cursor_y);
    assert_eq!((x, y), (0.0, 0.0), "no hover yet");
    let _ = frame(
        app,
        window,
        rig,
        &mut cache,
        &[moved(probe)],
        iced::mouse::Cursor::Available(probe),
    );
    let world = iced::Point::new(app.ui_state.cursor_x as f32, app.ui_state.cursor_y as f32);
    let local = app
        .interaction_state
        .active_canvas()
        .camera()
        .world_to_screen(world, Rectangle::default());
    iced::Point::new(probe.x - local.x, probe.y - local.y)
}

#[test]
#[ignore = "benchmark: needs a GPU"]
fn app_loop_bench() {
    let (cols, rows) = match std::env::var("SIGNEX_BENCH_SIZE").as_deref() {
        Ok("small") => (6, 5),
        Ok("large") => (24, 20),
        _ => (12, 10),
    };
    let steps: usize = std::env::var("SIGNEX_BENCH_STEPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let keep_wires = std::env::var("SIGNEX_BENCH_KEEP_WIRES").is_ok_and(|v| v != "0");
    let g = generate(cols, rows, Variant::from_env());
    let mut rig = rig(None, 1.0);
    eprintln!(
        "APPBENCH sheet {cols}x{rows}: {} symbols {} wires {} labels, keep_wires {keep_wires}, adapter {}",
        g.sheet.symbols.len(),
        g.sheet.wires.len(),
        g.sheet.labels.len(),
        rig.adapter.clone()
    );
    let (mut app, window) = app_with(&g, keep_wires);
    let origin = canvas_origin(&mut app, window, &mut rig);
    let camera = app.interaction_state.active_canvas().camera().clone();
    let center = iced::Point::new(WINDOW.0 / 2.0, WINDOW.1 / 2.0);
    let at = iced::mouse::Cursor::Available;
    let mut cache = None;
    let warm = |app: &mut Signex, rig: &mut _, cache: &mut Option<Cache>| {
        for _ in 0..3 {
            let _ = frame(
                app,
                window,
                rig,
                cache,
                &[],
                iced::mouse::Cursor::Unavailable,
            );
        }
    };
    warm(&mut app, &mut rig, &mut cache);

    // Hover: the cursor crosses the sheet with nothing held.
    let mut frames = Vec::new();
    for i in 0..steps {
        let p = iced::Point::new(
            center.x + 3.0 * i as f32 % 200.0,
            center.y + 2.0 * i as f32 % 150.0,
        );
        frames.push(frame(
            &mut app,
            window,
            &mut rig,
            &mut cache,
            &[moved(p)],
            at(p),
        ));
    }
    report("hover", &frames, &app);

    // Pan: middle button held, 4 and 2 px a move.
    let mut frames = Vec::new();
    let _ = frame(
        &mut app,
        window,
        &mut rig,
        &mut cache,
        &[press(iced::mouse::Button::Middle)],
        at(center),
    );
    for i in 0..steps {
        let p = iced::Point::new(
            center.x + 4.0 * (i + 1) as f32,
            center.y + 2.0 * (i + 1) as f32,
        );
        frames.push(frame(
            &mut app,
            window,
            &mut rig,
            &mut cache,
            &[moved(p)],
            at(p),
        ));
    }
    let _ = frame(
        &mut app,
        window,
        &mut rig,
        &mut cache,
        &[Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Middle,
        ))],
        at(center),
    );
    report("pan-mid", &frames, &app);

    // Touchpad scroll in Scroll wheel mode: pixel deltas.
    let mut frames = Vec::new();
    for _ in 0..steps {
        let scroll = Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Pixels { x: 3.0, y: -2.0 },
        });
        frames.push(frame(
            &mut app,
            window,
            &mut rig,
            &mut cache,
            &[scroll],
            at(center),
        ));
    }
    report("pan-scroll", &frames, &app);

    // Drag: restore the camera, select the block, press on the handle wire.
    *app.interaction_state.active_canvas_mut().camera_mut() = camera.clone();
    // `SIGNEX_BENCH_SELECT=one` drags the single wire under the handle (a
    // placed part is about as cheap); the default drags the whole block.
    let selected = if std::env::var("SIGNEX_BENCH_SELECT").is_ok_and(|v| v == "one") {
        let hit = app
            .item_hit_at(g.handle.0, g.handle.1)
            .expect("the handle wire");
        vec![hit]
    } else {
        g.block.clone()
    };
    app.interaction_state.active_canvas_mut().selected = selected;
    app.interaction_state.active_canvas_mut().clear_bg_cache();
    warm(&mut app, &mut rig, &mut cache);
    let start = iced::Point::new(
        origin.x + camera.offset.x + g.handle.0 as f32 * camera.scale,
        origin.y + camera.offset.y + g.handle.1 as f32 * camera.scale,
    );
    let mut frames = Vec::new();
    let _ = frame(
        &mut app,
        window,
        &mut rig,
        &mut cache,
        &[press(iced::mouse::Button::Left)],
        at(start),
    );
    // `SIGNEX_BENCH_MOVES` cursor events arrive per frame (a fast mouse).
    let per_frame: usize = std::env::var("SIGNEX_BENCH_MOVES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    // `SIGNEX_BENCH_STRIDE`: logical px the cursor travels per event (4 default).
    let stride: f32 = std::env::var("SIGNEX_BENCH_STRIDE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4.0);
    for i in 0..steps {
        let events: Vec<Event> = (0..per_frame)
            .map(|k| {
                let n = (i * per_frame + k + 1) as f32;
                moved(iced::Point::new(
                    start.x + stride * n,
                    start.y + stride * 0.5 * n,
                ))
            })
            .collect();
        let n = ((i + 1) * per_frame) as f32;
        let last = iced::Point::new(start.x + stride * n, start.y + stride * 0.5 * n);
        frames.push(frame(
            &mut app,
            window,
            &mut rig,
            &mut cache,
            &events,
            at(last),
        ));
    }
    report("drag", &frames, &app);
}
