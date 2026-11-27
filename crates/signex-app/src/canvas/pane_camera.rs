//! A pane's view put back on a sheet canvas (mockup row 207): the pan and
//! zoom a pane kept while another had the focus.

use super::{Camera, CanvasSlot};

impl CanvasSlot {
    /// Show the sheet at `camera` (screen offset of the world origin and
    /// pixels per mm, as [`Self::live_camera`] reads them). A queued fit is
    /// dropped: the view is the pane's. A value that is not a number is
    /// ignored.
    pub fn set_pane_camera(&mut self, camera: (f32, f32, f32)) {
        let (x, y, scale) = camera;
        if !(x.is_finite() && y.is_finite() && scale.is_finite() && scale > 0.0) {
            return;
        }
        {
            let mut live = self.camera.borrow_mut();
            live.offset = iced::Point::new(x, y);
            live.scale = scale.clamp(Camera::MIN_SCALE, Camera::MAX_SCALE);
        }
        self.cancel_pending_fit();
        self.fitted.set(None);
        self.clear_bg_cache();
        self.clear_overlay_cache();
        self.clear_content_cache();
    }
}
