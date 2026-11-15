//! What a canvas camera does when its canvas changes size (#2345): a view
//! that still shows its last fit is fitted again; any other view keeps the
//! world point at its centre in the centre, and keeps its zoom.

use iced::{Point, Rectangle, Size};

use super::Camera;

/// The last fit a canvas applied: the world rectangle it framed and the
/// camera it left. The view counts as fitted while the camera is unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FittedView {
    target: Rectangle,
    offset: Point,
    scale: f32,
}

impl FittedView {
    pub fn new(target: Rectangle, offset: Point, scale: f32) -> Self {
        Self {
            target,
            offset,
            scale,
        }
    }

    /// No pan or zoom since the fit.
    pub fn holds(&self, offset: Point, scale: f32) -> bool {
        self.offset == offset && self.scale == scale
    }
}

/// How the camera follows a canvas that changed size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeFollow {
    /// Leave the camera as it is.
    Stay,
    /// Fit this world rectangle into the new size.
    Refit(Rectangle),
    /// Move the screen offset by this many pixels, so the world point at the
    /// old centre lands on the new centre.
    Shift { dx: f32, dy: f32 },
}

/// Decide how a camera at `offset` / `scale` follows a canvas resized from
/// `before` to `after`. An empty size on either side (not laid out yet,
/// minimised) leaves the camera alone, so a restore cannot drift it.
pub fn resize_follow(
    before: Size,
    after: Size,
    fitted: Option<FittedView>,
    offset: Point,
    scale: f32,
) -> ResizeFollow {
    if before == after || !has_area(before) || !has_area(after) {
        return ResizeFollow::Stay;
    }
    match fitted {
        Some(fit) if fit.holds(offset, scale) => {
            if can_fit(after) {
                ResizeFollow::Refit(fit.target)
            } else {
                // Too small to fit into: keep the fit owed for a later size.
                ResizeFollow::Stay
            }
        }
        _ => ResizeFollow::Shift {
            dx: (after.width - before.width) / 2.0,
            dy: (after.height - before.height) / 2.0,
        },
    }
}

fn has_area(size: Size) -> bool {
    size.width > 0.0 && size.height > 0.0
}

/// Whether `size` leaves room inside the fit padding, as `fit_rect` needs.
pub fn can_fit(size: Size) -> bool {
    size.width > Camera::FIT_PADDING * 2.0 && size.height > Camera::FIT_PADDING * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: Size = Size::new(800.0, 600.0);
    const LARGE: Size = Size::new(1200.0, 900.0);
    const TARGET: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 50.0,
    };

    fn fitted() -> FittedView {
        FittedView::new(TARGET, Point::new(10.0, 20.0), 3.0)
    }

    #[test]
    fn a_view_still_at_its_fit_is_fitted_again() {
        let follow = resize_follow(SMALL, LARGE, Some(fitted()), Point::new(10.0, 20.0), 3.0);
        assert_eq!(follow, ResizeFollow::Refit(TARGET));
    }

    #[test]
    fn a_view_moved_since_its_fit_keeps_its_centre() {
        let follow = resize_follow(SMALL, LARGE, Some(fitted()), Point::new(11.0, 20.0), 3.0);
        assert_eq!(
            follow,
            ResizeFollow::Shift {
                dx: 200.0,
                dy: 150.0
            }
        );
        let zoomed = resize_follow(SMALL, LARGE, Some(fitted()), Point::new(10.0, 20.0), 4.0);
        assert_eq!(
            zoomed,
            ResizeFollow::Shift {
                dx: 200.0,
                dy: 150.0
            }
        );
    }

    #[test]
    fn a_view_never_fitted_keeps_its_centre() {
        let follow = resize_follow(LARGE, SMALL, None, Point::ORIGIN, 3.0);
        assert_eq!(
            follow,
            ResizeFollow::Shift {
                dx: -200.0,
                dy: -150.0
            }
        );
    }

    #[test]
    fn an_empty_size_or_no_change_leaves_the_camera() {
        for (before, after) in [(SMALL, SMALL), (Size::ZERO, LARGE), (LARGE, Size::ZERO)] {
            assert_eq!(
                resize_follow(before, after, Some(fitted()), Point::new(10.0, 20.0), 3.0),
                ResizeFollow::Stay,
                "{before:?} -> {after:?}"
            );
        }
    }

    #[test]
    fn a_fitted_view_too_small_to_fit_stays_fitted() {
        let tiny = Size::new(Camera::FIT_PADDING, Camera::FIT_PADDING);
        let follow = resize_follow(SMALL, tiny, Some(fitted()), Point::new(10.0, 20.0), 3.0);
        assert_eq!(follow, ResizeFollow::Stay);
    }
}
