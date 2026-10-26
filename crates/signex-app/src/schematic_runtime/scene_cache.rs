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

use super::*;

/// What the cached scene was built for.
struct BuiltFor {
    scale: f32,
    offset: (f32, f32),
    viewport: iced::Size,
    colors: CanvasColors,
    focus: Option<HashSet<uuid::Uuid>>,
    styling: Option<wire_style::OwnedWireStyling>,
    designators: Option<HashMap<uuid::Uuid, String>>,
}

struct Entry {
    built_for: BuiltFor,
    /// `None` when the sheet drew nothing.
    scene: Option<Scene>,
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
    ) -> Option<&Scene> {
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
            let scene =
                build_schematic_scene(sheet, transform, colors, build_bounds, focus_set, overrides);
            self.builds += 1;
            self.entry = Some(Entry {
                built_for: BuiltFor {
                    scale: transform.scale,
                    offset: (transform.offset_x, transform.offset_y),
                    viewport: bounds.size(),
                    colors: *colors,
                    focus: focus_set.cloned(),
                    styling: overrides.wire_styling.map(WireStyling::to_owned_styling),
                    designators: overrides.designators.cloned(),
                },
                scene,
            });
        }
        self.entry.as_ref().and_then(|entry| entry.scene.as_ref())
    }
}

impl BuiltFor {
    /// Whether the wire styling this scene was built with equals `now`; no
    /// styling and empty styling are the same thing.
    fn styling_matches(&self, now: Option<WireStyling<'_>>) -> bool {
        match &self.styling {
            Some(built) => built.matches(now),
            None => now.is_none_or(|s| s.color_overrides.is_empty() && s.class_styles.is_empty()),
        }
    }

    fn fits(
        &self,
        transform: &ScreenTransform,
        colors: &CanvasColors,
        viewport: iced::Size,
        focus_set: Option<&HashSet<uuid::Uuid>>,
        overrides: SceneOverrides<'_>,
    ) -> bool {
        if (self.scale - transform.scale).abs() >= 0.0001
            || self.viewport != viewport
            || self.colors != *colors
            || self.focus.as_ref() != focus_set
            || !self.styling_matches(overrides.wire_styling)
            || self.designators.as_ref() != overrides.designators
        {
            return false;
        }
        // The visible window, expressed in the screen space the scene was
        // built in, must stay inside the built (inflated) rectangle.
        let dx = self.offset.0 - transform.offset_x;
        let dy = self.offset.1 - transform.offset_y;
        dx.abs() <= viewport.width && dy.abs() <= viewport.height
    }
}
