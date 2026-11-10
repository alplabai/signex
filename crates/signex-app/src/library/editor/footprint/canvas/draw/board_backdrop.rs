//! The board under a board sketch (#2234): its tracks, vias and outline,
//! drawn faint below the sketch so the sketch reads over the board.

use iced::Color;
use iced::widget::canvas::{self, Path, Stroke};

use super::super::{FootprintCanvas, FootprintCanvasState};

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

impl FootprintCanvas<'_> {
    pub(in crate::library::editor::footprint::canvas) fn draw_board_backdrop(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        let Some(board) = self.state.board_backdrop.as_deref() else {
            return;
        };
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
