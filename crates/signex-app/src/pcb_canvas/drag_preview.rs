//! Footprints drawn where a drag or Move is taking them (#2358). While the
//! carry is anchored, the board content (CPU layer and GPU scene alike) is
//! a snapshot without the carried footprints, built once when the carry
//! starts, and the carried footprints are a small scene of their own at the
//! carry's offset, painted by the highlight layer above both mounts. The
//! offset moves in grid steps, so only that small scene is rebuilt, and only
//! when a step is crossed.

use std::collections::BTreeSet;

use iced::widget::canvas;
use iced::{Rectangle, Renderer};
use signex_gfx::scene::Scene;
use signex_renderer::pcb::PcbSnapshot;
use signex_types::pcb::Point;
use uuid::Uuid;

use super::content::cull_rect;
use super::cull::SceneBounds;
use super::tessellate::{tessellate, visible_items};
use super::{BoardCamera, PcbCanvas};

/// A drag preview in progress on one canvas.
#[derive(Debug, Default)]
pub(super) struct DragPreview {
    /// The footprints the base snapshot leaves out.
    footprints: BTreeSet<Uuid>,
    /// The board snapshot the preview replaced, put back when it ends.
    held: Option<PcbSnapshot>,
    /// The carried footprints at their offset.
    moving: Option<MovingScene>,
}

#[derive(Debug)]
struct MovingScene {
    offset: Point,
    scene: Scene,
    bounds: SceneBounds,
}

impl PcbCanvas {
    /// Whether the board content leaves out `footprints` for a drag preview.
    pub fn previews_drag_of(&self, footprints: &BTreeSet<Uuid>) -> bool {
        self.drag_preview
            .as_ref()
            .is_some_and(|p| p.footprints == *footprints)
    }

    /// Whether a drag preview is in progress.
    pub fn previews_drag(&self) -> bool {
        self.drag_preview.is_some()
    }

    /// Start a drag preview of `footprints`: the board content becomes
    /// `base`, the board without them. The snapshot in place is kept for
    /// [`Self::end_drag_preview`].
    pub fn begin_drag_preview(&mut self, footprints: BTreeSet<Uuid>, base: PcbSnapshot) {
        self.end_drag_preview();
        let held = self.renderer_snapshot.take();
        self.replace_snapshot(Some(base));
        self.drag_preview = Some(DragPreview {
            footprints,
            held,
            moving: None,
        });
    }

    /// Draw the carried footprints at `offset`; `snapshot` builds them there
    /// and runs only when the offset changed.
    pub fn move_drag_preview(&mut self, offset: Point, snapshot: impl FnOnce() -> PcbSnapshot) {
        let Some(preview) = self.drag_preview.as_ref() else {
            return;
        };
        if preview.moving.as_ref().is_some_and(|m| m.offset == offset) {
            return;
        }
        let scene = self.build_scene(&snapshot());
        let bounds = SceneBounds::from_scene(&scene);
        if let Some(preview) = self.drag_preview.as_mut() {
            preview.moving = Some(MovingScene {
                offset,
                scene,
                bounds,
            });
        }
    }

    /// End a drag preview: the board content is the snapshot it replaced.
    pub fn end_drag_preview(&mut self) {
        if let Some(preview) = self.drag_preview.take() {
            self.replace_snapshot(preview.held);
        }
    }

    /// The carried footprints' scene, if a preview draws one.
    pub fn drag_preview_scene(&self) -> Option<&Scene> {
        self.drag_preview
            .as_ref()
            .and_then(|p| p.moving.as_ref())
            .map(|m| &m.scene)
    }

    /// The carried footprints tessellated for a `viewport` of the highlight
    /// layer at `camera`; empty without a preview.
    pub(super) fn drag_preview_geometry(
        &self,
        renderer: &Renderer,
        viewport: Rectangle,
        camera: &BoardCamera,
    ) -> Vec<canvas::Geometry> {
        let Some(moving) = self.drag_preview.as_ref().and_then(|p| p.moving.as_ref()) else {
            return Vec::new();
        };
        let view = cull_rect(camera, viewport);
        let (items, _) = visible_items(&moving.scene, &moving.bounds, view);
        tessellate(renderer, viewport, &items, camera, 1)
    }
}
