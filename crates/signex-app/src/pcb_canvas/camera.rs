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

/// The closest board zoom, in pixels per mm: fine pitch copper needs far more
/// than a sheet does. The footprint editor uses the same limit.
pub const MAX_BOARD_SCALE: f32 = 400.0;

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

    pub fn zoom_at(&mut self, at: Point, notches: f32, step: f32, _bounds: Rectangle) -> bool {
        if notches == 0.0 {
            return false;
        }
        self.zoom_by_factor(at, Camera::zoom_factor_for_scroll(step, notches))
    }

    pub fn zoom_by_factor(&mut self, at: Point, factor: f32) -> bool {
        self.camera
            .zoom_by_factor_up_to(at, factor, MAX_BOARD_SCALE)
    }

    pub fn apply_scroll(
        &mut self,
        at: Point,
        intent: ScrollIntent,
        step: f32,
        bounds: Rectangle,
    ) -> bool {
        match intent {
            ScrollIntent::Zoom { notches } => self.zoom_at(at, notches, step, bounds),
            pan => self.camera.apply_scroll(at, pan, step, bounds),
        }
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

    /// The corners and clockwise turn a box dragged on screen from board
    /// point `from` to `to` is judged with (window or crossing, and what it
    /// covers). Seen from below (#2394) that box is the one dragged the
    /// other way on a view turned the other way.
    pub fn box_frame<T>(&self, from: T, to: T) -> (T, T, f32) {
        let rotation = self.rotation;
        if rotation.is_mirrored() {
            (to, from, (360.0 - rotation.degrees()) % 360.0)
        } else {
            (from, to, rotation.degrees())
        }
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
        let mirrored = self.camera.get_mut().rotation().is_mirrored();
        let rotation = ViewRotation::from_degrees(degrees).with_mirror(mirrored);
        self.set_camera_rotation(rotation)
    }

    /// Whether the view shows the board from below (#2394).
    pub fn view_flipped(&self) -> bool {
        self.camera.borrow().rotation().is_mirrored()
    }

    /// Show the board from below (mirrored left to right) or from above,
    /// about the centre of the canvas; the board is not touched. Whether
    /// the view changed.
    pub fn set_view_flipped(&mut self, flipped: bool) -> bool {
        let rotation = self.camera.get_mut().rotation().with_mirror(flipped);
        self.set_camera_rotation(rotation)
    }

    fn set_camera_rotation(&mut self, rotation: ViewRotation) -> bool {
        let viewport = self.viewport.get();
        let changed = self.camera.get_mut().set_rotation(rotation, viewport);
        if changed {
            self.bg_cache.clear();
            *self.content.get_mut() = None;
        }
        changed
    }

    /// Board mm to canvas pixels under the live camera.
    pub fn board_to_screen(&self, board: signex_types::pcb::Point) -> Point {
        let world = Point::new(board.x as f32, board.y as f32);
        self.camera.borrow().world_to_screen(world)
    }

    /// Show the board at `camera` (screen offset and pixels per mm, as
    /// `live_camera` reads them), keeping the view's turn: the view a pane
    /// kept while another had the focus (mockup row 207). A queued fit is
    /// dropped; a value that is not a number is ignored.
    pub fn set_pane_camera(&mut self, camera: (f32, f32, f32)) {
        let (x, y, scale) = camera;
        if !(x.is_finite() && y.is_finite() && scale.is_finite() && scale > 0.0) {
            return;
        }
        let live = &mut self.camera.get_mut().camera;
        live.offset = Point::new(x, y);
        live.scale = scale.clamp(Camera::MIN_SCALE, MAX_BOARD_SCALE);
        self.pending_fit.set(None);
        self.fitted.set(None);
        self.bg_cache.clear();
        *self.content.get_mut() = None;
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

    /// The board zooms past the sheet limit, up to its own, and the point
    /// under the cursor stays put. The sheet camera keeps its limit.
    #[test]
    fn the_board_zooms_past_the_sheet_limit_up_to_its_own() {
        let at = Point::new(300.0, 200.0);
        let mut camera = camera_at(0.0);
        let before = camera.screen_to_world(at);
        for _ in 0..200 {
            camera.zoom_at(at, 1.0, 1.1, VIEW);
        }
        assert_eq!(camera.scale(), MAX_BOARD_SCALE);
        const { assert!(MAX_BOARD_SCALE > Camera::MAX_SCALE) };
        let after = camera.screen_to_world(at);
        assert!((after.x - before.x).abs() < 1e-2 && (after.y - before.y).abs() < 1e-2);

        let mut sheet = Camera::default();
        for _ in 0..200 {
            sheet.zoom_at(at, 1.0, 1.1, VIEW);
        }
        assert_eq!(sheet.scale, Camera::MAX_SCALE);
    }

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
