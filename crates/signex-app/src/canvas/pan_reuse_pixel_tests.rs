//! The retained content layer lines up with the plain canvas, pixel for pixel
//! (#2078), on a canvas that is not at the window origin and at fractional
//! display scales.
//!
//! The renderer snaps each mesh by its clip bounds, so a content layer whose
//! clip starts elsewhere than the other layers' could land a fraction of a
//! pixel off them. The reference is the plain `canvas(SchematicCanvas)` mount
//! of trunk, whose layers all share the canvas-origin clip. Rendered through
//! iced's wgpu renderer (the harness of `gpu_mount_tests`).

use iced::advanced::renderer::Style;
use iced::widget::Widget as _;
use iced::{Color, Element, Length, Padding, Size, Theme};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

use super::gpu_mount_tests::{Frame, Gpu, fixture, gpu, slot_with};
use super::tests::test_prefs;
use super::*;

const SCALE: f32 = 8.0;

/// A window with the canvas padded off its origin, as in the dock.
#[derive(Clone, Copy)]
struct Mount {
    window: (u32, u32),
    pad: (f32, f32),
    dpi: f32,
}

/// The big dock-like window.
const DOCKED: Mount = Mount {
    window: (1920, 1080),
    pad: (245.0, 67.0),
    dpi: 1.0,
};

#[derive(Clone, Copy, PartialEq)]
enum Path {
    /// Trunk's mount.
    Plain,
    /// The retained-layer widget.
    Surface,
}

fn render_in(gpu: &mut Gpu, mount: Mount, element: Element<'_, crate::app::Message>) -> Frame {
    let element = iced::widget::container(element)
        .padding(Padding {
            top: mount.pad.1,
            left: mount.pad.0,
            ..Padding::ZERO
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    let size = Size::new(mount.window.0 as f32, mount.window.1 as f32);
    let mut ui = UserInterface::build(element, size, Cache::default(), &mut gpu.renderer);
    ui.draw(
        &mut gpu.renderer,
        &Theme::Dark,
        &Style {
            text_color: Color::WHITE,
        },
        iced::mouse::Cursor::Unavailable,
    );
    let physical = Size::new(
        (size.width * mount.dpi) as u32,
        (size.height * mount.dpi) as u32,
    );
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(
        physical,
        iced::advanced::renderer::Scale {
            window: mount.dpi,
            application: 1.0,
        },
    );
    let iced::Renderer::Primary(wgpu_renderer) = &mut gpu.renderer else {
        unreachable!("the harness builds a wgpu renderer")
    };
    Frame {
        data: wgpu_renderer.screenshot(&viewport, Color::BLACK),
        width: physical.width,
        height: physical.height,
    }
}

fn widget<'a>(
    path: Path,
    slot: &'a CanvasSlot,
    prefs: CanvasViewPrefs<'a>,
) -> Element<'a, crate::app::Message> {
    let program = SchematicCanvas::new(slot, prefs);
    match path {
        Path::Plain => iced::widget::canvas(program)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        Path::Surface => SchematicSurface::new(program).boxed(),
    }
}

/// The camera one frame is drawn at, and the pans that lead to it.
struct Run {
    start: iced::Point,
    pans: Vec<(f32, f32)>,
    select: bool,
}

/// Render `sheet` through `path`. The surface renders the start camera and
/// then every pan, so its last frame is built from retained geometry; the
/// plain path renders only the last camera. Returns the last frame and how
/// many times the surface tessellated its content.
fn last_frame(
    gpu: &mut Gpu,
    path: Path,
    mount: Mount,
    sheet: &SchematicSheet,
    run: &Run,
) -> (Frame, u32) {
    let mut slot = slot_with(sheet);
    if run.select {
        slot.selected = vec![SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire)];
    }
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.grid_visible = false;
    prefs.device_px_per_logical_px = mount.dpi;
    let mut offset = run.start;
    let mut frames = vec![offset];
    for &(dx, dy) in &run.pans {
        offset.x += dx;
        offset.y += dy;
        frames.push(offset);
    }
    if path == Path::Plain {
        frames.drain(..frames.len() - 1);
    }
    let mut last = None;
    for offset in frames {
        *slot.camera_mut() = Camera {
            offset,
            scale: SCALE,
        };
        last = Some(render_in(gpu, mount, widget(path, &slot, prefs)));
    }
    let rebuilds = slot.content_surface.borrow().rebuilds();
    (last.expect("at least one frame"), rebuilds)
}

fn differing(a: &Frame, b: &Frame) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for y in 0..a.height {
        for x in 0..a.width {
            let (p, q) = (a.px(x, y), b.px(x, y));
            if (0..3).any(|c| p[c].abs_diff(q[c]) > 24) {
                out.push((x, y));
            }
        }
    }
    out
}

/// The frame is not blank: the sheet's wires, arc and circles are in it.
fn assert_drawn(gpu: &mut Gpu, mount: Mount, run: &Run, frame: &Frame) {
    let blank = last_frame(gpu, Path::Plain, mount, &SchematicSheet::default(), run).0;
    let sheet_pixels = differing(frame, &blank).len();
    assert!(
        sheet_pixels > 300,
        "only {sheet_pixels} pixels differ from a blank sheet: the content was not drawn"
    );
}

fn assert_lines_up(mount: Mount, run: Run) {
    let mut gpu = gpu();
    let sheet = fixture();
    let (plain, _) = last_frame(&mut gpu, Path::Plain, mount, &sheet, &run);
    assert_drawn(&mut gpu, mount, &run, &plain);
    let (surface, rebuilds) = last_frame(&mut gpu, Path::Surface, mount, &sheet, &run);
    assert_eq!(rebuilds, 1, "the pans rebuilt the content");
    let off = differing(&surface, &plain);
    assert!(
        off.is_empty(),
        "{} pixels differ from the plain canvas, first at {:?}",
        off.len(),
        off.first()
    );
}

fn run(select: bool, pans: Vec<(f32, f32)>) -> Run {
    Run {
        start: iced::Point::new(20.0, 20.0),
        pans,
        select,
    }
}

/// `n` steps of `device_px` device pixels, in logical pixels.
fn steps(n: usize, device_px: f32, dpi: f32) -> Vec<(f32, f32)> {
    vec![(device_px / dpi, device_px / dpi * 0.5); n]
}

#[test]
fn the_docked_canvas_lines_up_before_any_pan() {
    assert_lines_up(DOCKED, run(false, vec![]));
}

#[test]
fn the_docked_canvas_lines_up_after_a_pan() {
    assert_lines_up(DOCKED, run(false, steps(5, 4.0, 1.0)));
}

#[test]
fn the_selection_lines_up_with_the_content_after_a_pan() {
    assert_lines_up(DOCKED, run(true, steps(5, 4.0, 1.0)));
}

#[test]
fn a_fractional_display_scale_lines_up() {
    for dpi in [1.25, 1.5] {
        let mount = Mount { dpi, ..DOCKED };
        assert_lines_up(mount, run(true, steps(5, 4.0, dpi)));
    }
}

/// A pan of almost the whole margin in one step, on a window small enough for
/// the sheet to come into view at the far end.
#[test]
fn a_pan_to_the_edge_of_the_margin_lines_up() {
    for dpi in [1.0, 1.25] {
        let mount = Mount {
            window: (400, 300),
            pad: (60.0, 67.0),
            dpi,
        };
        // The viewport is 340 logical px wide; the margin is that, whole
        // device pixels. Start with the sheet to the right of it.
        let margin = (340.0 * dpi).floor() / dpi;
        let one_px = 1.0 / dpi;
        let run = Run {
            start: iced::Point::new(330.0, 20.0),
            pans: vec![(-(margin - one_px), 0.0)],
            select: true,
        };
        assert_lines_up(mount, run);
    }
}
