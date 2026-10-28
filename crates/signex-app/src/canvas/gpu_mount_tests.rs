//! The GPU-mounted schematic, rendered for real (#199 S2).
//!
//! Builds iced's own wgpu renderer headlessly and runs the actual `stack!`
//! through `UserInterface::build` / `draw` / `Renderer::screenshot`. Never
//! tiny-skia: it swaps red and blue and cannot run shader primitives, so it
//! would pass falsely.
//!
//! The checks are coverage and parity measurements that hold on any adapter.
//! Only the exact wire colour is compared on Mesa lavapipe, the way
//! `signex-gfx`'s pixel goldens gate their strict compare. A missing adapter
//! fails loudly, and `SIGNEX_REQUIRE_LAVAPIPE=1` (set by CI) also fails closed
//! on a non-lavapipe one.

use iced::advanced::renderer::Style;
use iced::{Color, Element, Font, Pixels, Size, Theme};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use iced_wgpu::wgpu;
use signex_types::schematic::{
    FillType, HAlign, Junction, Label, LabelType, Point, SchDrawing, SchematicSheet, SelectedItem,
    SelectedKind, VAlign, Wire,
};

use super::tests::test_prefs;
use super::*;

const WIDTH: u32 = 260;
const HEIGHT: u32 = 200;
const SCALE: f32 = 8.0;
const OFFSET: f32 = 20.0;
/// The colour behind everything: what a layer that paints no background
/// leaves showing through.
const CLEAR: Color = Color::from_rgb(0.0, 0.0, 0.0);

struct Gpu {
    renderer: iced::Renderer,
    is_lavapipe: bool,
}

/// The renderer. A machine with no adapter fails loudly instead.
fn gpu() -> Gpu {
    let require = std::env::var_os("SIGNEX_REQUIRE_LAVAPIPE").is_some_and(|v| v == "1");
    let instance = wgpu::Instance::default();
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));
    // A missing adapter fails loudly, as in the other GPU tests: a pixel test
    // that skips itself is a test nobody notices has stopped running.
    let adapter = adapter.expect("no wgpu adapter: the GPU mount pixel tests need one");
    let info = adapter.get_info();
    let name = info.name.to_lowercase();
    let is_lavapipe = name.contains("llvmpipe") || name.contains("lavapipe");
    assert!(
        is_lavapipe || !require,
        "SIGNEX_REQUIRE_LAVAPIPE=1 but adapter `{}` is not lavapipe/llvmpipe",
        info.name
    );
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("gpu_mount_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("device");
    let engine = iced_wgpu::Engine::new(
        &adapter,
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        None,
        iced::advanced::graphics::Shell::headless(),
    );
    // The app loads its faces through the iced builder; a test must do it.
    let mut fonts = iced::advanced::graphics::text::font_system()
        .write()
        .expect("font system");
    for face in crate::fonts::bundled::BUNDLED_FONTS {
        fonts.load_font(std::borrow::Cow::Borrowed(face));
    }
    drop(fonts);
    Gpu {
        renderer: iced::Renderer::Primary(iced_wgpu::Renderer::new(
            engine,
            Font::DEFAULT,
            Pixels(16.0),
        )),
        is_lavapipe,
    }
}

/// An RGBA frame, `width` physical pixels across.
struct Frame {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

impl Frame {
    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    /// Pixels in the box `[x0, x1) x [y0, y1)` that differ from `other`.
    fn changed_vs(&self, other: &Frame, (x0, y0, x1, y1): (u32, u32, u32, u32)) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        for y in y0..y1.min(self.height) {
            for x in x0..x1.min(self.width) {
                let (a, b) = (self.px(x, y), other.px(x, y));
                if (0..3).any(|c| a[c].abs_diff(b[c]) > 24) {
                    out.push((x, y));
                }
            }
        }
        out
    }
}

fn render(gpu: &mut Gpu, element: Element<'_, crate::app::Message>) -> Frame {
    render_at(gpu, element, 1.0)
}

/// `element` laid out in the `WIDTH` x `HEIGHT` logical window at `dpi`.
fn render_at(gpu: &mut Gpu, element: Element<'_, crate::app::Message>, dpi: f32) -> Frame {
    let size = Size::new(WIDTH as f32, HEIGHT as f32);
    let mut ui = UserInterface::build(element, size, Cache::default(), &mut gpu.renderer);
    ui.draw(
        &mut gpu.renderer,
        &Theme::Dark,
        &Style {
            text_color: Color::WHITE,
        },
        iced::mouse::Cursor::Unavailable,
    );
    let (width, height) = ((WIDTH as f32 * dpi) as u32, (HEIGHT as f32 * dpi) as u32);
    let viewport =
        iced::advanced::graphics::Viewport::with_physical_size(Size::new(width, height), dpi);
    let iced::Renderer::Primary(wgpu_renderer) = &mut gpu.renderer else {
        unreachable!("built as Primary above")
    };
    Frame {
        data: wgpu_renderer.screenshot(&viewport, CLEAR),
        width,
        height,
    }
}

fn world(x: f32, y: f32) -> (u32, u32) {
    ((OFFSET + x * SCALE) as u32, (OFFSET + y * SCALE) as u32)
}

fn label(text: &str, x: f64, y: f64, rotation: f64) -> Label {
    Label {
        uuid: uuid::Uuid::new_v4(),
        text: text.into(),
        position: Point::new(x, y),
        rotation,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
    }
}

/// A wire, an arc bulging up, a sub-pixel junction dot, a horizontal label and a
/// label turned 90 degrees.
pub(super) fn fixture() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(5.0, 5.0),
        end: Point::new(20.0, 5.0),
        stroke_width: 0.25,
        unknown_columns: Default::default(),
    });
    sheet.drawings.push(SchDrawing::Arc {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(5.0, 13.0),
        mid: Point::new(10.0, 10.0),
        end: Point::new(15.0, 13.0),
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
    });
    // A junction dot, in the circle bucket, far below the radius floor.
    sheet.junctions.push(Junction {
        uuid: uuid::Uuid::new_v4(),
        position: Point::new(28.0, 5.0),
        diameter: 0.06,
        minted: false,
        unknown_columns: Default::default(),
    });
    sheet.labels.push(label("NETA", 5.0, 21.0, 0.0));
    sheet.labels.push(label("NETB", 27.0, 21.0, 90.0));
    sheet
}

pub(super) fn slot_with(sheet: &SchematicSheet) -> CanvasSlot {
    let mut slot = CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(sheet),
    ));
    *slot.camera_mut() = Camera {
        offset: iced::Point::new(OFFSET, OFFSET),
        scale: SCALE,
    };
    slot
}

/// Which element to render: the whole GPU stack, one of its layers, or the
/// plain CPU canvas.
enum Layer {
    Stack,
    Backdrop,
    Overlay,
    Cpu,
}

fn fill<'a, P: canvas::Program<crate::app::Message> + 'a>(
    program: P,
) -> Element<'a, crate::app::Message> {
    canvas(program)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

fn frame_of(gpu: &mut Gpu, layer: Layer, sheet: &SchematicSheet) -> Frame {
    frame_with(gpu, layer, sheet, &Look::default())
}

/// What differs between the frames a test compares.
#[derive(Clone, Copy)]
struct Look {
    /// Select the sheet's first wire.
    select_wire: bool,
    /// Focus the first label, and keep the wire in the AutoFocus set without
    /// giving the focus box a corner there: a `SymbolRefField` that names no
    /// symbol is in the set (by uuid) but has no bounds. The scene then leaves
    /// the far-away wire unfaded, so only the overlay's dim can darken it.
    focus_label: bool,
    auto_focus: bool,
    dpi: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            select_wire: false,
            focus_label: false,
            auto_focus: false,
            dpi: 1.0,
        }
    }
}

fn frame_with(gpu: &mut Gpu, layer: Layer, sheet: &SchematicSheet, look: &Look) -> Frame {
    let mut slot = slot_with(sheet);
    if look.select_wire {
        let wire = sheet.wires[0].uuid;
        slot.selected = vec![SelectedItem::new(wire, SelectedKind::Wire)];
    }
    if look.focus_label {
        let (label, wire) = (sheet.labels[0].uuid, sheet.wires[0].uuid);
        slot.selected = vec![
            SelectedItem::new(label, SelectedKind::Label),
            SelectedItem::new(wire, SelectedKind::SymbolRefField),
        ];
    }
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.grid_visible = false;
    prefs.auto_focus = look.auto_focus;
    let dpi = look.dpi;
    match layer {
        Layer::Stack => render_at(gpu, gpu_mount::view(&slot, prefs), dpi),
        Layer::Backdrop => render(gpu, fill(gpu_mount::SchematicBackdrop::new(&slot, prefs))),
        Layer::Overlay => render(gpu, fill(SchematicCanvas::new(&slot, prefs).gpu_overlay())),
        Layer::Cpu => render_at(gpu, fill(SchematicCanvas::new(&slot, prefs)), dpi),
    }
}

fn region(x: f32, y: f32, half_w: f32, half_h: f32) -> (u32, u32, u32, u32) {
    let (px, py) = world(x, y);
    (
        px.saturating_sub((half_w * SCALE) as u32),
        py.saturating_sub((half_h * SCALE) as u32),
        px + (half_w * SCALE) as u32,
        py + (half_h * SCALE) as u32,
    )
}

fn to_u8(c: signex_types::theme::Color) -> [u8; 3] {
    [c.r, c.g, c.b]
}

/// The fixture's regions, in window pixels.
struct Regions {
    wire: (u32, u32, u32, u32),
    arc: (u32, u32, u32, u32),
    circle: (u32, u32, u32, u32),
    horizontal: (u32, u32, u32, u32),
    vertical: (u32, u32, u32, u32),
}

fn regions() -> Regions {
    Regions {
        wire: region(12.5, 5.0, 8.0, 0.6),
        arc: region(10.0, 11.6, 6.0, 2.4),
        circle: region(28.0, 5.0, 0.8, 0.8),
        horizontal: region(8.0, 20.0, 4.0, 1.6),
        vertical: region(27.0, 19.0, 1.8, 4.0),
    }
}

/// How far the GPU glyph atlas may place a label's edge from the CPU text.
const TEXT_TOLERANCE_PX: u32 = 3;

/// `[min_x, min_y, max_x, max_y]` of `points`.
fn bbox(points: &[(u32, u32)]) -> [u32; 4] {
    points
        .iter()
        .fold([u32::MAX, u32::MAX, 0, 0], |b, &(x, y)| {
            [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
        })
}

fn centroid(points: &[(u32, u32)]) -> (f32, f32) {
    let n = points.len() as f32;
    let (sx, sy) = points.iter().fold((0.0, 0.0), |(sx, sy), &(x, y)| {
        (sx + x as f32, sy + y as f32)
    });
    (sx / n, sy / n)
}

#[test]
fn fixture_has_a_rotated_text_the_fallback_must_draw() {
    // Without this the fallback assertions below would pass vacuously.
    let sheet = fixture();
    let slot = slot_with(&sheet);
    let overrides = Default::default();
    let filters = Default::default();
    let canvas = SchematicCanvas::new(&slot, test_prefs(&overrides, &filters));
    let mut cache = slot.content_scene.borrow_mut();
    let transform = crate::schematic_runtime::ScreenTransform {
        offset_x: OFFSET,
        offset_y: OFFSET,
        scale: SCALE,
    };
    let bounds = Rectangle::new(
        iced::Point::ORIGIN,
        iced::Size::new(WIDTH as f32, HEIGHT as f32),
    );
    let scene = cache
        .scene_for(
            slot.active_snapshot().expect("snapshot"),
            &transform,
            &canvas.prefs.canvas_colors,
            bounds,
            None,
            Default::default(),
        )
        .expect("scene")
        .clone();
    let rotated: Vec<&str> = crate::renderer_scene_canvas::rotated_texts(&scene)
        .map(|t| t.content.as_str())
        .collect();
    assert_eq!(rotated, ["NETB"], "the 90 degree label is rotated text");
    assert!(
        scene.texts.iter().any(|t| t.content == "NETA"),
        "the horizontal label is axis-aligned text on the GPU"
    );
}

#[test]
fn gpu_stack_draws_the_sheet_and_matches_the_cpu_replay() {
    let mut gpu = gpu();
    let r = regions();
    let empty = SchematicSheet::default();
    let stack = frame_of(&mut gpu, Layer::Stack, &fixture());
    let blank = frame_of(&mut gpu, Layer::Stack, &empty);
    let cpu = frame_of(&mut gpu, Layer::Cpu, &fixture());
    let cpu_blank = frame_of(&mut gpu, Layer::Cpu, &empty);

    // Wire: exact colour only where the rasteriser is the reference one.
    let (wx, wy) = world(12.5, 5.0);
    if gpu.is_lavapipe {
        let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        let got = stack.px(wx, wy);
        let want = to_u8(colors.wire);
        assert!(
            (0..3).all(|c| got[c].abs_diff(want[c]) <= 24),
            "wire pixel {got:?} is not the wire colour {want:?}"
        );
    }
    assert!(
        !stack.changed_vs(&blank, r.wire).is_empty(),
        "no wire on the GPU"
    );

    // Axis-aligned label: GPU glyphs, where the CPU replay puts them.
    let horizontal = stack.changed_vs(&blank, r.horizontal);
    let cpu_horizontal = cpu.changed_vs(&cpu_blank, r.horizontal);
    assert!(
        !horizontal.is_empty() && !cpu_horizontal.is_empty(),
        "label missing: GPU {}, CPU {}",
        horizontal.len(),
        cpu_horizontal.len()
    );
    let (gpu_box, cpu_box) = (bbox(&horizontal), bbox(&cpu_horizontal));
    assert!(
        gpu_box
            .iter()
            .zip(cpu_box)
            .all(|(g, c)| g.abs_diff(c) <= TEXT_TOLERANCE_PX),
        "GPU label bbox {gpu_box:?} is not the CPU's {cpu_box:?}"
    );

    // Rotated label: only the CPU fallback can have drawn these.
    let vertical = stack.changed_vs(&blank, r.vertical);
    assert!(
        vertical.len() > 20,
        "rotated label missing: {}",
        vertical.len()
    );

    // A sub-pixel dot survives on the GPU through the radius floor.
    let circle = stack.changed_vs(&blank, r.circle);
    let cpu_circle = cpu.changed_vs(&cpu_blank, r.circle);
    assert!(!cpu_circle.is_empty(), "the CPU floors a tiny dot");
    assert!(
        !circle.is_empty(),
        "the GPU lost a sub-pixel dot (radius floor)"
    );

    // Arc: same side, same place as the CPU. A flipped Y or a mirrored sweep
    // moves the centroid across the chord.
    let gpu_arc = stack.changed_vs(&blank, r.arc);
    let cpu_arc = cpu.changed_vs(&cpu_blank, r.arc);
    assert!(!gpu_arc.is_empty() && !cpu_arc.is_empty(), "arc missing");
    let (_, gy) = centroid(&gpu_arc);
    let (_, cy) = centroid(&cpu_arc);
    let (_, chord_y) = world(10.0, 13.0);
    assert!(cy < chord_y as f32, "the CPU arc bulges up (Y-down)");
    assert!(
        (gy - cy).abs() < 3.0,
        "GPU arc centroid y {gy} differs from the CPU's {cy}"
    );
}

#[test]
fn layers_split_the_work_with_no_overlap() {
    let mut gpu = gpu();
    let r = regions();
    let backdrop = frame_of(&mut gpu, Layer::Backdrop, &fixture());
    let overlay = frame_of(&mut gpu, Layer::Overlay, &fixture());
    let clear = frame_of(&mut gpu, Layer::Overlay, &SchematicSheet::default());
    let clear_rgb = [CLEAR.r, CLEAR.g, CLEAR.b].map(|c| (c * 255.0).round() as u8);

    // The backdrop paints paper everywhere and no content.
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
    let paper = to_u8(colors.paper);
    for (x, y) in [
        (wx(12.5), wy(5.0)),
        (wx(8.0), wy(20.0)),
        (wx(27.0), wy(19.0)),
    ] {
        let got = backdrop.px(x, y);
        assert!(
            (0..3).all(|c| got[c].abs_diff(paper[c]) <= 3),
            "backdrop at ({x},{y}) is {got:?}, not paper {paper:?}"
        );
    }

    // The overlay paints no background and no base primitive: only the
    // rotated label differs from the bare clear colour.
    for (x, y) in [(1, 1), (WIDTH - 2, HEIGHT - 2), (wx(12.5), wy(5.0))] {
        let got = overlay.px(x, y);
        assert!(
            (0..3).all(|c| got[c] == clear_rgb[c]),
            "the overlay painted a background at ({x},{y}): {got:?} is not CLEAR {clear_rgb:?}"
        );
    }
    assert!(
        overlay.changed_vs(&clear, r.wire).is_empty(),
        "overlay drew the wire"
    );
    assert!(
        overlay.changed_vs(&clear, r.arc).is_empty(),
        "overlay drew the arc"
    );
    assert!(
        overlay.changed_vs(&clear, r.horizontal).is_empty(),
        "overlay drew the GPU's axis-aligned text"
    );
    assert!(
        overlay.changed_vs(&clear, r.vertical).len() > 20,
        "overlay lacks the rotated-text fallback"
    );
}

fn wx(x: f32) -> u32 {
    world(x, 0.0).0
}

fn wy(y: f32) -> u32 {
    world(0.0, y).1
}

/// `stack!` hands every event to every child: a second `update` here would
/// place two symbols per click.
#[test]
fn backdrop_neither_emits_nor_captures_any_event() {
    use iced::keyboard::{self, Key, Modifiers, key::Named};
    use iced::{Event, mouse, touch, window};
    let slot = CanvasSlot::new();
    let overrides = Default::default();
    let filters = Default::default();
    let backdrop = gpu_mount::SchematicBackdrop::new(&slot, test_prefs(&overrides, &filters));
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(100.0, 100.0));
    let cursor = mouse::Cursor::Available(iced::Point::new(10.0, 10.0));
    let events = [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)),
        Event::Mouse(mouse::Event::CursorMoved {
            position: iced::Point::new(12.0, 12.0),
        }),
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        }),
        Event::Mouse(mouse::Event::CursorLeft),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            modified_key: Key::Named(Named::Escape),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
            location: keyboard::Location::Standard,
            modifiers: Modifiers::default(),
            text: None,
            repeat: false,
        }),
        Event::Keyboard(keyboard::Event::ModifiersChanged(Modifiers::SHIFT)),
        Event::Touch(touch::Event::FingerPressed {
            id: touch::Finger(0),
            position: iced::Point::new(5.0, 5.0),
        }),
        Event::Window(window::Event::Focused),
    ];
    for event in &events {
        let action = canvas::Program::update(&backdrop, &mut (), event, bounds, cursor);
        assert!(action.is_none(), "backdrop reacted to {event:?}");
    }
}

/// Brightness of a pixel.
fn luma(px: [u8; 4]) -> u32 {
    px[..3].iter().map(|&c| u32::from(c)).sum()
}

/// `stack!` draws children i>0 inside `renderer.with_layer`, so the overlay's
/// selection highlight lands above the GPU primitive. If that ever stopped
/// holding, the wire pixel would read the same selected or not.
#[test]
fn the_selection_highlight_draws_above_the_gpu_content() {
    let mut gpu = gpu();
    let sheet = fixture();
    let (wx, wy) = world(12.5, 5.0);
    let plain = frame_of(&mut gpu, Layer::Stack, &sheet).px(wx, wy);
    let look = Look {
        select_wire: true,
        ..Look::default()
    };
    let selected = frame_with(&mut gpu, Layer::Stack, &sheet, &look).px(wx, wy);
    assert!(
        (0..3).all(|c| selected[c] >= plain[c]) && luma(selected) > luma(plain) + 8,
        "wire pixel {selected:?} shows no highlight over the unselected {plain:?}"
    );
}

/// The AutoFocus dim is the overlay's too: it must darken the GPU's wire (far
/// outside the focused label's box), not only the paper under it.
#[test]
fn the_auto_focus_dim_draws_above_the_gpu_content() {
    let mut gpu = gpu();
    let sheet = fixture();
    let lit_look = Look {
        focus_label: true,
        ..Look::default()
    };
    let dim_look = Look {
        auto_focus: true,
        ..lit_look
    };
    let (wx, wy) = world(12.5, 5.0);
    let lit = luma(frame_with(&mut gpu, Layer::Stack, &sheet, &lit_look).px(wx, wy));
    let dim = luma(frame_with(&mut gpu, Layer::Stack, &sheet, &dim_look).px(wx, wy));
    // A dim underneath the opaque wire leaves it as it was; only one on top
    // lowers it.
    assert!(
        dim * 100 < lit * 75,
        "the GPU wire is not dimmed: {dim} vs {lit}"
    );
}

/// At 2x the shader's DPI maths must put the wire where the CPU backdrop and
/// the overlay put it: twice the world position in physical pixels.
#[test]
fn a_hidpi_window_puts_the_wire_at_twice_the_pixel() {
    let mut gpu = gpu();
    let sheet = fixture();
    let look = Look {
        dpi: 2.0,
        ..Look::default()
    };
    let stack = frame_with(&mut gpu, Layer::Stack, &sheet, &look);
    let blank = frame_with(&mut gpu, Layer::Stack, &SchematicSheet::default(), &look);
    let (wx, wy) = world(12.5, 5.0);
    let wire_row = region(12.5, 5.0, 8.0, 0.6);
    let doubled = (
        wire_row.0 * 2,
        wire_row.1 * 2,
        wire_row.2 * 2,
        wire_row.3 * 2,
    );
    let changed = stack.changed_vs(&blank, doubled);
    assert!(!changed.is_empty(), "no wire at 2x");
    let (_, cy) = centroid(&changed);
    assert!(
        (cy - (wy * 2) as f32).abs() <= 2.0,
        "wire row centroid {cy} is not at 2x the world y {}",
        wy * 2
    );
    let (min_x, _, max_x, _) = (
        changed.iter().map(|p| p.0).min().unwrap_or(0),
        0,
        changed.iter().map(|p| p.0).max().unwrap_or(0),
        0,
    );
    let (start, end) = (world(5.0, 5.0).0 * 2, world(20.0, 5.0).0 * 2);
    assert!(
        min_x.abs_diff(start) <= 4 && max_x.abs_diff(end) <= 4,
        "wire spans {min_x}..{max_x}, not 2x {start}..{end}"
    );
    if gpu.is_lavapipe {
        let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        let got = stack.px(wx * 2, wy * 2);
        let want = to_u8(colors.wire);
        assert!(
            (0..3).all(|c| got[c].abs_diff(want[c]) <= 24),
            "2x wire pixel {got:?} is not the wire colour {want:?}"
        );
    }
}
