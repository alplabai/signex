use super::super::*;

impl SchematicCanvas<'_> {
    /// Consume a pending fit-to-content target and apply it to the camera.
    pub(in crate::canvas) fn update_pending_fit(
        &self,
        bounds: Rectangle,
    ) -> Option<canvas::Action<Message>> {
        // #632 — one hop, not two. This used to move the target from
        // `self.pending_fit` into `state.pending_fit` and apply it from
        // there on the same call; with the camera owned here, the
        // `Program::State` copy had no reader left and was removed.
        if let Some(target) = self.pending_fit.take() {
            self.camera_mut().fit_rect(target, bounds);
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorMoved,
            )));
        }
        None
    }

    /// Mouse-wheel zoom about the cursor.
    pub(in crate::canvas) fn update_wheel_scrolled(
        &self,
        delta: &mouse::ScrollDelta,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let scroll_y = match delta {
            mouse::ScrollDelta::Lines { y, .. } => *y,
            mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
        };

        if let Some(cursor_pos) = cursor.position_in(bounds) {
            let changed = self.camera_mut().zoom_at(cursor_pos, scroll_y, bounds);
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
