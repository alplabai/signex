//! Camera-independent cache of the schematic content [`Scene`].
//!
//! Panning changes only the camera offset. Rebuilding the renderer snapshot
//! and the scene on every pan frame (#171 item 6) repeats work whose output is
//! world-space and does not depend on the offset. [`ContentSceneCache`] keeps
//! the built scene and hands it back while the request is compatible; the
//! scene is then replayed through the *live* transform at draw time.
//!
//! What a built scene depends on, and so what invalidates it:
//! - the document, selection-driven focus set and wire styling: the owner
//!   calls [`ContentSceneCache::invalidate`] (from `clear_content_cache`),
//!   and focus set / wire styling are also compared here;
//! - the theme colours, compared here;
//! - the zoom scale: markers, glyph sizes and stroke widths are authored in
//!   screen pixels, so a scale change rebuilds once per zoom step;
//! - the viewport size and a pan that leaves the built margin: the build
//!   culls to the viewport inflated by one viewport on every side, so a pan
//!   inside that margin reuses the scene.

use std::sync::Arc;

use super::*;
use crate::scene_generation::next_scene_generation;

/// What a built scene depends on, captured when it was built.
pub(crate) struct SceneInputs {
    scale: f32,
    /// The window's display scale: the replay's stroke floor and snap depend
    /// on it, so a move to another display is a different input (#1887).
    device_px_per_logical_px: f32,
    offset: (f32, f32),
    viewport: iced::Size,
    colors: CanvasColors,
    focus: Option<HashSet<uuid::Uuid>>,
    styling: Option<wire_style::OwnedWireStyling>,
    designators: Option<HashMap<uuid::Uuid, String>>,
}

struct Entry {
    built_for: SceneInputs,
    /// `None` when the sheet drew nothing. Shared by refcount so the GPU
    /// path can hold the same geometry the CPU replay reads.
    scene: Option<Arc<Scene>>,
    /// Id of `scene`, drawn from the process-wide counter at build time. The
    /// GPU pipeline keys its resident upload on it, so it changes exactly when
    /// a new scene is built and never when the cache serves a hit.
    generation: u64,
}

#[derive(Default)]
pub struct ContentSceneCache {
    entry: Option<Entry>,
    builds: u32,
}

impl ContentSceneCache {
    /// How many times the scene was built (not served from the cache).
    #[cfg(test)]
    #[must_use]
    pub fn builds(&self) -> u32 {
        self.builds
    }

    /// Generation of the scene currently held, `None` before the first build
    /// or after [`Self::invalidate`]. See [`next_scene_generation`].
    #[must_use]
    pub fn generation(&self) -> Option<u64> {
        self.entry.as_ref().map(|entry| entry.generation)
    }

    /// Drop the cached scene; the next request rebuilds.
    pub fn invalidate(&mut self) {
        self.entry = None;
    }

    /// The content scene for `transform`, built only when the cached one no
    /// longer fits. `None` when the sheet draws nothing.
    pub fn scene_for(
        &mut self,
        sheet: &SchematicRenderSnapshot,
        transform: &ScreenTransform,
        colors: &CanvasColors,
        bounds: Rectangle,
        focus_set: Option<&HashSet<uuid::Uuid>>,
        overrides: SceneOverrides<'_>,
    ) -> Option<&Arc<Scene>> {
        self.scene_for_with(
            || std::borrow::Cow::Borrowed(sheet),
            transform,
            colors,
            bounds,
            focus_set,
            overrides,
        )
    }

    /// [`Self::scene_for`] for a sheet that is costly to produce: `sheet` runs
    /// only when the scene must be built, never on a cache hit.
    pub fn scene_for_with<'s>(
        &mut self,
        sheet: impl FnOnce() -> std::borrow::Cow<'s, SchematicRenderSnapshot>,
        transform: &ScreenTransform,
        colors: &CanvasColors,
        bounds: Rectangle,
        focus_set: Option<&HashSet<uuid::Uuid>>,
        overrides: SceneOverrides<'_>,
    ) -> Option<&Arc<Scene>> {
        let reusable = self.entry.as_ref().is_some_and(|entry| {
            entry
                .built_for
                .fits(transform, colors, bounds.size(), focus_set, overrides)
        });
        if !reusable {
            let margin = bounds.size();
            let build_bounds = Rectangle::new(
                iced::Point::new(-margin.width, -margin.height),
                iced::Size::new(margin.width * 3.0, margin.height * 3.0),
            );
            let scene = build_schematic_scene(
                &sheet(),
                transform,
                colors,
                build_bounds,
                focus_set,
                overrides,
            )
            .map(Arc::new);
            self.builds += 1;
            self.entry = Some(Entry {
                built_for: SceneInputs::capture(
                    transform,
                    colors,
                    bounds.size(),
                    focus_set,
                    overrides,
                ),
                scene,
                generation: next_scene_generation(),
            });
        }
        self.entry.as_ref().and_then(|entry| entry.scene.as_ref())
    }
}

impl SceneInputs {
    /// Whether the wire styling this scene was built with equals `now`; no
    /// styling and empty styling are the same thing.
    fn styling_matches(&self, now: Option<WireStyling<'_>>) -> bool {
        match &self.styling {
            Some(built) => built.matches(now),
            None => now.is_none_or(|s| s.color_overrides.is_empty() && s.class_styles.is_empty()),
        }
    }

    /// The inputs of a scene about to be built for `transform`.
    pub(crate) fn capture(
        transform: &ScreenTransform,
        colors: &CanvasColors,
        viewport: iced::Size,
        focus_set: Option<&HashSet<uuid::Uuid>>,
        overrides: SceneOverrides<'_>,
    ) -> Self {
        Self {
            scale: transform.scale,
            device_px_per_logical_px: transform.device_px_per_logical_px,
            offset: (transform.offset_x, transform.offset_y),
            viewport,
            colors: *colors,
            focus: focus_set.cloned(),
            styling: overrides.wire_styling.map(WireStyling::to_owned_styling),
            designators: overrides.designators.cloned(),
        }
    }

    /// Everything but the pan offset matches.
    fn same_but_pan(
        &self,
        transform: &ScreenTransform,
        colors: &CanvasColors,
        viewport: iced::Size,
        focus_set: Option<&HashSet<uuid::Uuid>>,
        overrides: SceneOverrides<'_>,
    ) -> bool {
        (self.scale - transform.scale).abs() < 0.0001
            && self.device_px_per_logical_px == transform.device_px_per_logical_px
            && self.viewport == viewport
            && self.colors == *colors
            && self.focus.as_ref() == focus_set
            && self.styling_matches(overrides.wire_styling)
            && self.designators.as_ref() == overrides.designators
    }

    /// Like [`Self::fits`] but a pan of a pixel does not fit: for geometry
    /// kept in screen space, which a pan would leave misplaced.
    pub(crate) fn fits_in_place(
        &self,
        transform: &ScreenTransform,
        colors: &CanvasColors,
        viewport: iced::Size,
        focus_set: Option<&HashSet<uuid::Uuid>>,
        overrides: SceneOverrides<'_>,
    ) -> bool {
        self.same_but_pan(transform, colors, viewport, focus_set, overrides)
            && (self.offset.0 - transform.offset_x).abs() < 0.01
            && (self.offset.1 - transform.offset_y).abs() < 0.01
    }

    fn fits(
        &self,
        transform: &ScreenTransform,
        colors: &CanvasColors,
        viewport: iced::Size,
        focus_set: Option<&HashSet<uuid::Uuid>>,
        overrides: SceneOverrides<'_>,
    ) -> bool {
        if !self.same_but_pan(transform, colors, viewport, focus_set, overrides) {
            return false;
        }
        // The visible window, expressed in the screen space the scene was
        // built in, must stay inside the built (inflated) rectangle.
        let dx = self.offset.0 - transform.offset_x;
        let dy = self.offset.1 - transform.offset_y;
        dx.abs() <= viewport.width && dy.abs() <= viewport.height
    }
}
