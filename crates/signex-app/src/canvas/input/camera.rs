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
        if self.resolve_pending_fit(bounds) {
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorMoved,
            )));
        }
        None
    }

    /// Mouse-wheel zoom about the cursor, or a pan: see [`wheel_intent`] for
    /// how "Mouse wheel" and "Two-finger scroll pans" steer it.
    pub(in crate::canvas) fn update_wheel_scrolled(
        &self,
        delta: &mouse::ScrollDelta,
        keys: WheelKeys,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let intent = wheel_intent(
            delta,
            50.0,
            self.prefs.trackpad_pan,
            self.prefs.wheel_mode,
            keys,
        );

        if let Some(cursor_pos) = cursor.position_in(bounds) {
            let changed =
                self.camera_mut()
                    .apply_scroll(cursor_pos, intent, self.prefs.zoom_step, bounds);
            if !changed {
                return None;
            }
            // Grid + content need redraw on zoom or pan
            return Some(
                canvas::Action::publish(Message::CanvasEvent(CanvasEvent::ViewMoved)).and_capture(),
            );
        }
        None
    }

    /// Trackpad pinch zoom about the cursor (ignored outside the canvas).
    pub(in crate::canvas) fn update_pinch_gesture(
        &self,
        delta: f32,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cursor_pos = cursor.position_in(bounds)?;
        let changed = self
            .camera_mut()
            .zoom_by_factor(cursor_pos, Camera::pinch_factor(delta));
        if !changed {
            return None;
        }
        Some(canvas::Action::publish(Message::CanvasEvent(CanvasEvent::ViewMoved)).and_capture())
    }
}
