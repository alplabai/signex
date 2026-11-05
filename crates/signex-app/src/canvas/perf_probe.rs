//! Temporary perf probe (not committed): pan and component drag on a busy sheet,
//! driven by real mouse events through the full UI, timed per frame on the GPU.
use std::time::{Duration, Instant};

use iced::{Point, Size, mouse};
use iced_wgpu::wgpu;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

use crate::app::{Message, Signex, TabInfo, TabKind};

fn stats(label: &str, mut v: Vec<Duration>) {
    if v.is_empty() {
        eprintln!("PERF {label:<60} (no samples)");
        return;
    }
    v.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let p90 = v[(v.len() * 9 / 10).min(v.len() - 1)];
    eprintln!(
        "PERF {label:<60} min {:8.3}  med {:8.3}  p90 {:8.3}  max {:8.3} ms",
        ms(v[0]),
        ms(v[v.len() / 2]),
        ms(p90),
        ms(v[v.len() - 1])
    );
}

fn shift(p: &mut signex_types::schematic::Point, dx: f64, dy: f64) {
    p.x += dx;
    p.y += dy;
}

/// The sample sheet tiled `nx` by `ny` times, every item with a fresh uuid.
fn busy_sheet(nx: usize, ny: usize) -> SchematicSheet {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/sample-project/linear-regulator.snxsch"
    ))
    .expect("sample");
    let base = signex_types::format::SnxSchematic::parse(&text)
        .expect("parse")
        .sheet;
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut grow = |p: &signex_types::schematic::Point| {
        minx = minx.min(p.x);
        miny = miny.min(p.y);
        maxx = maxx.max(p.x);
        maxy = maxy.max(p.y);
    };
    base.symbols.iter().for_each(|s| grow(&s.position));
    base.wires.iter().for_each(|w| {
        grow(&w.start);
        grow(&w.end);
    });
    let (w, h) = (maxx - minx + 30.0, maxy - miny + 30.0);
    let mut out = base.clone();
    out.symbols.clear();
    out.wires.clear();
    out.junctions.clear();
    out.labels.clear();
    out.no_connects.clear();
    out.text_notes.clear();
    out.drawings.clear();
    out.child_sheets.clear();
    out.paper_size = "A0".into();
    for j in 0..ny {
        for i in 0..nx {
            let (dx, dy) = (i as f64 * w - minx + 20.0, j as f64 * h - miny + 20.0);
            for s in &base.symbols {
                let mut s = s.clone();
                s.uuid = uuid::Uuid::now_v7();
                shift(&mut s.position, dx, dy);
                if let Some(t) = s.ref_text.as_mut() {
                    shift(&mut t.position, dx, dy);
                }
                if let Some(t) = s.val_text.as_mut() {
                    shift(&mut t.position, dx, dy);
                }
                out.symbols.push(s);
            }
            for x in &base.wires {
                let mut x = x.clone();
                x.uuid = uuid::Uuid::now_v7();
                shift(&mut x.start, dx, dy);
                shift(&mut x.end, dx, dy);
                out.wires.push(x);
            }
            for x in &base.junctions {
                let mut x = x.clone();
                x.uuid = uuid::Uuid::now_v7();
                shift(&mut x.position, dx, dy);
                out.junctions.push(x);
            }
            for x in &base.labels {
                let mut x = x.clone();
                x.uuid = uuid::Uuid::now_v7();
                shift(&mut x.position, dx, dy);
                out.labels.push(x);
            }
            for x in &base.no_connects {
                let mut x = x.clone();
                x.uuid = uuid::Uuid::now_v7();
                shift(&mut x.position, dx, dy);
                out.no_connects.push(x);
            }
        }
    }
    out
}

fn app_with(sheet: SchematicSheet) -> Signex {
    let (mut app, _t) = Signex::new();
    let path = std::path::PathBuf::from("busy.snxsch");
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "busy".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.sync_canvas_from_visible_schematic(crate::schematic_runtime::RenderInvalidation::FULL);
    app
}

fn gpu() -> Option<iced::Renderer> {
    let instance = wgpu::Instance::default();
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .ok()?;
    eprintln!("PERF adapter {:?}", adapter.get_info().name);
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("perf"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .ok()?;
    let engine = iced_wgpu::Engine::new(
        &adapter,
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        None,
        iced::advanced::graphics::Shell::headless(),
    );
    Some(iced::Renderer::Primary(iced_wgpu::Renderer::new(
        engine,
        iced::advanced::renderer::Settings::default(),
    )))
}

const WIN: Size = Size::new(1920.0, 1080.0);

/// Per-frame phase times: [app.update of the frame's messages, build, ui.update, draw, gpu].
#[derive(Default)]
struct Phases([Vec<Duration>; 5], Vec<Duration>);

struct Driver {
    app: Signex,
    renderer: iced::Renderer,
    cache: iced_runtime::user_interface::Cache,
    main: iced::window::Id,
}

impl Driver {
    /// One frame: deliver `events` at `cursor`, run the messages they publish
    /// through `Signex::update`, rebuild, draw and render.
    fn frame(&mut self, events: &[iced::Event], cursor: Point, record: Option<&mut Phases>) {
        let cursor = mouse::Cursor::Available(cursor);
        let theme = self.app.theme(self.main).expect("theme");
        let t0 = Instant::now();
        let mut ui = iced_runtime::UserInterface::build(
            self.app.view(self.main),
            WIN,
            std::mem::take(&mut self.cache),
            &mut self.renderer,
        );
        let mut bus = iced::advanced::shell::Bus::new();
        let waker = iced::advanced::shell::Waker::noop();
        let t1 = Instant::now();
        let _ = ui.update(
            &iced::window::Headless,
            &waker,
            events,
            cursor,
            &mut self.renderer,
            &mut bus,
        );
        self.cache = ui.into_cache();
        let t2 = Instant::now();
        let msgs: Vec<Message> = bus.drain().map(|(m, _)| m).collect();
        for m in msgs {
            let _ = self.app.update(m);
        }
        let t3 = Instant::now();
        // Redraw pass, as the runtime does after messages.
        let mut ui = iced_runtime::UserInterface::build(
            self.app.view(self.main),
            WIN,
            std::mem::take(&mut self.cache),
            &mut self.renderer,
        );
        let t4 = Instant::now();
        ui.draw(
            &mut self.renderer,
            &theme,
            &iced::advanced::renderer::Style {
                text_color: iced::Color::WHITE,
            },
            cursor,
        );
        self.cache = ui.into_cache();
        let t5 = Instant::now();
        let viewport = iced::advanced::graphics::Viewport::with_physical_size(
            Size::new(1920, 1080),
            iced::advanced::renderer::Scale::default(),
        );
        let iced::Renderer::Primary(w) = &mut self.renderer else {
            unreachable!()
        };
        let _ = w.screenshot(&viewport, iced::Color::BLACK);
        let t6 = Instant::now();
        if let Some(p) = record {
            p.0[0].push(t3 - t2);
            p.0[1].push((t1 - t0) + (t4 - t3));
            p.0[2].push(t2 - t1);
            p.0[3].push(t5 - t4);
            p.0[4].push(t6 - t5);
            p.1.push(t5 - t0);
        }
    }

    fn report(name: &str, p: Phases) {
        let [a, b, c, d, e] = p.0;
        stats(&format!("{name}: Signex::update (messages)"), a);
        stats(&format!("{name}: view + layout (x2)"), b);
        stats(&format!("{name}: ui.update (canvas event)"), c);
        stats(&format!("{name}: ui.draw (canvas geometry)"), d);
        stats(&format!("{name}: gpu render + readback"), e);
        stats(&format!("{name}: CPU frame total (excl. gpu)"), p.1);
    }
}

fn moved(p: Point) -> iced::Event {
    iced::Event::Mouse(mouse::Event::CursorMoved { position: p })
}
fn pressed(b: mouse::Button) -> iced::Event {
    iced::Event::Mouse(mouse::Event::ButtonPressed(b))
}
fn released(b: mouse::Button) -> iced::Event {
    iced::Event::Mouse(mouse::Event::ButtonReleased(b))
}

fn run(nx: usize, ny: usize, renderer: iced::Renderer) -> iced::Renderer {
    const N: usize = 60;
    let sheet = busy_sheet(nx, ny);
    let (ns, nw) = (sheet.symbols.len(), sheet.wires.len());
    let target = sheet.symbols[0].uuid;
    let target_pos = sheet.symbols[0].position;
    let app = app_with(sheet);
    let main = app.ui_state.main_window_id.expect("main window");
    let mut d = Driver {
        app,
        renderer,
        cache: Default::default(),
        main,
    };
    let tag = format!("{ns} symbols {nw} wires");

    // Settle: find the canvas, fit content.
    let probe = Point::new(WIN.width / 2.0, WIN.height / 2.0);
    for _ in 0..3 {
        d.frame(&[moved(probe)], probe, None);
    }
    let vp = d.app.interaction_state.active_canvas().viewport.get();
    eprintln!("PERF [{tag}] canvas viewport {vp:?}");
    d.app.interaction_state.active_canvas_mut().fit_to_content();
    for _ in 0..3 {
        d.frame(&[moved(probe)], probe, None);
    }
    eprintln!(
        "PERF [{tag}] camera {:?}",
        d.app.interaction_state.active_canvas().live_camera()
    );

    // Hover (no buttons).
    let mut p = Phases::default();
    for i in 0..N {
        let c = Point::new(vp.x + 300.0 + (i % 40) as f32 * 5.0, vp.y + 300.0);
        d.frame(&[moved(c)], c, Some(&mut p));
    }
    Driver::report(&format!("[{tag}] hover"), p);

    // Pan with the middle button.
    let start = Point::new(vp.x + vp.width / 2.0, vp.y + vp.height / 2.0);
    d.frame(&[moved(start), pressed(mouse::Button::Middle)], start, None);
    let mut p = Phases::default();
    for i in 0..N {
        let c = Point::new(start.x + (i as f32 + 1.0) * 4.0, start.y + (i as f32 + 1.0) * 2.0);
        d.frame(&[moved(c)], c, Some(&mut p));
    }
    let end = Point::new(start.x + N as f32 * 4.0, start.y + N as f32 * 2.0);
    d.frame(&[released(mouse::Button::Middle)], end, None);
    eprintln!(
        "PERF [{tag}] camera after pan {:?}",
        d.app.interaction_state.active_canvas().live_camera()
    );
    Driver::report(&format!("[{tag}] middle-pan"), p);

    // Drag one symbol: select it, press on its origin, move.
    let (ox, oy, sc) = d.app.interaction_state.active_canvas().live_camera();
    let at = Point::new(
        vp.x + ox + target_pos.x as f32 * sc,
        vp.y + oy + target_pos.y as f32 * sc,
    );
    d.app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(target, SelectedKind::Symbol)];
    d.frame(&[moved(at)], at, None);
    d.frame(&[pressed(mouse::Button::Left)], at, None);
    let mut p = Phases::default();
    let mut live = 0;
    for i in 0..N {
        let c = Point::new(at.x + (i as f32 + 1.0) * 3.0, at.y + (i as f32 % 7.0));
        d.frame(&[moved(c)], c, Some(&mut p));
        if d.app.interaction_state.active_canvas().drag_layers.borrow().is_active() {
            live += 1;
        }
    }
    let endc = Point::new(at.x + N as f32 * 3.0, at.y);
    let rev0 = d.app.document_state.engines.values().next().map(|e| e.revision());
    d.frame(&[released(mouse::Button::Left)], endc, None);
    let rev1 = d.app.document_state.engines.values().next().map(|e| e.revision());
    eprintln!("PERF [{tag}] drag: frames with drag layers active {live}/{N}; revision {rev0:?} -> {rev1:?}");
    Driver::report(&format!("[{tag}] symbol drag"), p);
    d.renderer
}

#[test]
#[ignore = "perf probe"]
fn busy_sheet_probe() {
    let Some(mut r) = gpu() else {
        eprintln!("PERF no GPU");
        return;
    };
    for (nx, ny) in [(1, 1), (4, 3), (8, 6)] {
        r = run(nx, ny, r);
    }
}

#[test]
#[ignore = "perf probe"]
fn drag_step_breakdown() {
    for (nx, ny) in [(4, 3), (8, 6)] {
        let sheet = busy_sheet(nx, ny);
        let tag = format!("{} symbols {} wires", sheet.symbols.len(), sheet.wires.len());
        let sel = vec![SelectedItem::new(sheet.symbols[0].uuid, SelectedKind::Symbol)];
        const N: u32 = 30;
        let t = Instant::now();
        for _ in 0..N {
            drop(std::hint::black_box(sheet.clone()));
        }
        eprintln!("SPAN [{tag}] sheet.clone()+drop            {:8.3} ms", t.elapsed().as_secs_f64() * 1000.0 / N as f64);
        let t = Instant::now();
        for _ in 0..N {
            drop(std::hint::black_box(signex_engine::before_nets(&sheet)));
        }
        eprintln!("SPAN [{tag}] before_nets (one net derive)  {:8.3} ms", t.elapsed().as_secs_f64() * 1000.0 / N as f64);
        let preview = signex_engine::MovePreview::new(&sheet);
        let colors = Default::default();
        let moved_ids = super::drag_layers::moved_item_ids(&sheet, &sel);
        let _ = signex_engine::perf_spans::take();
        let (mut total, mut split) = (Duration::ZERO, Duration::ZERO);
        for i in 0..N {
            let d = 1.27 * (i as f64 + 1.0);
            let moves = vec![(sel[0], d, (i % 3) as f64 * 1.27)];
            let t = Instant::now();
            let (moved, _held) = preview.moved_with(&moves, true, &colors);
            total += t.elapsed();
            let t = Instant::now();
            let f = super::drag_layers::split_ghost(&sheet, &moved, &moved_ids);
            split += t.elapsed();
            drop(f);
            drop(moved);
        }
        let per = |d: Duration, n: u32| d.as_secs_f64() * 1000.0 / n as f64;
        eprintln!("SPAN [{tag}] moved_with total / step       {:8.3} ms", per(total, N));
        eprintln!("SPAN [{tag}] split_ghost / step            {:8.3} ms", per(split, N));
        for (name, (d, c)) in signex_engine::perf_spans::take() {
            eprintln!("SPAN [{tag}]   {name:<28} {:8.3} ms/step  ({c} calls)", per(d, N));
        }
    }
}
