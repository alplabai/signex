//! The library part on the cursor (#2364), drawn by the board renderer and
//! dimmed, in the highlight layer so it sits over the GPU mount too. The app
//! hands over a snapshot of the board holding only the part where the next
//! click would place it; the scene is built here once per change.

use std::sync::Arc;

use iced::widget::canvas;
use signex_gfx::scene::Scene;
use signex_renderer::pcb::PcbSnapshot;

use super::PcbCanvas;
use super::tessellate::paint_scene;

/// How much of its colour the part on the cursor keeps.
const GHOST_ALPHA: f32 = 0.5;

impl PcbCanvas {
    /// Show `ghost`, the part on the cursor, dimmed, or nothing for `None`.
    pub fn set_part_ghost(&mut self, ghost: Option<&PcbSnapshot>) {
        self.set_ghost(ghost, GHOST_ALPHA);
    }

    /// Show `ghost` at its full colour: a route being drawn is copper as
    /// bright as what is on the board, never dimmer.
    pub fn set_route_ghost(&mut self, ghost: Option<&PcbSnapshot>) {
        self.set_ghost(ghost, 1.0);
    }

    fn set_ghost(&mut self, ghost: Option<&PcbSnapshot>, alpha: f32) {
        self.part_ghost = ghost.map(|snapshot| {
            let mut scene = self.build_scene(snapshot);
            dim(&mut scene, alpha);
            (Arc::new(scene), alpha)
        });
    }

    /// Whether a part is on the cursor of this canvas.
    pub fn has_part_ghost(&self) -> bool {
        self.part_ghost.is_some()
    }

    /// The share of its colour the ghost on the cursor keeps.
    pub fn ghost_alpha(&self) -> Option<f32> {
        self.part_ghost.as_ref().map(|(_, alpha)| *alpha)
    }

    /// Paint the part on the cursor onto `frame`.
    pub(super) fn paint_part_ghost(&self, frame: &mut canvas::Frame) {
        if let Some((scene, _)) = &self.part_ghost {
            paint_scene(frame, scene, &self.camera_ref());
        }
    }
}

/// Scale the alpha of every colour in `scene` by `alpha`.
fn dim(scene: &mut Scene, alpha: f32) {
    let fade = |color: &mut [f32; 4]| color[3] *= alpha;
    for line in scene.lines.iter_mut().chain(&mut scene.overlay_lines) {
        fade(&mut line.color);
    }
    for circle in scene.circles.iter_mut().chain(&mut scene.overlay_circles) {
        fade(&mut circle.color);
    }
    for polygon in scene
        .polygons
        .iter_mut()
        .chain(&mut scene.top_polygons)
        .chain(&mut scene.overlay_polygons)
    {
        fade(&mut polygon.fill_color);
        if let Some(stroke) = polygon.stroke_color.as_mut() {
            fade(stroke);
        }
    }
}
