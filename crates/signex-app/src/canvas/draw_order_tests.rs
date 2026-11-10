//! Draw order of text against shapes inside one canvas `Frame`.
//!
//! The fork's canvas frames draw shapes, images and text in call order (iced
//! fork issue #18); they used to put all text on top. Painters that emit a
//! label next to a shape must therefore emit the label last. Two cases:
//!
//! * the sheet background, read off a bare wgpu frame (no adapter): the
//!   paper fill and every frame line sit before the title-block text;
//! * footprint pad numbers, rendered to pixels through the GPU harness of
//!   `gpu_mount_tests`: a pad drawn later must not cover an earlier pad's
//!   number.

use iced::advanced::graphics::color::pack;
use iced::widget::{Widget as _, canvas};
use iced::{Point, Rectangle, Renderer, Size, Theme, mouse};
use iced_wgpu::Geometry;
use signex_output::template::{FontStyle, FramePrimitive, FrameTextAnchor};

use super::gpu_mount_tests::{gpu, render};
use super::grid_order_tests::MeshOnly;
use super::tests::test_prefs;
use super::*;
use crate::library::editor::footprint::canvas::{
    FootprintCanvasState, LayerInput, PadLabels, draw_pads,
};
use crate::library::editor::footprint::layers::{LayerPalette, LayerVisibility};
use crate::library::editor::footprint::state::EditorPad;

/// The title-block text is drawn after the paper fill and after every line of
/// the frame, even when the primitive list puts the text first.
#[test]
fn sheet_frame_text_follows_the_paper_and_every_frame_line() {
    let (overrides, filters) = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    let paper = pack(prefs.theme_paper).components();

    let mut slot = CanvasSlot::new();
    *slot.camera_mut() = Camera {
        offset: Point::new(20.0, 20.0),
        scale: 8.0,
    };
    slot.sheet_frame = vec![
        // Text first on purpose: the old order drew it before the lines.
        FramePrimitive::Text {
            x: 20.0,
            y: 20.0,
            size_mm: 4.0,
            style: FontStyle::Normal,
            anchor: FrameTextAnchor::TopLeft,
            text: "A".to_string(),
        },
        FramePrimitive::Line {
            from: (5.0, 5.0),
            to: (100.0, 5.0),
            width_mm: 0.5,
        },
        FramePrimitive::Rect {
            x: 10.0,
            y: 10.0,
            w: 100.0,
            h: 80.0,
            width_mm: 0.5,
        },
    ];
    let canvas = SchematicCanvas::new(&slot, prefs);
    let bounds = Rectangle::with_size(Size::new(800.0, 600.0));
    let mut frame = canvas::Frame::<MeshOnly>::new(&MeshOnly, bounds.size());
    canvas.paint_background(&mut frame, bounds);
    let Geometry::Live { sections } = frame.into_geometry() else {
        panic!("a fresh frame is live geometry");
    };

    let text_section = sections
        .iter()
        .position(|s| !s.text.is_empty())
        .expect("the title text is in the frame");
    assert_eq!(
        sections.iter().map(|s| s.text.len()).sum::<usize>(),
        1,
        "one string was emitted"
    );
    // Meshes inside a section draw before its text; later sections draw after.
    assert!(
        sections[text_section + 1..]
            .iter()
            .all(|s| s.meshes.is_empty()),
        "a shape is drawn after the sheet text"
    );
    let paper_section = sections
        .iter()
        .rposition(|s| {
            s.meshes.iter().any(|m| match m {
                iced::advanced::graphics::Mesh::Solid { buffers, .. } => buffers
                    .vertices
                    .iter()
                    .any(|v| v.color.components() == paper),
                iced::advanced::graphics::Mesh::Gradient { .. } => false,
            })
        })
        .expect("the paper fill is in the frame");
    assert!(
        paper_section <= text_section,
        "the paper fill (section {paper_section}) must not follow the text (section {text_section})"
    );
}

/// Two overlapping pads; the first pad's number must stay on top of the
/// second pad's copper.
struct TwoPads;

impl canvas::Program<crate::app::Message> for TwoPads {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let pads = [
            EditorPad::new_default("1".to_string(), (0.0, 0.0)),
            EditorPad::new_default("2".to_string(), (0.5, 0.0)),
        ];
        let pads: Vec<EditorPad> = pads
            .into_iter()
            .map(|mut p| {
                p.size_mm = (2.0, 2.0);
                p
            })
            .collect();
        let state = FootprintCanvasState::with_view(40.0, Point::new(60.0, 100.0));
        let input = LayerInput {
            pads: &pads,
            silk_f: &[],
            silk_b: &[],
            visibility: LayerVisibility::default(),
            courtyard_outline: None,
            courtyard_rect: None,
            selected_pad: None,
            selected_pads_extra: &[],
            selected_silk_f: None,
            pad_labels: PadLabels::Editor,
            palette: LayerPalette::Legacy,
        };
        draw_pads(&mut frame, &state, &input);
        vec![frame.into_geometry()]
    }
}

#[test]
fn a_later_pad_does_not_cover_an_earlier_pads_number() {
    let mut gpu = gpu();
    let element = canvas(TwoPads)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .boxed();
    let frame = render(&mut gpu, element);
    // Pad "1" is centred on (60, 100) and is inside pad "2" (x in 40..120).
    // Its number is drawn in a colour far from copper. If pad 2 covered it,
    // only ~15% of that contrast would show through the 0.85-alpha copper.
    //
    // Measured relative to the window's own copper colour (the per-channel
    // median, as the number covers a minority of the window) rather than as
    // "near-black" pixels: how dark the glyph strokes get depends on the
    // rasteriser's anti-aliasing and font, which differ between software
    // Vulkan on Linux and Metal on macOS.
    let mut region: Vec<[u8; 3]> = Vec::new();
    for y in 92..108 {
        for x in 52..68 {
            let p = frame.px(x, y);
            region.push([p[0], p[1], p[2]]);
        }
    }
    let median = |channel: usize| {
        let mut values: Vec<u8> = region.iter().map(|p| p[channel]).collect();
        values.sort_unstable();
        values[values.len() / 2]
    };
    let copper = [median(0), median(1), median(2)];
    let contrasting = region
        .iter()
        .filter(|p| {
            (0..3)
                .map(|c| u32::from(p[c].abs_diff(copper[c])))
                .sum::<u32>()
                > 90
        })
        .count();
    assert!(
        contrasting > 6,
        "pad 1's number is hidden under pad 2 ({contrasting} of {} px differ from the copper {copper:?})",
        region.len()
    );
}
