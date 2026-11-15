//! The board camera follows a resized canvas (#2345), by the schematic rule
//! in [`crate::canvas::resize_follow`].

use iced::{Rectangle, Size};

use super::PcbCanvas;
use crate::canvas::{FittedView, ResizeFollow, resize_follow};

impl PcbCanvas {
    /// Apply a queued fit against the canvas's real `viewport`; whether the
    /// camera moved.
    pub fn resolve_pending_fit(&self, viewport: Rectangle) -> bool {
        let Some(target) = self.pending_fit.take() else {
            return false;
        };
        self.apply_fit(target, viewport);
        true
    }

    /// Fit board rectangle `target` into `viewport` and remember it, so a
    /// resize can refit.
    fn apply_fit(&self, target: Rectangle, viewport: Rectangle) {
        let mut camera = self.camera.borrow_mut();
        camera.fit_rect(target, viewport);
        self.fitted.set(Some(FittedView::new(
            target,
            camera.offset(),
            camera.scale(),
        )));
    }

    /// Size of the canvas as last laid out (zero before the first layout).
    pub fn viewport_size(&self) -> Size {
        self.viewport.get().size()
    }

    /// The canvas is about to be laid out at `after`: refit a view still at
    /// its last fit, else keep the centre, the zoom and the turn. See
    /// [`crate::canvas::CanvasSlot::follow_resize`]. Whether the camera moved.
    pub fn follow_resize(&self, after: Size) -> bool {
        if self.pending_fit.get().is_some() {
            return false;
        }
        let before = self.viewport.get();
        let follow = {
            let camera = self.camera.borrow();
            resize_follow(
                before.size(),
                after,
                self.fitted.get(),
                camera.offset(),
                camera.scale(),
            )
        };
        let viewport = Rectangle::new(before.position(), after);
        match follow {
            ResizeFollow::Stay => return false,
            ResizeFollow::Refit(target) => self.apply_fit(target, viewport),
            // A screen-space shift: the turn does not change what it keeps.
            ResizeFollow::Shift { dx, dy } => self.camera.borrow_mut().pan(dx, dy),
        }
        self.viewport.set(viewport);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: Rectangle = Rectangle {
        x: 10.0,
        y: 5.0,
        width: 80.0,
        height: 50.0,
    };

    fn view(width: f32, height: f32) -> Rectangle {
        Rectangle::new(iced::Point::ORIGIN, Size::new(width, height))
    }

    fn fitted_at(viewport: Rectangle) -> PcbCanvas {
        let canvas = PcbCanvas::new();
        canvas.set_viewport(viewport);
        canvas.pending_fit.set(Some(BOARD));
        assert!(canvas.resolve_pending_fit(viewport));
        canvas
    }

    #[test]
    fn a_fitted_board_is_fitted_again_at_the_new_size() {
        let (small, large) = (view(800.0, 600.0), view(1400.0, 1000.0));
        let canvas = fitted_at(small);
        assert!(canvas.follow_resize(large.size()));
        assert_eq!(canvas.live_camera(), fitted_at(large).live_camera());
    }

    #[test]
    fn a_zoomed_board_keeps_its_centre_and_zoom() {
        let (small, large) = (view(800.0, 600.0), view(1400.0, 1000.0));
        let canvas = fitted_at(small);
        assert!(canvas.zoom_about_centre(true, 1.25));
        let centre = |v: Rectangle| iced::Point::new(v.width / 2.0, v.height / 2.0);
        let before = canvas.camera.borrow().screen_to_world(centre(small));
        let scale = canvas.live_camera().2;

        assert!(canvas.follow_resize(large.size()));

        let after = canvas.camera.borrow().screen_to_world(centre(large));
        assert!((after.x - before.x).abs() < 1e-3 && (after.y - before.y).abs() < 1e-3);
        assert_eq!(canvas.live_camera().2, scale, "the zoom is kept");
    }

    #[test]
    fn a_canvas_never_laid_out_is_left_alone() {
        let canvas = PcbCanvas::new();
        let before = canvas.live_camera();
        assert!(!canvas.follow_resize(Size::new(1400.0, 1000.0)));
        assert_eq!(canvas.live_camera(), before);
    }
}
