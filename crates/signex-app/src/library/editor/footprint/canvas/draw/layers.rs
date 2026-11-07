//! Footprint content layers as free functions over a read-only
//! [`LayerInput`]: silk graphics, courtyard and pads. The editor builds
//! the input from its state each draw; the Components-panel preview
//! builds it once from its loaded footprint. Both paint through here.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Point, Size};
use signex_library::primitive::footprint::FpGraphic;

use crate::library::editor::footprint::layers::{FpLayer, LayerVisibility};
use crate::library::editor::footprint::state::{CourtyardRect, EditorPad};

use super::super::FootprintCanvasState;
use super::pad::{PadLabels, draw_pad, draw_pad_number};
use super::silk::draw_silk_graphics;

/// Everything the content layers read. Selection fields are empty/None
/// for a read-only view.
#[derive(Clone, Copy)]
pub(crate) struct LayerInput<'a> {
    pub pads: &'a [EditorPad],
    pub silk_f: &'a [FpGraphic],
    pub silk_b: &'a [FpGraphic],
    pub visibility: LayerVisibility,
    /// Outline-following courtyard; wins over `courtyard_rect`.
    pub courtyard_outline: Option<&'a [(f64, f64)]>,
    pub courtyard_rect: Option<CourtyardRect>,
    pub selected_pad: Option<usize>,
    pub selected_pads_extra: &'a [usize],
    pub selected_silk_f: Option<usize>,
    pub pad_labels: PadLabels,
}

/// Silk-front and silk-back graphics.
pub(crate) fn draw_silk(frame: &mut canvas::Frame, cam: &FootprintCanvasState, i: &LayerInput<'_>) {
    if i.visibility.get(FpLayer::FSilks) {
        draw_silk_graphics(frame, cam, i.silk_f, FpLayer::FSilks, i.selected_silk_f);
    }
    if i.visibility.get(FpLayer::BSilks) {
        draw_silk_graphics(frame, cam, i.silk_b, FpLayer::BSilks, None);
    }
}

/// Front fabrication graphics (previews only; the editor does not show
/// the fab layer).
pub(crate) fn draw_fab(frame: &mut canvas::Frame, cam: &FootprintCanvasState, fab_f: &[FpGraphic]) {
    draw_silk_graphics(frame, cam, fab_f, FpLayer::FFab, None);
}

/// Back silk and back fab, always drawn (previews only): the editor's
/// default visibility keeps the back layers off.
pub(crate) fn draw_back_layers(
    frame: &mut canvas::Frame,
    cam: &FootprintCanvasState,
    silk_b: &[FpGraphic],
    fab_b: &[FpGraphic],
) {
    draw_silk_graphics(frame, cam, fab_b, FpLayer::BFab, None);
    draw_silk_graphics(frame, cam, silk_b, FpLayer::BSilks, None);
}

/// Courtyard: the outline polygon when present, else the bbox rectangle.
pub(crate) fn draw_courtyard(
    frame: &mut canvas::Frame,
    cam: &FootprintCanvasState,
    i: &LayerInput<'_>,
) {
    if !i.visibility.get(FpLayer::EdgeCuts) {
        return;
    }
    let edge_color = FpLayer::EdgeCuts.color();
    let stroke = || Stroke::default().with_width(1.5).with_color(edge_color);
    if let Some(outline) = i.courtyard_outline {
        if outline.len() >= 3 {
            let path = Path::new(|b| {
                let first = cam.world_to_screen(outline[0]);
                b.move_to(first);
                for v in outline.iter().skip(1) {
                    b.line_to(cam.world_to_screen(*v));
                }
                b.line_to(first);
            });
            frame.stroke(&path, stroke());
        }
    } else if let Some(c) = i.courtyard_rect {
        let p0 = cam.world_to_screen((c.min_x, c.min_y));
        let p1 = cam.world_to_screen((c.max_x, c.max_y));
        let rect = Path::rectangle(Point::new(p0.x, p0.y), Size::new(p1.x - p0.x, p1.y - p0.y));
        frame.stroke(&rect, stroke());
    }
}

/// Pads, last of the content layers so they sit on top.
pub(crate) fn draw_pads(frame: &mut canvas::Frame, cam: &FootprintCanvasState, i: &LayerInput<'_>) {
    for (idx, pad) in i.pads.iter().enumerate() {
        if !i.visibility.get(pad.primary_layer()) {
            continue;
        }
        let is_selected = i.selected_pad == Some(idx) || i.selected_pads_extra.contains(&idx);
        draw_pad(frame, cam, pad, is_selected);
    }
    // Numbers last so no pad body covers a neighbour's number.
    for pad in i.pads {
        if i.visibility.get(pad.primary_layer()) {
            draw_pad_number(frame, cam, pad, i.pad_labels);
        }
    }
}
