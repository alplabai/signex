//! The failing places on the canvas (mockup rows 179d, 190): a ring on each
//! too-sharp inside corner and open profile end, with a callout. Drawn over
//! the bake preview, so the place stays visible while the card is open.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use super::super::super::FootprintCanvasState;
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;
/// The ring's radius and stroke, against the caption size.
const RING_EM: f32 = 1.0;
const STROKE_EM: f32 = 0.15;

pub(in crate::library::editor::footprint::canvas::draw) fn draw_failure_marks(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
    colour: Color,
) {
    let ring = TS.caption * RING_EM;
    let stroke = Stroke::default()
        .with_width(TS.caption * STROKE_EM)
        .with_color(colour);
    for mark in state.failure_marks(sketch) {
        let at = cstate.world_to_screen(mark.at);
        frame.stroke(&Path::circle(at, ring), stroke);
        frame.fill_text(canvas::Text {
            content: mark.label,
            position: Point::new(at.x + ring * 1.25, at.y - ring),
            color: colour,
            size: iced::Pixels(TS.caption),
            align_y: iced::alignment::Vertical::Bottom,
            ..canvas::Text::default()
        });
    }
}
