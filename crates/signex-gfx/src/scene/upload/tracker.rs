//! Decides what a GPU surface has to re-upload for the next frame.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.
//!
//! A surface keeps one [`UploadTracker`] next to its GPU buffers. Each frame it
//! asks [`UploadTracker::plan`] for the dirty categories and the culling window,
//! then feeds both to [`super::apply_dirty_uploads_with_culling`].
//!
//! Two things make a category dirty:
//! * the scene changed - found by comparing against the scene that is resident
//!   on the GPU, narrowed by the producer's own [`Scene::dirty`] hint;
//! * the view moved outside the window the resident geometry was culled to.
//!
//! The cull window is the visible rectangle plus a margin, so a pan inside the
//! margin costs nothing and an item at the viewport edge is already resident
//! before it scrolls in.

use std::sync::Arc;

use super::{UploadCulling, ViewportAabbMm};
use crate::scene::{DirtyFlags, Scene};

/// Geometry categories whose buffers are culled to the window.
pub const GEOMETRY_FLAGS: DirtyFlags = DirtyFlags::LINES
    .union(DirtyFlags::CIRCLES)
    .union(DirtyFlags::ARCS)
    .union(DirtyFlags::POLYGONS)
    .union(DirtyFlags::OVERLAY);

/// How far the cull window extends past the visible rectangle, as a fraction
/// of its width/height on each side.
pub const CULL_MARGIN_FRACTION: f32 = 0.5;

/// What to upload this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UploadPlan {
    pub dirty: DirtyFlags,
    pub culling: UploadCulling,
}

/// The scene resident on the GPU, and the window it was culled to.
#[derive(Debug, Default)]
pub struct UploadTracker {
    resident: Option<Arc<Scene>>,
    generation: Option<u64>,
    /// Window the resident geometry was culled to; `None` = not culled.
    culled_for: Option<ViewportAabbMm>,
}

impl UploadTracker {
    /// Plan the uploads for `scene` shown through `visible` (world mm), or
    /// `None` when the view is degenerate and nothing can be culled.
    ///
    /// `generation` is the producer's identity for the geometry: equal
    /// generations mean unchanged. `None` means unknown, so the scene is
    /// compared by value against the resident one.
    ///
    /// Commits the new state: the caller is expected to apply the plan.
    pub fn plan(
        &mut self,
        scene: &Arc<Scene>,
        generation: Option<u64>,
        visible: Option<ViewportAabbMm>,
    ) -> UploadPlan {
        let mut dirty = match &self.resident {
            None => GEOMETRY_FLAGS,
            Some(prev) if Arc::ptr_eq(prev, scene) => DirtyFlags::empty(),
            Some(_) if generation.is_some() && generation == self.generation => DirtyFlags::empty(),
            Some(prev) => {
                let hint = if scene.dirty.is_empty() {
                    DirtyFlags::ALL
                } else {
                    scene.dirty
                };
                scene.changed_geometry_since(prev) & hint
            }
        };

        let window_valid = match (self.culled_for, visible) {
            // Resident geometry is complete; anything fits inside it.
            (None, _) => self.resident.is_some(),
            (Some(_), None) => false,
            (Some(window), Some(visible)) => window.contains(&visible),
        };
        if !window_valid {
            self.culled_for = visible.map(|v| v.expanded(CULL_MARGIN_FRACTION));
            dirty |= GEOMETRY_FLAGS;
        }

        self.resident = Some(Arc::clone(scene));
        self.generation = generation;
        UploadPlan {
            dirty,
            culling: UploadCulling {
                viewport_mm: self.culled_for,
            },
        }
    }
}
