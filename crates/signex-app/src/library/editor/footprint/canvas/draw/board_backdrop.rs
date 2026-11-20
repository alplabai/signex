//! The board under a board sketch (#2234): its courtyards, pads, tracks,
//! vias and outline, drawn faint below the sketch so the sketch reads over
//! the board. Pads and courtyards (#2518, Graphite mockup #179) take the
//! board palette's colours, the same in both themes.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use super::super::{FootprintCanvas, FootprintCanvasState};
use crate::library::editor::footprint::state::{BackdropPad, PadCopper};
use crate::styles::board_palette::{BoardPalette, BoardSwatch};

/// The board edge, as the PCB canvas draws it.
const EDGE: Color = Color {
    r: 0.93,
    g: 0.80,
    b: 0.27,
    a: 0.85,
};
/// Copper under the sketch: present, but quieter than the sketch.
const COPPER: Color = Color {
    r: 0.80,
    g: 0.36,
    b: 0.24,
    a: 0.45,
};
/// How much of a pad's palette colour shows under the sketch.
const PAD_ALPHA: f32 = 0.45;
/// How much of the courtyard colour shows: the mockup's faint backdrop.
const COURTYARD_ALPHA: f32 = 0.55;

impl FootprintCanvas<'_> {
    pub(in crate::library::editor::footprint::canvas) fn draw_board_backdrop(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        let Some(board) = self.state.board_backdrop.as_deref() else {
            return;
        };
        let palette = BoardPalette::new();
        let courtyard = faint(palette.swatch(BoardSwatch::Courtyard), COURTYARD_ALPHA);
        for outline in &board.courtyards {
            let Some((&first, rest)) = outline.points.split_first() else {
                continue;
            };
            let path = Path::new(|b| {
                b.move_to(cstate.world_to_screen(first));
                for &point in rest {
                    b.line_to(cstate.world_to_screen(point));
                }
                if outline.closed {
                    b.close();
                }
            });
            frame.stroke(
                &path,
                Stroke::default().with_color(courtyard).with_width(1.2),
            );
        }
        for pad in &board.pads {
            let ink = match pad.copper {
                PadCopper::Top => palette.swatch(BoardSwatch::TopCopper),
                PadCopper::Bottom => palette.swatch(BoardSwatch::BottomCopper),
                PadCopper::Through => palette.multi_layer,
            };
            frame.fill(&pad_path(pad, cstate), faint(ink, PAD_ALPHA));
        }
        for track in &board.tracks {
            let path = Path::line(
                cstate.world_to_screen(track.from),
                cstate.world_to_screen(track.to),
            );
            let width = (track.width as f32 * cstate.scale).max(1.0);
            frame.stroke(
                &path,
                Stroke::default().with_color(COPPER).with_width(width),
            );
        }
        for &(centre, diameter) in &board.vias {
            let radius = (diameter as f32 * 0.5 * cstate.scale).max(1.0);
            frame.fill(
                &Path::circle(cstate.world_to_screen(centre), radius),
                COPPER,
            );
        }
        if let Some((&first, rest)) = board.outline.split_first() {
            let path = Path::new(|b| {
                b.move_to(cstate.world_to_screen(first));
                for &point in rest {
                    b.line_to(cstate.world_to_screen(point));
                }
                b.close();
            });
            frame.stroke(&path, Stroke::default().with_color(EDGE).with_width(1.5));
        }
    }
}

fn faint(colour: Color, alpha: f32) -> Color {
    Color {
        a: colour.a * alpha,
        ..colour
    }
}

/// A pad's outline on screen: a disc for a round pad, else its rectangle
/// turned by the pad's rotation.
fn pad_path(pad: &BackdropPad, cstate: &FootprintCanvasState) -> Path {
    let centre = cstate.world_to_screen(pad.centre);
    if pad.round {
        let radius = (pad.size.0.max(pad.size.1) as f32 * 0.5 * cstate.scale).max(1.0);
        return Path::circle(centre, radius);
    }
    let (sin, cos) = pad.rotation_deg.to_radians().sin_cos();
    let (hw, hh) = (pad.size.0 * 0.5, pad.size.1 * 0.5);
    let corners: Vec<Point> = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
        .into_iter()
        .map(|(x, y)| {
            cstate.world_to_screen((
                pad.centre.0 + x * cos - y * sin,
                pad.centre.1 + x * sin + y * cos,
            ))
        })
        .collect();
    Path::new(|b| {
        b.move_to(corners[0]);
        for &corner in &corners[1..] {
            b.line_to(corner);
        }
        b.close();
    })
}
