//! The schematic camera follows a resized canvas (#2345); the rule is in
//! [`super::fitted_view`].

use iced::{Rectangle, Size};

use super::{CanvasSlot, ResizeFollow, resize_follow};

impl CanvasSlot {
    /// Size of the canvas as last laid out (zero before the first layout).
    pub fn viewport_size(&self) -> Size {
        self.viewport.get().size()
    }

    /// The canvas is about to be laid out at `after`: refit a view still at
    /// its last fit, else keep the centre and the zoom. Runs from `update`,
    /// before the first canvas event at the new size, so the recorded
    /// viewport is still the old one. A canvas never laid out, or with a fit
    /// still owed, is left to that fit. Whether the camera moved.
    pub fn follow_resize(&self, after: Size) -> bool {
        if self.pending_fit.get().is_some() || self.pending_sheet_fit.get() {
            return false;
        }
        let before = self.viewport.get();
        let follow = {
            let camera = self.camera();
            resize_follow(
                before.size(),
                after,
                self.fitted.get(),
                camera.offset,
                camera.scale,
            )
        };
        let viewport = Rectangle::new(before.position(), after);
        match follow {
            ResizeFollow::Stay => return false,
            ResizeFollow::Refit(target) => self.apply_fit(target, viewport),
            ResizeFollow::Shift { dx, dy } => self.camera_mut().pan(dx, dy),
        }
        // Menu zoom reads the centre before the next canvas event lands.
        self.viewport.set(viewport);
        true
    }
}
