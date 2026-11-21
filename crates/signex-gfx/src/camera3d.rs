//! The orbit camera of a 3D view (#2390): a point it looks at, the turn
//! about the vertical axis, the elevation above the view's base plane and
//! the distance. View state only; the math the GPU camera, picking, fit,
//! the presets, the flip and its animation all read, and the lighting
//! presets the mesh shader is lit by.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: wgpu/WGSL public docs, glam public docs.

use glam::{Mat4, Vec3, Vec4};

/// Elevation limits in degrees: straight down onto the top face, straight
/// up at the bottom face.
const MAX_PITCH_DEG: f32 = 90.0;
/// Degrees one pixel of drag turns the view.
const ORBIT_DEG_PER_PX: f32 = 0.4;
/// How far one wheel step moves in or out, as a factor of the distance.
const ZOOM_STEP: f32 = 1.15;
const MIN_DISTANCE_MM: f32 = 0.5;
const MAX_DISTANCE_MM: f32 = 100_000.0;
/// Space left around what a fit frames, as a factor of its radius.
const FIT_MARGIN: f32 = 1.1;

/// A named viewpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewPreset {
    /// Straight down onto the top face, as the flat view shows it.
    Top,
    /// Straight up at the bottom face, mirrored left to right.
    Bottom,
    /// Level, from the front edge (the low y side).
    Front,
    /// Level, from the left edge.
    Left,
    /// Level, from the right edge.
    Right,
    /// From above the front-left corner.
    Iso,
    /// Level, from the back edge (the high y side).
    Back,
}

impl ViewPreset {
    /// Every preset, in the order a toolbar lists them.
    pub const ALL: [Self; 7] = [
        Self::Top,
        Self::Bottom,
        Self::Front,
        Self::Left,
        Self::Right,
        Self::Iso,
        Self::Back,
    ];

    /// `(yaw, pitch)` in degrees.
    fn angles(self) -> (f32, f32) {
        match self {
            Self::Top => (0.0, MAX_PITCH_DEG),
            Self::Bottom => (180.0, -MAX_PITCH_DEG),
            Self::Front => (0.0, 0.0),
            Self::Left => (-90.0, 0.0),
            Self::Right => (90.0, 0.0),
            Self::Iso => (-35.0, 35.0),
            Self::Back => (180.0, 0.0),
        }
    }
}

/// Which side of the board a view looks at: the presets as they are, or
/// with the board turned over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ViewSide {
    #[default]
    Top,
    /// The board turned over about the axis that is vertical on screen
    /// (its y axis in the top view), so the bottom faces the eye where the
    /// top did.
    Bottom,
}

/// How long the animated turn of a flip takes, milliseconds.
pub const FLIP_DURATION_MS: u64 = 400;

/// A lighting preset of a 3D view. Each sets the light's colour, how much
/// of the shading is ambient, and how far above the eye the key light sits;
/// the light still follows the eye, so every view reads. The view's
/// backdrop follows the preset, not the UI theme (the caller paints it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LightingPreset {
    /// White light, 35% ambient, the key a little above the eye: the
    /// shading the 3D view has always had.
    #[default]
    Studio,
    /// Warm light (1.00, 0.91, 0.78), 38% ambient, the key as Studio's.
    Workshop,
    /// Cool, brighter light (1.00, 1.04, 1.12), 45% ambient, the key high
    /// above the eye like the sky.
    Outdoor,
    /// Dim blue-grey light (0.62, 0.65, 0.72), 12% ambient, the key as
    /// Studio's, so faces turned away fall nearly black.
    Dark,
}

impl LightingPreset {
    /// Every preset, in the order the cycle visits them.
    pub const ALL: [Self; 4] = [Self::Studio, Self::Workshop, Self::Outdoor, Self::Dark];

    /// The preset after this one; Dark comes back to Studio.
    pub fn next(self) -> Self {
        match self {
            Self::Studio => Self::Workshop,
            Self::Workshop => Self::Outdoor,
            Self::Outdoor => Self::Dark,
            Self::Dark => Self::Studio,
        }
    }

    /// Where the preset is in the cycle, from 1 (Studio) to 4 (Dark).
    pub fn ordinal(self) -> usize {
        match self {
            Self::Studio => 1,
            Self::Workshop => 2,
            Self::Outdoor => 3,
            Self::Dark => 4,
        }
    }

    /// The light colour (rgb) with the ambient share (a), and how far the
    /// key sits above the eye: screen-up units per unit toward the eye.
    fn light(self) -> ([f32; 4], f32) {
        match self {
            Self::Studio => ([1.0, 1.0, 1.0, 0.35], 0.4),
            Self::Workshop => ([1.0, 0.91, 0.78, 0.38], 0.4),
            Self::Outdoor => ([1.0, 1.04, 1.12, 0.45], 1.2),
            Self::Dark => ([0.62, 0.65, 0.72, 0.12], 0.4),
        }
    }
}

/// `deg` turned into (-180, 180].
fn wrap_deg(deg: f32) -> f32 {
    180.0 - (180.0 - deg).rem_euclid(360.0)
}

/// Where a 3D view looks from. `yaw_deg` 0 looks from the low-y side
/// toward +y; positive turns the eye toward +x. `pitch_deg` 90 looks
/// straight down, -90 straight up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    pub target: [f32; 3],
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub distance: f32,
    pub fov_y_deg: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: [0.0; 3],
            yaw_deg: 0.0,
            pitch_deg: MAX_PITCH_DEG,
            distance: 100.0,
            fov_y_deg: 30.0,
        }
    }
}

/// What the mesh pipeline's uniform holds: the camera, the light and the
/// colour a highlighted part is washed with. 112 bytes, mirrored field for
/// field by `Camera` in `shader/mesh.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshUniform {
    pub view_proj: [[f32; 4]; 4],
    /// Toward the light, `w` unused.
    pub light_dir: [f32; 4],
    pub highlight: [f32; 4],
    /// The light colour (rgb) and the ambient share of the shading (a): a
    /// face is lit `rgb * (a + (1 - a) * diffuse)`.
    pub light: [f32; 4],
}

impl OrbitCamera {
    fn forward_and_up(&self) -> (Vec3, Vec3) {
        let (yaw, pitch) = (self.yaw_deg.to_radians(), self.pitch_deg.to_radians());
        // From the target toward the eye, and the screen's up, kept
        // perpendicular so straight down or up needs no special case.
        let back = Vec3::new(
            pitch.cos() * yaw.sin(),
            -pitch.cos() * yaw.cos(),
            pitch.sin(),
        );
        let up = Vec3::new(
            -pitch.sin() * yaw.sin(),
            pitch.sin() * yaw.cos(),
            pitch.cos(),
        );
        (back, up)
    }

    /// The eye position.
    pub fn eye(&self) -> [f32; 3] {
        let (back, _) = self.forward_and_up();
        (Vec3::from(self.target) + back * self.distance).to_array()
    }

    fn view_proj_mat(&self, aspect: f32) -> Mat4 {
        let (back, up) = self.forward_and_up();
        let target = Vec3::from(self.target);
        let eye = target + back * self.distance;
        let view = Mat4::look_at_rh(eye, target, up);
        // A near plane at 5% of the distance keeps depth fine enough that copper
        // a few hundredths of a mm above the mask never fights it.
        let near = (self.distance * 0.05).max(0.01);
        let far = self.distance * 50.0 + 1000.0;
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let proj = Mat4::perspective_rh(self.fov_y_deg.to_radians(), aspect, near, far);
        proj * view
    }

    /// The uniform for a viewport `width` x `height` (any unit), lit from
    /// the eye in the Studio preset, with `highlight` as the selection wash.
    pub fn uniform(&self, width: f32, height: f32, highlight: [f32; 4]) -> MeshUniform {
        self.uniform_lit(width, height, highlight, LightingPreset::Studio)
    }

    /// [`Self::uniform`] lit by `lighting`.
    pub fn uniform_lit(
        &self,
        width: f32,
        height: f32,
        highlight: [f32; 4],
        lighting: LightingPreset,
    ) -> MeshUniform {
        let (back, up) = self.forward_and_up();
        let (light, lift) = lighting.light();
        // A head light above the eye, so faces facing the eye are lit and
        // level edges still shade apart.
        let toward = (back + up * lift).normalize_or_zero();
        MeshUniform {
            view_proj: self.view_proj_mat(width / height).to_cols_array_2d(),
            light_dir: [toward.x, toward.y, toward.z, 0.0],
            highlight,
            light,
        }
    }

    /// Where `point` lands in a viewport `width` x `height`, from its
    /// top-left corner; `None` behind the eye.
    pub fn project(&self, point: [f32; 3], width: f32, height: f32) -> Option<[f32; 2]> {
        let clip =
            self.view_proj_mat(width / height) * Vec4::new(point[0], point[1], point[2], 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip / clip.w;
        Some([(ndc.x + 1.0) * 0.5 * width, (1.0 - ndc.y) * 0.5 * height])
    }

    /// The ray through viewport point `at` (from the top-left corner of a
    /// `width` x `height` viewport): its origin and unit direction.
    pub fn ray(&self, at: [f32; 2], width: f32, height: f32) -> ([f32; 3], [f32; 3]) {
        let inverse = self.view_proj_mat(width / height).inverse();
        let x = 2.0 * at[0] / width - 1.0;
        let y = 1.0 - 2.0 * at[1] / height;
        let near = inverse.project_point3(Vec3::new(x, y, 0.0));
        let far = inverse.project_point3(Vec3::new(x, y, 1.0));
        (near.to_array(), (far - near).normalize_or_zero().to_array())
    }

    /// Turn the view by a drag of `dx`, `dy` pixels.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw_deg = (self.yaw_deg - dx * ORBIT_DEG_PER_PX).rem_euclid(360.0);
        self.pitch_deg =
            (self.pitch_deg + dy * ORBIT_DEG_PER_PX).clamp(-MAX_PITCH_DEG, MAX_PITCH_DEG);
    }

    /// Slide the view by a drag of `dx`, `dy` pixels in a viewport
    /// `height` pixels tall: the point under the pointer stays under it at
    /// the target's depth.
    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        if height <= 0.0 {
            return;
        }
        let (back, up) = self.forward_and_up();
        let right = up.cross(back);
        let mm_per_px = 2.0 * self.distance * (self.fov_y_deg.to_radians() * 0.5).tan() / height;
        let moved = Vec3::from(self.target) - right * dx * mm_per_px + up * dy * mm_per_px;
        self.target = moved.to_array();
    }

    /// Move in (`steps` > 0) or out by wheel steps.
    pub fn zoom(&mut self, steps: f32) {
        let factor = ZOOM_STEP.powf(-steps);
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE_MM, MAX_DISTANCE_MM);
    }

    /// Look from `preset`, keeping the target and distance.
    pub fn set_preset(&mut self, preset: ViewPreset) {
        let (yaw, pitch) = preset.angles();
        self.yaw_deg = yaw.rem_euclid(360.0);
        self.pitch_deg = pitch;
    }

    /// Look from `preset` at `side` of the board, keeping the target and
    /// distance. On the top side this is [`Self::set_preset`]; on the
    /// bottom side the preset's view is [`Self::flipped`], so the top
    /// preset on the bottom side is [`ViewPreset::Bottom`].
    pub fn set_preset_on(&mut self, preset: ViewPreset, side: ViewSide) {
        self.set_preset(preset);
        if side == ViewSide::Bottom {
            *self = self.flipped();
        }
    }

    /// This view with the board turned over about the axis that is
    /// vertical on screen: the eye goes to the far side (yaw + 180°,
    /// wrapped to (-180, 180]; pitch negated) and the screen's up stays.
    /// The target, distance and field of view are kept; flipping twice
    /// comes back.
    pub fn flipped(&self) -> OrbitCamera {
        OrbitCamera {
            yaw_deg: wrap_deg(self.yaw_deg + 180.0),
            pitch_deg: -self.pitch_deg,
            ..*self
        }
    }

    /// The view `t` of the way from this one to `to`, for an animated
    /// move: `t` is clamped to 0..=1 and eased (smoothstep), the yaw turns
    /// the short way round, and the target, pitch, distance and field of
    /// view move in a straight line. `t` = 0 is this view and `t` = 1 is
    /// `to`, exactly.
    pub fn tween(&self, to: &OrbitCamera, t: f32) -> OrbitCamera {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        if t <= 0.0 {
            return *self;
        }
        if t >= 1.0 {
            return *to;
        }
        let eased = t * t * (3.0 - 2.0 * t);
        let lerp = |a: f32, b: f32| a + (b - a) * eased;
        let turn = wrap_deg(to.yaw_deg - self.yaw_deg);
        OrbitCamera {
            target: [
                lerp(self.target[0], to.target[0]),
                lerp(self.target[1], to.target[1]),
                lerp(self.target[2], to.target[2]),
            ],
            yaw_deg: wrap_deg(self.yaw_deg + turn * eased),
            pitch_deg: lerp(self.pitch_deg, to.pitch_deg),
            distance: lerp(self.distance, to.distance),
            fov_y_deg: lerp(self.fov_y_deg, to.fov_y_deg),
        }
    }

    /// Frame the box `lo`..`hi` in a viewport of `aspect` (width over
    /// height), keeping the direction it is seen from.
    pub fn fit(&mut self, lo: [f32; 3], hi: [f32; 3], aspect: f32) {
        let (lo, hi) = (Vec3::from(lo), Vec3::from(hi));
        if !(lo.is_finite() && hi.is_finite()) {
            return;
        }
        let radius = ((hi - lo).length() * 0.5).max(MIN_DISTANCE_MM);
        let half_v = self.fov_y_deg.to_radians() * 0.5;
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let half_h = (half_v.tan() * aspect).atan();
        let half = half_v.min(half_h);
        self.target = ((lo + hi) * 0.5).to_array();
        self.distance = (radius * FIT_MARGIN / half.sin()).clamp(MIN_DISTANCE_MM, MAX_DISTANCE_MM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2
    }

    /// The top view shows the board as the flat view does: +x right, +y up.
    #[test]
    fn the_top_view_keeps_x_right_and_y_up() {
        let camera = OrbitCamera::default();
        let centre = camera.project([0.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        let right = camera.project([10.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        let up = camera.project([0.0, 10.0, 0.0], 800.0, 600.0).unwrap();
        assert!(close(centre, [400.0, 300.0]));
        assert!(right[0] > centre[0] && (right[1] - centre[1]).abs() < 1e-3);
        assert!(up[1] < centre[1] && (up[0] - centre[0]).abs() < 1e-3);
    }

    /// The bottom view mirrors left to right and keeps y up.
    #[test]
    fn the_bottom_view_mirrors_x() {
        let mut camera = OrbitCamera::default();
        camera.set_preset(ViewPreset::Bottom);
        let centre = camera.project([0.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        let right = camera.project([10.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        let up = camera.project([0.0, 10.0, 0.0], 800.0, 600.0).unwrap();
        assert!(right[0] < centre[0], "+x is on the left from below");
        assert!(up[1] < centre[1], "+y is still up");
        assert!(camera.eye()[2] < 0.0, "the eye is under the board");
    }

    /// A ray through a projected point comes back to that point.
    #[test]
    fn a_ray_through_a_projected_point_passes_through_it() {
        let mut camera = OrbitCamera::default();
        camera.set_preset(ViewPreset::Iso);
        let point = [12.0, -7.0, 3.0];
        let at = camera.project(point, 800.0, 600.0).unwrap();
        let (origin, dir) = camera.ray(at, 800.0, 600.0);
        let to = Vec3::from(point) - Vec3::from(origin);
        let off = to - Vec3::from(dir) * to.dot(Vec3::from(dir));
        assert!(off.length() < 1e-2, "{off:?}");
    }

    #[test]
    fn fit_frames_the_box_around_its_centre() {
        let mut camera = OrbitCamera::default();
        camera.fit([0.0, 0.0, 0.0], [100.0, 50.0, 2.0], 800.0 / 600.0);
        assert_eq!(camera.target, [50.0, 25.0, 1.0]);
        for corner in [[0.0, 0.0, 0.0], [100.0, 50.0, 2.0], [0.0, 50.0, 0.0]] {
            let at = camera.project(corner, 800.0, 600.0).unwrap();
            assert!((0.0..=800.0).contains(&at[0]) && (0.0..=600.0).contains(&at[1]));
        }
    }

    #[test]
    fn orbit_keeps_the_elevation_within_straight_up_and_down() {
        let mut camera = OrbitCamera::default();
        camera.orbit(0.0, 1000.0);
        assert_eq!(camera.pitch_deg, 90.0);
        camera.orbit(0.0, -5000.0);
        assert_eq!(camera.pitch_deg, -90.0);
    }

    #[test]
    fn zoom_in_moves_closer_and_out_moves_back() {
        let mut camera = OrbitCamera::default();
        camera.zoom(1.0);
        assert!(camera.distance < 100.0);
        camera.zoom(-1.0);
        assert!((camera.distance - 100.0).abs() < 1e-3);
    }

    /// A pan moves the target in the screen plane, by the drag.
    #[test]
    fn a_pan_keeps_the_point_under_the_pointer() {
        let mut camera = OrbitCamera::default();
        let before = camera.project([0.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        camera.pan(40.0, -20.0, 600.0);
        let after = camera.project([0.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        assert!(close(
            [after[0] - before[0], after[1] - before[1]],
            [40.0, -20.0]
        ));
    }

    /// The same angle, whichever turn of the circle each is written in.
    fn same_angle(a: f32, b: f32) -> bool {
        wrap_deg(a - b).abs() < 1e-3
    }

    fn same_view(a: &OrbitCamera, b: &OrbitCamera) -> bool {
        let near = |x: f32, y: f32| (x - y).abs() < 1e-3;
        (0..3).all(|i| near(a.target[i], b.target[i]))
            && same_angle(a.yaw_deg, b.yaw_deg)
            && near(a.pitch_deg, b.pitch_deg)
            && near(a.distance, b.distance)
            && near(a.fov_y_deg, b.fov_y_deg)
    }

    fn sample() -> OrbitCamera {
        OrbitCamera {
            target: [12.0, -7.0, 0.8],
            yaw_deg: 250.0,
            pitch_deg: 35.0,
            distance: 140.0,
            fov_y_deg: 30.0,
        }
    }

    #[test]
    fn every_preset_is_listed_once() {
        assert_eq!(ViewPreset::ALL.len(), 7);
        for (i, a) in ViewPreset::ALL.iter().enumerate() {
            assert!(!ViewPreset::ALL[i + 1..].contains(a), "{a:?} twice");
        }
        assert_eq!(ViewPreset::ALL[6], ViewPreset::Back);
    }

    /// Back looks level from the high y side toward -y; +x is on the left.
    #[test]
    fn the_back_view_looks_level_from_high_y() {
        let mut camera = OrbitCamera::default();
        camera.set_preset(ViewPreset::Back);
        let eye = camera.eye();
        assert!(eye[1] > 50.0, "{eye:?}");
        assert!(eye[0].abs() < 1e-3 && eye[2].abs() < 1e-3, "{eye:?}");
        let centre = camera.project([0.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        let right = camera.project([10.0, 0.0, 0.0], 800.0, 600.0).unwrap();
        let up = camera.project([0.0, 0.0, 10.0], 800.0, 600.0).unwrap();
        assert!(right[0] < centre[0], "+x is on the left from the back");
        assert!(up[1] < centre[1], "+z is up");
    }

    #[test]
    fn the_top_preset_on_the_bottom_side_is_the_bottom_preset() {
        let mut on_bottom = OrbitCamera::default();
        on_bottom.set_preset_on(ViewPreset::Top, ViewSide::Bottom);
        let mut bottom = OrbitCamera::default();
        bottom.set_preset(ViewPreset::Bottom);
        assert_eq!(on_bottom, bottom);
    }

    #[test]
    fn a_preset_on_the_top_side_is_the_plain_preset() {
        for preset in ViewPreset::ALL {
            let mut on_top = sample();
            on_top.set_preset_on(preset, ViewSide::Top);
            let mut plain = sample();
            plain.set_preset(preset);
            assert_eq!(on_top, plain, "{preset:?}");
        }
    }

    /// A flip keeps the target, distance, field of view and the screen's
    /// up, and puts the eye on the far side; twice comes back.
    #[test]
    fn flipping_twice_comes_back() {
        let camera = sample();
        let once = camera.flipped();
        assert_eq!(once.target, camera.target);
        assert_eq!(once.distance, camera.distance);
        assert_eq!(once.fov_y_deg, camera.fov_y_deg);
        assert!(once.yaw_deg > -180.0 && once.yaw_deg <= 180.0);
        let (eye, flipped_eye) = (Vec3::from(camera.eye()), Vec3::from(once.eye()));
        let target = Vec3::from(camera.target);
        assert!(((eye - target) + (flipped_eye - target)).length() < 1e-2);
        assert!((camera.forward_and_up().1 - once.forward_and_up().1).length() < 1e-5);
        assert!(same_view(&once.flipped(), &camera));
    }

    #[test]
    fn a_tween_ends_exactly_at_its_two_views() {
        let from = sample();
        let mut to = OrbitCamera::default();
        to.set_preset(ViewPreset::Iso);
        to.distance = 80.0;
        assert_eq!(from.tween(&to, 0.0), from);
        assert_eq!(from.tween(&to, 1.0), to);
        assert_eq!(from.tween(&to, -2.0), from, "clamped below");
        assert_eq!(from.tween(&to, 3.0), to, "clamped above");
        let half = from.tween(&to, 0.5);
        assert!((half.distance - 110.0).abs() < 1e-3, "{half:?}");
        let early = from.tween(&to, 0.1);
        assert!(
            (early.distance - 140.0).abs() < (140.0 - 80.0) * 0.1,
            "eased: slow at the start"
        );
    }

    /// From just short of a half turn one way to just past it, the yaw
    /// crosses the back, not the front.
    #[test]
    fn a_tween_turns_the_short_way() {
        let from = OrbitCamera {
            yaw_deg: 170.0,
            ..sample()
        };
        let to = OrbitCamera {
            yaw_deg: -170.0,
            ..sample()
        };
        let half = from.tween(&to, 0.5);
        assert!(same_angle(half.yaw_deg, 180.0), "{}", half.yaw_deg);
        let from = OrbitCamera {
            yaw_deg: 350.0,
            ..sample()
        };
        let to = OrbitCamera {
            yaw_deg: 10.0,
            ..sample()
        };
        assert!(same_angle(from.tween(&to, 0.5).yaw_deg, 0.0));
    }

    #[test]
    fn the_plain_uniform_is_the_studio_preset() {
        let camera = sample();
        let wash = [0.1, 0.2, 0.3, 0.4];
        assert_eq!(
            camera.uniform(800.0, 600.0, wash),
            camera.uniform_lit(800.0, 600.0, wash, LightingPreset::Studio)
        );
        let studio = camera.uniform(800.0, 600.0, wash);
        assert_eq!(studio.light, [1.0, 1.0, 1.0, 0.35]);
        let (back, up) = camera.forward_and_up();
        let head = (back + up * 0.4).normalize_or_zero();
        assert_eq!(studio.light_dir, [head.x, head.y, head.z, 0.0]);
    }

    #[test]
    fn every_lighting_preset_lights_the_view_its_own_way() {
        let camera = sample();
        let lights: Vec<[f32; 4]> = LightingPreset::ALL
            .iter()
            .map(|&preset| camera.uniform_lit(800.0, 600.0, [0.0; 4], preset).light)
            .collect();
        for (i, light) in lights.iter().enumerate() {
            assert!(!lights[i + 1..].contains(light), "{i} shares its light");
            assert!((0.0..1.0).contains(&light[3]), "an ambient share");
        }
    }

    /// The uniform is a whole number of 16-byte rows, as WGSL lays it out.
    #[test]
    fn the_mesh_uniform_is_112_bytes() {
        assert_eq!(std::mem::size_of::<MeshUniform>(), 112);
        assert_eq!(std::mem::size_of::<MeshUniform>() % 16, 0);
    }

    #[test]
    fn the_lighting_cycle_visits_all_four_and_comes_back() {
        let mut preset = LightingPreset::default();
        assert_eq!(preset, LightingPreset::Studio);
        for (i, &want) in LightingPreset::ALL.iter().enumerate() {
            assert_eq!(preset, want);
            assert_eq!(preset.ordinal(), i + 1);
            preset = preset.next();
        }
        assert_eq!(preset, LightingPreset::Studio);
    }

    #[test]
    fn a_wrapped_angle_lies_above_minus_180_up_to_180() {
        for (deg, want) in [
            (180.0, 180.0),
            (-180.0, 180.0),
            (270.0, -90.0),
            (0.0, 0.0),
            (540.0, 180.0),
        ] {
            assert_eq!(wrap_deg(deg), want, "{deg}");
        }
    }
}
