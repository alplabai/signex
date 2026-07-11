use super::super::*;

impl SchematicCanvas {
    /// Consume a pending fit-to-content target and apply it to the camera.
    pub(in crate::canvas) fn update_pending_fit(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
    ) -> Option<canvas::Action<Message>> {
        // Transfer pending fit from SchematicCanvas to CanvasState (consumes it)
        if let Some(target) = self.pending_fit.take() {
            state.pending_fit = Some(target);
        }

        // Apply pending fit-to-content
        if let Some(target) = state.pending_fit.take() {
            state.camera.fit_rect(target, bounds);
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorMoved,
            )));
        }
        None
    }

    /// Mouse-wheel zoom about the cursor.
    pub(in crate::canvas) fn update_wheel_scrolled(
        &self,
        state: &mut CanvasState,
        delta: &mouse::ScrollDelta,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let scroll_y = match delta {
            mouse::ScrollDelta::Lines { y, .. } => *y,
            mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
        };

        if let Some(cursor_pos) = cursor.position_in(bounds) {
            let changed = state.camera.zoom_at(cursor_pos, scroll_y, bounds);
            if !changed {
                return None;
            }
            // Grid + content need redraw on zoom
            return Some(
                canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                    .and_capture(),
            );
        }
        None
    }
}
