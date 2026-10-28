//! Measure tool (#1256): click A, click B, read dx / dy / distance. Read-only:
//! nothing here touches the document or the undo history.

use signex_types::measure::Measurement;
use signex_types::schematic::Point;

use super::super::super::*;
use crate::canvas::{MeasureOverlay, ShapePreviewKind};

impl Signex {
    /// One snapped canvas click while `Tool::Measure` is active. The first click
    /// anchors A (dropping the previous result), the second places B, logs the
    /// result and keeps it on the canvas until Esc or the next click.
    pub(super) fn measure_click(&mut self, world_x: f64, world_y: f64) {
        let point = Point::new(world_x, world_y);
        let Some(a) = self.interaction_state.shape_anchor.take() else {
            self.interaction_state.shape_anchor = Some(point);
            let canvas = self.interaction_state.active_canvas_mut();
            canvas.measure = None;
            canvas.shape_anchor = Some((point, ShapePreviewKind::Measure));
            canvas.clear_overlay_cache();
            return;
        };
        let measurement = Measurement::between(a, point);
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.shape_anchor = None;
        canvas.measure = Some(MeasureOverlay { a, b: point });
        canvas.clear_overlay_cache();
        crate::diagnostics::log_info(format!(
            "Measure: {}",
            measurement.describe(self.ui_state.unit)
        ));
    }

    /// The status-bar segment for the Measure tool: the prompt for the next
    /// click, or the last result in the current unit.
    pub(crate) fn measure_status(&self, window_id: iced::window::Id) -> Option<String> {
        if self.interaction_state.current_tool != Tool::Measure {
            return None;
        }
        let canvas = self.interaction_state.canvas_for_window(window_id);
        Some(match (&canvas.measure, canvas.shape_anchor) {
            (_, Some(_)) => "Click the second point".to_owned(),
            (Some(done), None) => Measurement::between(done.a, done.b).describe(self.ui_state.unit),
            (None, None) => "Click the first point".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::coord::Unit;

    fn app_measuring(unit: Unit) -> (Signex, iced::window::Id) {
        let (mut app, _task) = Signex::new();
        app.ui_state.unit = unit;
        app.interaction_state.current_tool = Tool::Measure;
        (app, iced::window::Id::unique())
    }

    #[test]
    fn the_status_segment_prompts_then_reports_in_the_active_unit() {
        let (mut app, window) = app_measuring(Unit::Mil);
        assert_eq!(
            app.measure_status(window).as_deref(),
            Some("Click the first point")
        );

        app.measure_click(0.0, 0.0);
        assert_eq!(
            app.measure_status(window).as_deref(),
            Some("Click the second point")
        );

        app.measure_click(2.0, 0.0);
        assert_eq!(
            app.measure_status(window).as_deref(),
            Some("dx 78.74 mil  dy 0.00 mil  dist 78.74 mil")
        );
        app.ui_state.unit = Unit::Mm;
        assert_eq!(
            app.measure_status(window).as_deref(),
            Some("dx 2.000 mm  dy 0.000 mm  dist 2.000 mm")
        );
    }

    #[test]
    fn no_status_segment_under_another_tool() {
        let (mut app, window) = app_measuring(Unit::Mm);
        app.interaction_state.current_tool = Tool::Wire;
        assert!(app.measure_status(window).is_none());
    }
}
