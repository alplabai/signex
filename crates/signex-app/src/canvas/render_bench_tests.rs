//! The schematic render paths, measured (#2040): the iced canvas (CPU
//! tessellation, `geometry::Cache`, retained pan layer) against the GPU mount
//! (`SchematicSurface`, scene upload, instanced draw). `#[ignore]`d: run by
//! hand, see `docs/architecture/schematic-render-bench.md`.
//!
//! Both paths are driven through the real widgets: `UserInterface::build`,
//! `update` (the drag is real mouse events), `draw`, then
//! `iced_wgpu::Renderer::screenshot`, which uploads, renders and reads back, so
//! the GPU work is inside the timing. Never tiny-skia.
//!
//! ```text
//! cargo test -p signex-app --release --lib render_bench -- --ignored --nocapture --test-threads=1
//! SIGNEX_BENCH_SIZE=medium SIGNEX_BENCH_DPR=2 cargo test ... (same line)
//! ```

use std::time::{Duration, Instant};

use iced::advanced::graphics::Antialiasing;
use iced::advanced::renderer::Style;
use iced::widget::Widget as _;
use iced::{Color, Element, Event, Font, Pixels, Size, Theme};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use iced_wgpu::wgpu;
use signex_types::schematic::{
    FillType, Graphic, HAlign, Junction, Label, LabelType, LibGraphic, LibPin, LibSymbol, Pin,
    PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol,
    TextNote, TextProp, VAlign, Wire,
};
use uuid::Uuid;

use super::tests::test_prefs;
use super::*;

const WINDOW: (f32, f32) = (1600.0, 1300.0);
const CELL_W: f64 = 60.0;
const CELL_H: f64 = 40.0;
const MARGIN: f64 = 20.0;
/// The scale at which the sheet is worked on (about 148% of a 96 dpi mm).
const WORK_SCALE: f32 = 5.6;

// --- the sheet -------------------------------------------------------------

struct Generated {
    sheet: SchematicSheet,
    paper: (f32, f32),
    /// The middle of a wire inside the drag block: pressing there grabs it.
    handle: (f64, f64),
    /// Everything in a block of cells around the middle of the sheet.
    block: Vec<SelectedItem>,
}

fn id(n: &std::cell::Cell<u128>) -> Uuid {
    n.set(n.get() + 1);
    Uuid::from_u128(n.get())
}

fn lib(name: &str, half: (f64, f64), pins: &[(f64, f64, f64)], text: bool) -> LibSymbol {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": name })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-half.0, -half.1),
            end: Point::new(half.0, half.1),
            width: 0.254,
            fill: FillType::Background,
        },
    });
    for (n, (x, y, rotation)) in pins.iter().enumerate() {
        lib.pins.push(LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(*x, *y),
                rotation: *rotation,
                length: 2.54,
                name: format!("P{n}"),
                number: (n + 1).to_string(),
                visible: true,
                name_visible: text,
                number_visible: text,
            },
        });
    }
    lib
}

fn text_prop(x: f64, y: f64, text: bool) -> TextProp {
    TextProp {
        position: Point::new(x, y),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        hidden: !text,
        font_family: None,
    }
}

/// What the generated sheet carries. `SIGNEX_BENCH_VARIANT`: `full` (default),
/// `norot` (no text turned 90 degrees) or `notext` (no text at all).
#[derive(Clone, Copy)]
struct Variant {
    rotated: bool,
    text: bool,
}

impl Variant {
    fn from_env() -> Self {
        match std::env::var("SIGNEX_BENCH_VARIANT").as_deref() {
            Ok("norot") => Self {
                rotated: false,
                text: true,
            },
            Ok("notext") => Self {
                rotated: false,
                text: false,
            },
            _ => Self {
                rotated: true,
                text: true,
            },
        }
    }
}

/// A grid of identical cells: one 8-pin IC, four resistors, 16 wires, 4 net
/// labels (half of them turned 90 degrees), 4 junctions and a note each.
/// Deterministic: the uuids count up.
fn generate(cols: usize, rows: usize, v: Variant) -> Generated {
    let n = std::cell::Cell::new(0u128);
    let mut sheet = SchematicSheet::default();
    let ys = [-3.81, -1.27, 1.27, 3.81];
    let mut ic_pins = Vec::new();
    for y in ys {
        ic_pins.push((-10.16, y, 0.0));
    }
    for y in ys {
        ic_pins.push((10.16, y, 180.0));
    }
    sheet
        .lib_symbols
        .insert("IC".into(), lib("IC", (7.62, 5.08), &ic_pins, v.text));
    sheet.lib_symbols.insert(
        "R".into(),
        lib(
            "R",
            (2.54, 1.0),
            &[(-3.81, 0.0, 0.0), (3.81, 0.0, 180.0)],
            v.text,
        ),
    );
    let mut block_cells = Vec::new();
    let (mid_c, mid_r) = (cols / 2, rows / 2);
    let mut handle = (0.0, 0.0);
    for r in 0..rows {
        for c in 0..cols {
            let (ox, oy) = (MARGIN + c as f64 * CELL_W, MARGIN + r as f64 * CELL_H);
            let in_block = c.abs_diff(mid_c) <= 1 && r.abs_diff(mid_r) <= 1;
            let (ix, iy) = (ox + 30.0, oy + 16.0);
            let mut cell: Vec<SelectedItem> = Vec::new();
            let symbol = |sheet: &mut SchematicSheet, lib_id: &str, x: f64, y: f64, rf: &str| {
                let uuid = id(&n);
                let mut s = Symbol {
                    uuid,
                    lib_id: lib_id.into(),
                    reference: rf.into(),
                    value: "100k".into(),
                    position: Point::new(x, y),
                    ref_text: Some(text_prop(x, y - 7.0, v.text)),
                    val_text: Some(text_prop(x, y + 7.0, v.text)),
                    ..Symbol::default()
                };
                s.unit = 1;
                sheet.symbols.push(s);
                SelectedItem::new(uuid, SelectedKind::Symbol)
            };
            cell.push(symbol(&mut sheet, "IC", ix, iy, "U1"));
            let wire = |sheet: &mut SchematicSheet, a: (f64, f64), b: (f64, f64)| {
                let uuid = id(&n);
                sheet.wires.push(Wire {
                    uuid,
                    start: Point::new(a.0, a.1),
                    end: Point::new(b.0, b.1),
                    stroke_width: 0.25,
                    unknown_columns: Default::default(),
                    locked: false,
                });
                SelectedItem::new(uuid, SelectedKind::Wire)
            };
            for (k, y) in ys.iter().enumerate() {
                let py = iy + y;
                // Left: stub and a net label, odd ones turned 90 degrees.
                cell.push(wire(&mut sheet, (ix - 10.16, py), (ix - 18.0, py)));
                let uuid = id(&n);
                if v.text {
                    sheet.labels.push(Label {
                        uuid,
                        text: format!("N{c}_{r}_{k}"),
                        position: Point::new(ix - 18.0, py),
                        rotation: if k % 2 == 1 && v.rotated { 90.0 } else { 0.0 },
                        label_type: LabelType::Net,
                        shape: String::new(),
                        font_size: 1.27,
                        font_family: None,
                        justify: HAlign::Left,
                        justify_v: VAlign::Bottom,
                        unknown_columns: Default::default(),
                        locked: false,
                    });
                    cell.push(SelectedItem::new(uuid, SelectedKind::Label));
                }
                // Right: stub, a resistor, output, rail.
                cell.push(wire(&mut sheet, (ix + 10.16, py), (ix + 14.0, py)));
                cell.push(symbol(&mut sheet, "R", ix + 17.81, py, "R1"));
                cell.push(wire(&mut sheet, (ix + 21.62, py), (ox + 54.0, py)));
                let uuid = id(&n);
                sheet.junctions.push(Junction {
                    uuid,
                    position: Point::new(ox + 54.0, py),
                    diameter: 0.0,
                    minted: false,
                    unknown: Default::default(),
                    unknown_columns: Default::default(),
                });
                cell.push(SelectedItem::new(uuid, SelectedKind::Junction));
                if k < 3 {
                    cell.push(wire(&mut sheet, (ox + 54.0, py), (ox + 54.0, py + 2.54)));
                }
            }
            // A wire across the cell, one cell wide, nothing else on it.
            let hy = oy + 36.0;
            let handle_wire = wire(&mut sheet, (ox, hy), (ox + CELL_W, hy));
            if (c, r) == (mid_c, mid_r) {
                handle = (ox + CELL_W / 2.0, hy);
            }
            cell.push(handle_wire);
            let uuid = id(&n);
            if v.text {
                sheet.text_notes.push(TextNote {
                    uuid,
                    text: format!("Section {c}/{r}: output filter"),
                    position: Point::new(ox + 2.0, oy + 3.0),
                    rotation: 0.0,
                    font_size: 1.27,
                    font_family: None,
                    justify_h: HAlign::Left,
                    justify_v: VAlign::Top,
                    locked: false,
                });
                cell.push(SelectedItem::new(uuid, SelectedKind::TextNote));
            }
            if in_block {
                block_cells.extend(cell);
            }
        }
    }
    Generated {
        paper: (
            (MARGIN * 2.0 + cols as f64 * CELL_W) as f32,
            (MARGIN * 2.0 + rows as f64 * CELL_H) as f32,
        ),
        sheet,
        handle,
        block: block_cells,
    }
}

fn counts(sheet: &SchematicSheet) -> String {
    let pins: usize = sheet
        .symbols
        .iter()
        .map(|s| sheet.body_of(&s).map_or(0, |l| l.pins.len()))
        .sum();
    format!(
        "{} symbols, {} pins, {} wires, {} junctions, {} labels, {} text notes",
        sheet.symbols.len(),
        pins,
        sheet.wires.len(),
        sheet.junctions.len(),
        sheet.labels.len(),
        sheet.text_notes.len()
    )
}

// --- the renderer ----------------------------------------------------------

struct Rig {
    renderer: iced::Renderer,
    adapter: String,
    dpr: f32,
}

fn rig(aa: Option<Antialiasing>, dpr: f32) -> Rig {
    let instance = wgpu::Instance::default();
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("no wgpu adapter");
    let info = adapter.get_info();
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("render_bench_device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("device");
    // A failed submission otherwise shows up only as an unmappable readback.
    device.set_device_lost_callback(|reason, message| {
        eprintln!("BENCH wgpu device lost: {reason:?}: {message}");
    });
    device.on_uncaptured_error(std::sync::Arc::new(|e: wgpu::Error| {
        eprintln!("BENCH wgpu error: {e}");
    }));
    let engine = iced_wgpu::Engine::new(
        &adapter,
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        aa,
        iced::advanced::graphics::Shell::headless(),
    );
    let mut fonts = iced::advanced::graphics::text::font_system()
        .write()
        .expect("font system");
    for face in crate::fonts::bundled::BUNDLED_FONTS {
        fonts.load_font(std::borrow::Cow::Borrowed(face));
    }
    drop(fonts);
    Rig {
        renderer: iced::Renderer::Primary(iced_wgpu::Renderer::new(
            engine,
            iced::advanced::renderer::Settings {
                font: Font::DEFAULT,
                text_size: Pixels(16.0),
                ..Default::default()
            },
        )),
        adapter: format!("{} ({:?})", info.name, info.backend),
        dpr,
    }
}

/// What `footprint` reports for this process: total and the IOAccelerator
/// category (where Metal buffers and textures live on Apple silicon), in MB.
#[allow(
    clippy::disallowed_methods,
    reason = "a bench measures its own process with a system tool"
)]
fn footprint_mb() -> Option<(f64, f64)> {
    let out = std::process::Command::new("footprint")
        .args(["-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mb = |value: &str, unit: &str| -> f64 {
        let v: f64 = value.parse().unwrap_or(0.0);
        match unit {
            "GB" => v * 1024.0,
            "MB" => v,
            "KB" => v / 1024.0,
            _ => v / 1_048_576.0,
        }
    };
    let (mut total, mut accel) = (None, 0.0);
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if line.contains("Footprint:") {
            let at = t.iter().position(|w| *w == "Footprint:")?;
            total = Some(mb(t.get(at + 1)?, t.get(at + 2)?));
        } else if t.len() >= 8 && t[7..].join(" ") == "IOAccelerator" {
            accel += mb(t[0], t[1]);
        }
    }
    Some((total?, accel))
}

/// Peak resident set size of this process so far, MB.
fn peak_rss_mb() -> f64 {
    // SAFETY: `getrusage` fills the struct it is given and nothing else.
    let usage = unsafe {
        let mut usage = std::mem::zeroed::<libc::rusage>();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        usage
    };
    // Bytes on macOS, kilobytes elsewhere.
    let unit = if cfg!(target_os = "macos") {
        1.0
    } else {
        1024.0
    };
    usage.ru_maxrss as f64 * unit / 1_048_576.0
}

/// Resident set size of this process, MB.
#[allow(
    clippy::disallowed_methods,
    reason = "a bench measures its own process with a system tool"
)]
fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
        / 1024.0
}

// --- one frame -------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Path {
    /// The CPU canvas as the app mounts it today.
    Canvas,
    /// The GPU mount: backdrop, content shader, texts, drag shader, overlay.
    Gpu,
    /// The backdrop canvas alone (paper, grid, frame): common to both paths.
    Backdrop,
    /// The two shader layers alone: the GPU content with no CPU layer.
    Shaders,
    /// The rotated and oversize texts the GPU mount draws on the CPU.
    Texts,
    /// The interactive overlay of the GPU mount (selection, HUD, previews).
    Overlay,
}

impl Path {
    fn name(self) -> &'static str {
        match self {
            Path::Canvas => "canvas",
            Path::Gpu => "gpu",
            Path::Backdrop => "backdrop",
            Path::Shaders => "shaders",
            Path::Texts => "texts",
            Path::Overlay => "overlay",
        }
    }

    /// Whether the mounted widgets take mouse events (a drag needs them).
    fn is_interactive(self) -> bool {
        matches!(self, Path::Canvas | Path::Gpu)
    }
}

#[derive(Default)]
struct Phases {
    build: Vec<Duration>,
    update: Vec<Duration>,
    draw: Vec<Duration>,
    present: Vec<Duration>,
    total: Vec<Duration>,
}

impl Phases {
    fn line(&self) -> String {
        let cell = |v: &[Duration]| {
            let mut v = v.to_vec();
            v.sort();
            let at = |q: f64| v[((v.len() - 1) as f64 * q) as usize].as_secs_f64() * 1e3;
            format!("{:7.2}/{:7.2}", at(0.5), at(0.95))
        };
        format!(
            "build {} update {} draw {} present {} TOTAL {}",
            cell(&self.build),
            cell(&self.update),
            cell(&self.draw),
            cell(&self.present),
            cell(&self.total),
        )
    }
}

/// One frame through the real widgets. `events` go through `UserInterface::update`.
fn frame(
    rig: &mut Rig,
    path: Path,
    slot: &CanvasSlot,
    prefs: CanvasViewPrefs<'_>,
    cache: &mut Option<Cache>,
    events: &[Event],
    cursor: mouse::Cursor,
) -> [Duration; 5] {
    let whole = Instant::now();
    let element: Element<'_, crate::app::Message> = match path {
        Path::Canvas => SchematicSurface::new(SchematicCanvas::new(slot, prefs)).boxed(),
        Path::Gpu => super::gpu_mount::view(slot, prefs),
        Path::Backdrop => canvas(super::gpu_mount::SchematicBackdrop::new(slot, prefs))
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .boxed(),
        Path::Texts => canvas(super::gpu_drag::SchematicStaticTextLayer::new(slot, prefs))
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .boxed(),
        Path::Overlay => canvas(SchematicCanvas::new(slot, prefs).gpu_overlay())
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .boxed(),
        Path::Shaders => iced::widget::stack![
            iced::widget::shader(super::gpu_mount::SchematicShaderProgram::new(slot, prefs))
                .width(iced::Length::Fill)
                .height(iced::Length::Fill),
            iced::widget::shader(super::gpu_drag::SchematicDragShaderProgram::new(
                slot, prefs
            ))
            .width(iced::Length::Fill)
            .height(iced::Length::Fill),
        ]
        .boxed(),
    };
    let size = Size::new(WINDOW.0, WINDOW.1);
    let t = Instant::now();
    let mut ui = UserInterface::build(
        element,
        size,
        cache.take().unwrap_or_default(),
        &mut rig.renderer,
    );
    let build = t.elapsed();

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
    let dpr = rig.dpr;
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
    std::hint::black_box(r.screenshot(&viewport, Color::BLACK));
    let present = t.elapsed();
    [build, update, draw, present, whole.elapsed()]
}

fn push(p: &mut Phases, f: [Duration; 5]) {
    p.build.push(f[0]);
    p.update.push(f[1]);
    p.draw.push(f[2]);
    p.present.push(f[3]);
    p.total.push(f[4]);
}

// --- scenarios -------------------------------------------------------------

#[derive(Clone, Copy)]
enum View {
    /// The whole sheet on screen: the worst case for the content layer.
    Fit,
    /// Working zoom: a few dozen symbols.
    Work,
}

struct Case<'a> {
    generated: &'a Generated,
    view: View,
    path: Path,
    dpr: f32,
    keep_wires: bool,
}

impl Case<'_> {
    fn camera(&self) -> Camera {
        match self.view {
            View::Fit => {
                let scale = (WINDOW.0 / self.generated.paper.0)
                    .min(WINDOW.1 / self.generated.paper.1)
                    * 0.97;
                Camera {
                    offset: iced::Point::new(8.0, 8.0),
                    scale,
                }
            }
            View::Work => {
                // The drag handle in the middle of the window.
                let (hx, hy) = self.generated.handle;
                Camera {
                    offset: iced::Point::new(
                        WINDOW.0 / 2.0 - hx as f32 * WORK_SCALE,
                        WINDOW.1 / 2.0 - hy as f32 * WORK_SCALE,
                    ),
                    scale: WORK_SCALE,
                }
            }
        }
    }

    fn slot(&self, selected: bool) -> CanvasSlot {
        let mut slot = slot_for(&self.generated.sheet, self.generated.paper);
        *slot.camera_mut() = self.camera();
        if selected {
            slot.selected = self.generated.block.clone();
        }
        slot
    }
}

fn slot_for(sheet: &SchematicSheet, paper: (f32, f32)) -> CanvasSlot {
    let mut slot = CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(sheet, 1),
    ));
    slot.paper_width_mm = paper.0;
    slot.paper_height_mm = paper.1;
    slot
}

/// `SIGNEX_BENCH_GRID=0` draws no grid, to read the grid's share.
fn grid_visible() -> bool {
    std::env::var("SIGNEX_BENCH_GRID").map_or(true, |v| v != "0")
}

fn at(p: iced::Point) -> mouse::Cursor {
    mouse::Cursor::Available(p)
}

fn moved(p: iced::Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: p })
}

/// Run `steps` frames; `step(i, slot)` moves the camera and returns the events
/// and cursor of frame `i`.
fn run(
    case: &Case<'_>,
    rig: &mut Rig,
    selected: bool,
    steps: usize,
    mut step: impl FnMut(usize, &CanvasSlot) -> (Vec<Event>, mouse::Cursor),
) -> (Phases, String) {
    let slot = case.slot(selected);
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.device_px_per_logical_px = case.dpr;
    prefs.move_keeps_wires = case.keep_wires;
    prefs.grid_visible = grid_visible();
    let mut cache = None;
    // Warm-up: the first frames compile pipelines and fill the glyph atlas.
    for _ in 0..3 {
        frame(
            rig,
            case.path,
            &slot,
            prefs,
            &mut cache,
            &[],
            mouse::Cursor::Unavailable,
        );
    }
    let mut phases = Phases::default();
    for i in 0..steps {
        let (events, cursor) = step(i, &slot);
        push(
            &mut phases,
            frame(rig, case.path, &slot, prefs, &mut cache, &events, cursor),
        );
    }
    let moving = slot.published_drag.frame().map_or("none".to_string(), |f| {
        format!(
            "{}sym/{}wires",
            f.moving.symbols.len(),
            f.moving.wires.len()
        )
    });
    let note = format!(
        "moving={moving} tessellations={} scene_gen={:?} drag_static_builds={}",
        slot.content_surface.borrow().rebuilds(),
        slot.content_scene.borrow().generation(),
        slot.drag_layers.borrow().builds(),
    );
    (phases, note)
}

fn report(case: &Case<'_>, aa: &str, action: &str, phases: &Phases, note: &str, mem: String) {
    eprintln!(
        "BENCH {:<8} aa={:<5} {:<4} {:<8} {} | {note} {mem}",
        case.path.name(),
        aa,
        match case.view {
            View::Fit => "fit",
            View::Work => "work",
        },
        action,
        phases.line(),
    );
}

fn measure_case(case: &Case<'_>, aa: &str, rig: &mut Rig) {
    let cam = case.camera();
    let center = iced::Point::new(WINDOW.0 / 2.0, WINDOW.1 / 2.0);

    // Floor: a frame of an empty sheet, the same stack, so readback plus the empty backdrop.
    let empty = SchematicSheet::default();
    let floor_slot = slot_for(&empty, case.generated.paper);
    *floor_slot.camera_mut() = cam.clone();
    {
        let overrides = Default::default();
        let filters = Default::default();
        let mut prefs = test_prefs(&overrides, &filters);
        prefs.device_px_per_logical_px = case.dpr;
        prefs.grid_visible = grid_visible();
        let mut cache = None;
        let mut p = Phases::default();
        for i in 0..23 {
            let f = frame(
                rig,
                case.path,
                &floor_slot,
                prefs,
                &mut cache,
                &[],
                mouse::Cursor::Unavailable,
            );
            if i >= 3 {
                push(&mut p, f);
            }
        }
        report(case, aa, "floor", &p, "(empty sheet)", String::new());
    }
    // The empty-sheet frames above built the pipelines, atlas and buffers
    // every sheet needs: the baseline the sheet's own memory is read against.
    let (rss0, fp0) = (rss_mb(), footprint_mb());

    // Cold: a fresh slot and widget tree each time, the cache-miss cost.
    {
        let overrides = Default::default();
        let filters = Default::default();
        let mut prefs = test_prefs(&overrides, &filters);
        prefs.device_px_per_logical_px = case.dpr;
        prefs.grid_visible = grid_visible();
        let mut p = Phases::default();
        for i in 0..7 {
            let slot = case.slot(false);
            let mut cache = None;
            let f = frame(
                rig,
                case.path,
                &slot,
                prefs,
                &mut cache,
                &[],
                mouse::Cursor::Unavailable,
            );
            if i >= 2 {
                push(&mut p, f);
            }
        }
        report(case, aa, "cold", &p, "(fresh slot)", String::new());
    }

    let (idle, note) = run(case, rig, false, 60, |_, _| {
        (vec![], mouse::Cursor::Unavailable)
    });
    let mem = format!(
        "rss={:.0}MB {}",
        rss_mb(),
        footprint_mb().map_or("n/a".into(), |(t, a)| format!(
            "footprint={t:.0}MB ioaccel={a:.0}MB"
        ))
    );
    report(case, aa, "idle", &idle, &note, mem);

    // Pan: whole pixels, so the retained layer can stand in.
    let (pan, note) = run(case, rig, false, 60, |i, slot| {
        let mut c = slot.camera_mut();
        c.offset.x = cam.offset.x + 6.0 * (i + 1) as f32;
        c.offset.y = cam.offset.y + 4.0 * (i + 1) as f32;
        (vec![], mouse::Cursor::Unavailable)
    });
    report(case, aa, "pan", &pan, &note, String::new());

    // Pan by a fraction of a device pixel each step: no retained reuse.
    let (pan_frac, note) = run(case, rig, false, 30, |i, slot| {
        let mut c = slot.camera_mut();
        c.offset.x = cam.offset.x + 6.37 * (i + 1) as f32;
        c.offset.y = cam.offset.y + 4.21 * (i + 1) as f32;
        (vec![], mouse::Cursor::Unavailable)
    });
    report(case, aa, "pan-frac", &pan_frac, &note, String::new());

    // Zoom about the window centre: every step is another scale.
    let (zoom, note) = run(case, rig, false, 30, |i, slot| {
        let factor = 1.05f32.powi(i as i32 + 1);
        let mut c = slot.camera_mut();
        c.scale = cam.scale * factor;
        c.offset.x = center.x - (center.x - cam.offset.x) * factor;
        c.offset.y = center.y - (center.y - cam.offset.y) * factor;
        (vec![], mouse::Cursor::Unavailable)
    });
    report(case, aa, "zoom", &zoom, &note, String::new());

    // Drag: press on the handle wire, then move the cursor 4 px a frame.
    if case.path.is_interactive() && matches!(case.view, View::Work) {
        let start = iced::Point::new(
            cam.offset.x + case.generated.handle.0 as f32 * cam.scale,
            cam.offset.y + case.generated.handle.1 as f32 * cam.scale,
        );
        let (drag, note) = run(case, rig, true, 60, |i, _| {
            if i == 0 {
                let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
                return (vec![press], at(start));
            }
            let p = iced::Point::new(start.x + 4.0 * i as f32, start.y + 2.0 * i as f32);
            (vec![moved(p)], at(p))
        });
        report(case, aa, "drag", &drag, &note, String::new());
    }
    eprintln!(
        "BENCH mem {:<8} aa={aa} {}: after empty sheet rss {:.0} MB footprint/ioaccel {:?}; \
         after the run rss {:.0} MB (process peak {:.0} MB) footprint/ioaccel {:?} MB",
        case.path.name(),
        match case.view {
            View::Fit => "fit",
            View::Work => "work",
        },
        rss0,
        fp0,
        rss_mb(),
        peak_rss_mb(),
        footprint_mb(),
    );
}

fn scene_report(g: &Generated) {
    let overrides = Default::default();
    let filters = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    // Shared by both paths: lowering the sheet to the render snapshot, once
    // per document revision.
    let mut lower = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        std::hint::black_box(
            crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&g.sheet, 1),
        );
        lower.push(t.elapsed());
    }
    lower.sort();
    eprintln!(
        "BENCH render snapshot (both paths, per document revision) median {:.2} ms",
        lower[2].as_secs_f64() * 1e3
    );
    for view in [View::Fit, View::Work] {
        let case = Case {
            generated: g,
            view,
            path: Path::Gpu,
            dpr: 1.0,
            keep_wires: false,
        };
        let slot = case.slot(false);
        let program = super::gpu_mount::SchematicShaderProgram::new(&slot, prefs);
        let bounds = Rectangle::new(iced::Point::ORIGIN, Size::new(WINDOW.0, WINDOW.1));
        // A cold scene: the cull, the lowering and the instance emit.
        let mut build = Vec::new();
        let mut last = None;
        for _ in 0..7 {
            slot.content_scene.borrow_mut().invalidate();
            let t = Instant::now();
            let prim =
                iced::widget::shader::Program::draw(&program, &(), at(bounds.center()), bounds);
            build.push(t.elapsed());
            last = Some(prim);
        }
        build.drain(..2);
        build.sort();
        let s = &last.expect("built").scene;
        eprintln!(
            "BENCH scene {:<4} cold build median {:.2} ms max {:.2} ms | lines {} circles {} arcs {} \
             polygons {} top_polygons {} drawings {} texts {} (rotated or oversize: {})",
            match view {
                View::Fit => "fit",
                View::Work => "work",
            },
            build[build.len() / 2].as_secs_f64() * 1e3,
            build[build.len() - 1].as_secs_f64() * 1e3,
            s.lines.len(),
            s.circles.len(),
            s.arcs.len(),
            s.polygons.len(),
            s.top_polygons.len(),
            s.drawings.len() + s.drawings_front.len(),
            s.texts.len(),
            s.texts.iter().filter(|t| !t.is_axis_aligned()).count(),
        );
    }
}

#[test]
#[ignore = "benchmark: needs a GPU and a release build"]
fn render_bench() {
    let (cols, rows) = match std::env::var("SIGNEX_BENCH_SIZE").as_deref() {
        Ok("medium") => (12, 10),
        Ok("small") => (6, 5),
        _ => (24, 20),
    };
    let dpr: f32 = std::env::var("SIGNEX_BENCH_DPR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let v = Variant::from_env();
    let g = generate(cols, rows, v);
    eprintln!(
        "BENCH sheet {cols}x{rows} cells, variant text={} rotated={}: {}",
        v.text,
        v.rotated,
        counts(&g.sheet)
    );
    eprintln!(
        "BENCH drag block: {} items; window {}x{} logical at dpr {dpr}",
        g.block.len(),
        WINDOW.0,
        WINDOW.1
    );
    scene_report(&g);
    let list = |var: &str, default: &str| -> Vec<String> {
        std::env::var(var)
            .unwrap_or_else(|_| default.into())
            .split(',')
            .map(str::to_owned)
            .collect()
    };
    let paths = list(
        "SIGNEX_BENCH_PATHS",
        "canvas,gpu,backdrop,shaders,texts,overlay",
    );
    let aas = list("SIGNEX_BENCH_AA", "off,msaa4");
    let views = list("SIGNEX_BENCH_VIEWS", "fit,work");
    eprintln!("BENCH columns are median/p95 ms per frame");
    for aa_name in aas.iter().map(String::as_str) {
        let aa = (aa_name == "msaa4").then_some(Antialiasing::MSAAx4);
        for path in [
            Path::Canvas,
            Path::Gpu,
            Path::Backdrop,
            Path::Shaders,
            Path::Texts,
            Path::Overlay,
        ] {
            if !paths.iter().any(|p| p == path.name()) {
                continue;
            }
            let mut rig = rig(aa, dpr);
            eprintln!("BENCH adapter {}", rig.adapter);
            for (view, name) in [(View::Fit, "fit"), (View::Work, "work")] {
                if !views.iter().any(|v| v == name) {
                    continue;
                }
                let case = Case {
                    generated: &g,
                    view,
                    path,
                    dpr,
                    keep_wires: false,
                };
                measure_case(&case, aa_name, &mut rig);
            }
        }
    }
}
