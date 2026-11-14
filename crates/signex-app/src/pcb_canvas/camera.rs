//! The board view's camera (#2316): pan, zoom and the turn of the view on
//! screen, in one transform that every board path reads (CPU draw, GPU
//! shader, culling, hit-test, the highlight layer, fit).
//!
//! `screen = rotation(board) * scale + offset`. Pan and zoom act on the
//! screen offset and the scale, so the schematic [`Camera`] does them
//! unchanged; this type adds the turn and owns every mapping that a turn
//! changes. It does not deref to [`Camera`], so no caller can reach the
//! unturned mapping by mistake.

use iced::{Point, Rectangle};
use signex_gfx::view_rotation::ViewRotation;

use crate::canvas::{Camera, ScrollIntent};

#[derive(Debug, Clone, Default)]
pub struct BoardCamera {
    camera: Camera,
    rotation: ViewRotation,
}

impl From<Camera> for BoardCamera {
    fn from(camera: Camera) -> Self {
        Self {
            camera,
            rotation: ViewRotation::IDENTITY,
        }
    }
}

impl BoardCamera {
    /// Screen offset of the board origin, in pixels.
    pub fn offset(&self) -> Point {
        self.camera.offset
    }

    /// Pixels per mm.
    pub fn scale(&self) -> f32 {
        self.camera.scale
    }

    pub fn rotation(&self) -> ViewRotation {
        self.rotation
    }

    pub fn zoom_percent(&self) -> f64 {
        self.camera.zoom_percent()
    }

    fn offset_px(&self) -> [f32; 2] {
        [self.camera.offset.x, self.camera.offset.y]
    }

    /// Board mm to canvas pixels.
    pub fn world_to_screen(&self, world: Point) -> Point {
        let [x, y] =
            self.rotation
                .world_to_screen([world.x, world.y], self.offset_px(), self.camera.scale);
        Point::new(x, y)
    }

    /// Canvas pixels to board mm.
    pub fn screen_to_world(&self, screen: Point) -> Point {
        let [x, y] = self.rotation.screen_to_world(
            [screen.x, screen.y],
            self.offset_px(),
            self.camera.scale,
        );
        Point::new(x, y)
    }

    /// The board box `(min, max)` around what a `size`-pixel canvas shows,
    /// or `None` while the camera is degenerate.
    pub fn visible_world_bounds(&self, size: iced::Size) -> Option<([f32; 2], [f32; 2])> {
        self.rotation.visible_world_box(
            [size.width, size.height],
            self.offset_px(),
            self.camera.scale,
        )
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.camera.pan(dx, dy);
    }

    pub fn zoom_at(&mut self, at: Point, notches: f32, step: f32, bounds: Rectangle) -> bool {
        self.camera.zoom_at(at, notches, step, bounds)
    }

    pub fn zoom_by_factor(&mut self, at: Point, factor: f32) -> bool {
        self.camera.zoom_by_factor(at, factor)
    }

    pub fn apply_scroll(
        &mut self,
        at: Point,
        intent: ScrollIntent,
        step: f32,
        bounds: Rectangle,
    ) -> bool {
        self.camera.apply_scroll(at, intent, step, bounds)
    }

    /// Pan so board point `world` sits at the viewport centre.
    pub fn center_on(&mut self, world: Point, viewport: Rectangle) {
        let [x, y] = self.rotation.apply([world.x, world.y]);
        let s = self.camera.scale;
        self.camera.offset =
            Point::new(viewport.width / 2.0 - x * s, viewport.height / 2.0 - y * s);
    }

    /// Fit board rectangle `world` into the viewport, as turned on screen.
    pub fn fit_rect(&mut self, world: Rectangle, viewport: Rectangle) {
        if world.width <= 0.0 || world.height <= 0.0 {
            return;
        }
        let [w, h] = self.rotation.bounding_size([world.width, world.height]);
        let padding = Camera::FIT_PADDING;
        let scale_x = (viewport.width - padding * 2.0) / w;
        let scale_y = (viewport.height - padding * 2.0) / h;
        self.camera.scale = scale_x
            .min(scale_y)
            .clamp(Camera::MIN_SCALE, Camera::MAX_SCALE);
        self.center_on(world.center(), viewport);
    }

    /// Turn the view to `rotation`, keeping the board point at the centre of
    /// `viewport` where it is. Whether the angle changed.
    pub fn set_rotation(&mut self, rotation: ViewRotation, viewport: Rectangle) -> bool {
        if rotation == self.rotation {
            return false;
        }
        let centre = Point::new(viewport.width / 2.0, viewport.height / 2.0);
        let pivot = self.screen_to_world(centre);
        self.rotation = rotation;
        if pivot.x.is_finite() && pivot.y.is_finite() {
            self.center_on(pivot, viewport);
        }
        true
    }
}

/// The view's turn as the canvas exposes it (#2316).
impl super::PcbCanvas {
    /// The view's turn on screen, clockwise degrees in `[0, 360)`.
    pub fn view_rotation_deg(&self) -> f32 {
        self.camera.borrow().rotation().degrees()
    }

    /// The view's turn, for the GPU shader.
    pub fn view_rotation(&self) -> ViewRotation {
        self.camera.borrow().rotation()
    }

    /// Turn the view to `degrees` about the centre of the canvas; the board
    /// is not touched. Whether the angle changed.
    pub fn set_view_rotation(&mut self, degrees: f32) -> bool {
        let viewport = self.viewport.get();
        let turned = self
            .camera
            .get_mut()
            .set_rotation(ViewRotation::from_degrees(degrees), viewport);
        if turned {
            self.bg_cache.clear();
            *self.content.get_mut() = None;
        }
        turned
    }

    /// Board mm to canvas pixels under the live camera.
    pub fn board_to_screen(&self, board: signex_types::pcb::Point) -> Point {
        let world = Point::new(board.x as f32, board.y as f32);
        self.camera.borrow().world_to_screen(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    fn camera_at(deg: f32) -> BoardCamera {
        let mut camera = BoardCamera::from(Camera {
            offset: Point::new(120.0, -35.0),
            scale: 6.0,
        });
        camera.set_rotation(ViewRotation::from_degrees(deg), VIEW);
        camera
    }

    #[test]
    fn screen_and_board_round_trip_at_0_37_90_and_180() {
        for deg in [0.0, 37.0, 90.0, 180.0] {
            let camera = camera_at(deg);
            for board in [Point::new(0.0, 0.0), Point::new(42.5, -13.25)] {
                let back = camera.screen_to_world(camera.world_to_screen(board));
                assert!(
                    (back.x - board.x).abs() < 1e-3 && (back.y - board.y).abs() < 1e-3,
                    "{deg}: {board:?} came back as {back:?}"
                );
            }
            let screen = Point::new(311.0, 207.0);
            let back = camera.world_to_screen(camera.screen_to_world(screen));
            assert!((back.x - screen.x).abs() < 1e-2 && (back.y - screen.y).abs() < 1e-2);
        }
    }

    #[test]
    fn turning_keeps_the_board_point_at_the_centre() {
        let mut camera = camera_at(0.0);
        let centre = Point::new(400.0, 300.0);
        let before = camera.screen_to_world(centre);
        assert!(camera.set_rotation(ViewRotation::from_degrees(37.0), VIEW));
        let after = camera.screen_to_world(centre);
        assert!((after.x - before.x).abs() < 1e-3 && (after.y - before.y).abs() < 1e-3);
        assert!(!camera.set_rotation(ViewRotation::from_degrees(397.0), VIEW));
    }

    /// A quarter turn shows board +x going down the screen.
    #[test]
    fn a_quarter_turn_is_clockwise_on_screen() {
        let camera = camera_at(90.0);
        let a = camera.world_to_screen(Point::new(0.0, 0.0));
        let b = camera.world_to_screen(Point::new(10.0, 0.0));
        assert!((b.x - a.x).abs() < 1e-3 && (b.y - a.y - 60.0).abs() < 1e-3);
    }

    /// Fit at 90 degrees frames the whole board: a wide board fits by its
    /// width along the screen's height.
    #[test]
    fn fit_at_90_frames_the_whole_board() {
        let mut camera = camera_at(90.0);
        let board = Rectangle::new(Point::new(-5.0, 10.0), iced::Size::new(200.0, 50.0));
        camera.fit_rect(board, VIEW);
        let corners = [
            Point::new(board.x, board.y),
            Point::new(board.x + board.width, board.y),
            Point::new(board.x, board.y + board.height),
            Point::new(board.x + board.width, board.y + board.height),
        ];
        for c in corners {
            let s = camera.world_to_screen(c);
            assert!(
                s.x >= -1e-3
                    && s.x <= VIEW.width + 1e-3
                    && s.y >= -1e-3
                    && s.y <= VIEW.height + 1e-3,
                "{c:?} is off screen at {s:?}"
            );
        }
        // Height-limited: 200 mm across 600 - 80 px.
        assert!((camera.scale() - 520.0 / 200.0).abs() < 1e-4);
    }

    #[test]
    fn the_visible_bounds_hold_every_screen_corner() {
        let camera = camera_at(37.0);
        let (min, max) = camera
            .visible_world_bounds(VIEW.size())
            .expect("a live camera");
        for p in [
            Point::ORIGIN,
            Point::new(800.0, 600.0),
            Point::new(800.0, 0.0),
        ] {
            let w = camera.screen_to_world(p);
            assert!(w.x >= min[0] - 1e-3 && w.x <= max[0] + 1e-3);
            assert!(w.y >= min[1] - 1e-3 && w.y <= max[1] + 1e-3);
        }
    }
}
