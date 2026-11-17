//! The orbit camera of a 3D view (#2390): a point it looks at, the turn
//! about the vertical axis, the elevation above the view's base plane and
//! the distance. View state only; the math the GPU camera, picking, fit and
//! the presets all read.
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
}

impl ViewPreset {
    /// Every preset, in the order a toolbar lists them.
    pub const ALL: [Self; 6] = [
        Self::Top,
        Self::Bottom,
        Self::Front,
        Self::Left,
        Self::Right,
        Self::Iso,
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
        }
    }
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
/// colour a highlighted part is washed with. 96 bytes, mirrored by
/// `Camera` in `shader/mesh.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshUniform {
    pub view_proj: [[f32; 4]; 4],
    /// Toward the light, `w` unused.
    pub light_dir: [f32; 4],
    pub highlight: [f32; 4],
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
    /// the eye, with `highlight` as the selection wash.
    pub fn uniform(&self, width: f32, height: f32, highlight: [f32; 4]) -> MeshUniform {
        let (back, up) = self.forward_and_up();
        // A head light, a little above the eye, so faces facing the eye are
        // lit and level edges still shade apart.
        let light = (back + up * 0.4).normalize_or_zero();
        MeshUniform {
            view_proj: self.view_proj_mat(width / height).to_cols_array_2d(),
            light_dir: [light.x, light.y, light.z, 0.0],
            highlight,
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
}
