//! The retained content layer lines up with the plain canvas, pixel for pixel
//! (#2078), on a canvas that is not at the window origin and at fractional
//! display scales.
//!
//! The reference is trunk's own frame: the plain `canvas(SchematicCanvas)`
//! mount with the content drawn in a frame of the canvas size (clip at the
//! canvas origin), where the retained layer's clip starts a margin further up
//! and left. Rendered through iced's wgpu renderer (the harness of `gpu_mount_tests`).

use iced::advanced::renderer::Style;
use iced::widget::Widget as _;
use iced::{Color, Element, Length, Padding, Size, Theme};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_types::schematic::{HAlign, SchematicSheet, SelectedItem, SelectedKind};

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
    /// Trunk's mount, with the content drawn as trunk drew it.
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
    if run.select
        && let Some(wire) = sheet.wires.first()
    {
        slot.selected = vec![SelectedItem::new(wire.uuid, SelectedKind::Wire)];
    }
    // The reference draws its content as trunk did: a frame of the canvas
    // size, so its clip starts at the canvas origin.
    slot.trunk_content.set(path == Path::Plain);
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
        // What the app does when the view moves (`CanvasEvent::ViewMoved`):
        // the selection overlay is cached and follows the camera only
        // through this.
        slot.clear_overlay_cache();
        slot.clear_bg_cache();
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

fn max_channel_diff(a: &Frame, b: &Frame) -> u8 {
    a.data
        .iter()
        .zip(&b.data)
        .map(|(p, q)| p.abs_diff(*q))
        .max()
        .unwrap_or(0)
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

/// Where the last frame's camera puts world `(x, y)` mm, in physical pixels.
fn window_px(mount: Mount, run: &Run, x: f32, y: f32) -> (f32, f32) {
    let (dx, dy) = run
        .pans
        .iter()
        .fold((0.0, 0.0), |(ax, ay), (px, py)| (ax + px, ay + py));
    (
        (mount.pad.0 + run.start.x + dx + x * SCALE) * mount.dpi,
        (mount.pad.1 + run.start.y + dy + y * SCALE) * mount.dpi,
    )
}

/// Centre `(x, y)` of the pixels near `want` inside `[x0, x1] x [y0, y1]`,
/// with how many there are.
fn centre_of(
    frame: &Frame,
    want: [u8; 3],
    (x0, y0, x1, y1): (f32, f32, f32, f32),
) -> Option<((f32, f32), usize)> {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0usize);
    for y in y0.max(0.0) as u32..(y1 as u32).min(frame.height) {
        for x in x0.max(0.0) as u32..(x1 as u32).min(frame.width) {
            let px = frame.px(x, y);
            if (0..3).all(|c| px[c].abs_diff(want[c]) <= 24) {
                sx += x as f32 + 0.5;
                sy += y as f32 + 0.5;
                n += 1;
            }
        }
    }
    (n > 0).then(|| ((sx / n as f32, sy / n as f32), n))
}

/// Offset of the selection highlight's centre from the centre of the second
/// wire (the first one is mostly under the highlight), read off one frame: the
/// two layers drawn together.
fn selection_offset(frame: &Frame, mount: Mount, run: &Run) -> (f32, f32) {
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteLight);
    let of = |c: signex_types::theme::Color| [c.r, c.g, c.b];
    let band = |x0, y0, x1, y1| {
        let (a, b) = window_px(mount, run, x0, y0);
        let (c, d) = window_px(mount, run, x1, y1);
        (a, b, c, d)
    };
    // The first wire runs (5, 5) to (20, 5) mm and the highlight sits around
    // it; the second runs (5, 8) to (20, 8) mm, clear of the highlight.
    let (wire, wire_n) =
        centre_of(frame, of(colors.wire), band(4.0, 7.0, 21.0, 9.0)).expect("no wire pixels");
    let (sel, sel_n) = centre_of(
        frame,
        of(signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight).accent),
        band(4.0, 3.5, 21.0, 6.5),
    )
    .expect("no selection pixels");
    eprintln!("wire centre {wire:?} ({wire_n} px), selection centre {sel:?} ({sel_n} px)");
    (sel.0 - wire.0, sel.1 - wire.1)
}

/// Where and how the two frames differ, by the colour class each shows there.
fn describe_differences(surface: &Frame, trunk: &Frame, differing: &[(u32, u32)]) {
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteLight);
    let classes = [
        ("wire", colors.wire),
        (
            "selection",
            signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight).accent,
        ),
        ("paper", colors.paper),
        ("background", colors.background),
    ];
    let class = |px: [u8; 4]| {
        classes
            .iter()
            .find(|(_, c)| {
                [c.r, c.g, c.b]
                    .iter()
                    .zip(px)
                    .all(|(a, b)| a.abs_diff(b) <= 24)
            })
            .map_or("other", |(name, _)| *name)
    };
    let mut groups: std::collections::BTreeMap<(&str, &str), (usize, [u32; 4])> =
        Default::default();
    for &(x, y) in differing {
        let key = (class(surface.px(x, y)), class(trunk.px(x, y)));
        let entry = groups.entry(key).or_insert((0, [u32::MAX, u32::MAX, 0, 0]));
        entry.0 += 1;
        entry.1 = [
            entry.1[0].min(x),
            entry.1[1].min(y),
            entry.1[2].max(x),
            entry.1[3].max(y),
        ];
    }
    for ((got, want), (n, [x0, y0, x1, y1])) in groups {
        eprintln!("  surface {got} where trunk has {want}: {n} px in [{x0},{y0}]-[{x1},{y1}]");
    }
}

fn assert_lines_up(mount: Mount, run: Run) {
    let mut gpu = gpu();
    let sheet = fixture();
    assert!(!sheet.wires.is_empty(), "the fixture has a wire");
    let (trunk, _) = last_frame(&mut gpu, Path::Plain, mount, &sheet, &run);
    assert_drawn(&mut gpu, mount, &run, &trunk);
    let (surface, rebuilds) = last_frame(&mut gpu, Path::Surface, mount, &sheet, &run);
    assert_eq!(rebuilds, 1, "the pans rebuilt the content");
    let off = differing(&surface, &trunk);
    eprintln!(
        "dpi {}: {} pixels differ from trunk, max channel diff {}",
        mount.dpi,
        off.len(),
        max_channel_diff(&surface, &trunk)
    );
    describe_differences(&surface, &trunk, &off);
    if run.select {
        let (tx, ty) = selection_offset(&trunk, mount, &run);
        let (sx, sy) = selection_offset(&surface, mount, &run);
        eprintln!("selection minus wire: trunk ({tx:.2}, {ty:.2}), surface ({sx:.2}, {sy:.2})");
        let allowed = 0.5 * mount.dpi;
        assert!(
            (sx - tx).abs() <= allowed && (sy - ty).abs() <= allowed,
            "the selection sits ({:.2}, {:.2}) px off the wire against trunk",
            sx - tx,
            sy - ty
        );
    }
    assert!(
        off.is_empty(),
        "{} pixels differ from trunk, first at {:?}",
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

/// Pans of 8 and 4 logical px: whole device pixels at every scale below
/// (8 * 1.25, 8 * 1.5 and 8 * 1.75 are whole), and exact in f32, so the camera
/// the plain canvas draws at carries no rounding noise.
///
/// A camera that does carry some (a pan of `4 / 1.5` px, five times over) can
/// put an edge exactly on a half device pixel, where the hairline snap rounds
/// by the noise: trunk's frame then differs from the retained one, which was
/// snapped at an exact offset, by the pixel at that edge.
#[test]
fn a_fractional_display_scale_lines_up() {
    for dpi in [1.25, 1.5, 1.75, 2.0] {
        let mount = Mount { dpi, ..DOCKED };
        assert_lines_up(mount, run(true, vec![(8.0, 4.0); 5]));
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

/// An oversize text (a 1100 px em, right-justified) whose anchor sits at
/// x = -1500 px at the start camera: inside the retained layer's left margin,
/// which is a viewport wide, and outside the viewport itself.
fn oversize_text_in_the_margin() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.text_notes.push(oversize_note(-1500.0, HAlign::Right));
    sheet
}

/// An "H" with a 1100 px em, bottom-anchored at (`x`, 900) px at scale
/// [`SCALE`] from the origin camera.
fn oversize_note(x: f32, justify_h: HAlign) -> signex_types::schematic::TextNote {
    use signex_types::schematic::{Point as WorldPoint, TextNote, VAlign};
    TextNote {
        uuid: uuid::Uuid::now_v7(),
        text: "H".into(),
        position: WorldPoint::new(f64::from(x / SCALE), f64::from(900.0 / SCALE)),
        rotation: 0.0,
        font_size: f64::from(1100.0 / SCALE * signex_types::font::mm_per_em(None)),
        font_family: None,
        justify_h,
        justify_v: VAlign::Bottom,
        locked: false,
    }
}

/// More oversize texts in the retained layer's left margin than one frame
/// draws, ahead in document order of one in view: the text in view is drawn
/// into the retained layer as on the plain canvas, which culls the margin
/// (#2254). Claiming in document order, or reading the whole retained frame
/// as the viewport, spends the budget on the margin and leaves it blank.
#[test]
fn margin_texts_ahead_of_one_in_view_do_not_take_its_place_in_the_retained_layer() {
    let mut gpu = gpu();
    let mut text_notes: Vec<_> = (0..=crate::oversize_text::MAX_OVERSIZE_TEXTS_PER_FRAME)
        .map(|_| oversize_note(-1500.0, HAlign::Right))
        .collect();
    text_notes.push(oversize_note(600.0, HAlign::Left));
    let sheet = SchematicSheet {
        text_notes,
        ..SchematicSheet::default()
    };
    let run = Run {
        start: iced::Point::ORIGIN,
        pans: Vec::new(),
        select: false,
    };
    let (plain, _) = last_frame(&mut gpu, Path::Plain, DOCKED, &sheet, &run);
    let (surface, _) = last_frame(&mut gpu, Path::Surface, DOCKED, &sheet, &run);
    let blank = last_frame(
        &mut gpu,
        Path::Plain,
        DOCKED,
        &SchematicSheet::default(),
        &run,
    )
    .0;
    assert!(
        differing(&plain, &blank).len() > 1000,
        "the plain canvas draws the text in view"
    );
    let missing = differing(&surface, &plain).len();
    assert_eq!(missing, 0, "{missing} px of the retained layer differ");
}

/// The retained layer is built a pan before the text comes into view: the
/// oversize text in its margin must have been drawn into it, so the panned
/// layer equals the plain canvas at the final camera. The cull measured the
/// frame from the origin, dropped the margin and left this blank.
#[test]
fn an_oversize_text_in_the_margin_is_in_the_retained_layer_after_a_pan() {
    let mut gpu = gpu();
    let sheet = oversize_text_in_the_margin();
    let run = Run {
        start: iced::Point::ORIGIN,
        pans: vec![(1650.0, 0.0)],
        select: false,
    };
    let (plain, _) = last_frame(&mut gpu, Path::Plain, DOCKED, &sheet, &run);
    let (surface, _) = last_frame(&mut gpu, Path::Surface, DOCKED, &sheet, &run);
    let blank = last_frame(
        &mut gpu,
        Path::Plain,
        DOCKED,
        &SchematicSheet::default(),
        &run,
    )
    .0;
    assert!(
        differing(&plain, &blank).len() > 1000,
        "the plain canvas draws the text"
    );
    let missing = differing(&surface, &plain).len();
    assert_eq!(missing, 0, "{missing} px of the panned layer differ");
}
