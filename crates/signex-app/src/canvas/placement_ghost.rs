//! The ghost of content attached to the cursor (a library part waiting for
//! its click).
//!
//! The fragment is drawn through the same scene builder as the sheet, so the
//! ghost looks like what the click will place. The scene is camera
//! independent in position: it is built once per zoom step with the fragment
//! origin at the screen origin, and every frame it is replayed shifted by
//! the cursor's anchor ([`PlacementGhost::transform_at`]).

use std::cell::RefCell;

use iced::Rectangle;
use iced::widget::canvas;
use signex_engine::PlacementPayload;
use signex_gfx::scene::Scene;
use signex_types::schematic::SchematicSheet;
use signex_types::theme::CanvasColors;

use crate::schematic_runtime::{ScreenTransform, build_schematic_scene, draw_built_scene};

/// A payload with more items than this is drawn as its bounding box only: a
/// full scene of a large paste would be rebuilt at every zoom step for a ghost
/// that is only there to be aimed.
pub const MAX_GHOST_SCENE_ITEMS: usize = 60;

/// Half-extent (px) of the cull rectangle the ghost scene is built in. A
/// fragment is a part or a pasted selection: far inside this at any zoom.
const GHOST_BUILD_HALF_EXTENT_PX: f32 = 1.0e6;

/// Where a click at `world` lands, and so where the ghost is drawn: the
/// cursor snapped to the grid when snapping is on. The single rule the draw
/// path and the commit path share.
#[must_use]
pub fn ghost_anchor(world: (f64, f64), snap_enabled: bool, grid_mm: f64) -> (f64, f64) {
    if snap_enabled && grid_mm > 0.0 {
        (
            (world.0 / grid_mm).round() * grid_mm,
            (world.1 / grid_mm).round() * grid_mm,
        )
    } else {
        world
    }
}

/// The ghost to draw this frame: none while placement is paused (the Tab form
/// is open and a click cannot commit), like the other cursor ghosts.
#[must_use]
pub fn visible_ghost(ghost: Option<&PlacementGhost>, paused: bool) -> Option<&PlacementGhost> {
    ghost.filter(|_| !paused)
}

/// What a built ghost scene depends on: stroke and glyph sizes follow the
/// zoom, and every colour comes from the theme. The fragment is fixed for the
/// life of a ghost, so it is not part of the key.
#[derive(Clone, Copy, PartialEq)]
struct BuiltFor {
    scale: f32,
    colors: CanvasColors,
}

/// A fragment to draw at the cursor, with its scene cached per zoom step.
pub struct PlacementGhost {
    fragment: SchematicSheet,
    /// `Some` when the payload is too large to draw item by item: the box
    /// drawn in its place, in fragment coordinates.
    bounding_box: Option<signex_types::schematic::Aabb>,
    /// What the scene was built for and the scene; `None` scene = draws
    /// nothing.
    built: RefCell<Option<(BuiltFor, Option<Scene>)>>,
}

impl PlacementGhost {
    #[must_use]
    pub fn new(payload: &PlacementPayload) -> Self {
        let bounding_box = if payload.item_count() > MAX_GHOST_SCENE_ITEMS {
            payload.bounds()
        } else {
            None
        };
        Self {
            fragment: payload.fragment().clone(),
            bounding_box,
            built: RefCell::new(None),
        }
    }

    /// Whether this ghost is drawn as a box instead of its items.
    #[must_use]
    pub fn is_bounding_box_only(&self) -> bool {
        self.bounding_box.is_some()
    }

    /// The camera transform shifted so the fragment origin lands on `anchor`
    /// (world mm): `offset += anchor * scale`.
    #[must_use]
    pub fn transform_at(camera: &ScreenTransform, anchor: (f64, f64)) -> ScreenTransform {
        ScreenTransform {
            offset_x: camera.offset_x + anchor.0 as f32 * camera.scale,
            offset_y: camera.offset_y + anchor.1 as f32 * camera.scale,
            scale: camera.scale,
        }
    }

    /// Draw the ghost with its origin at `anchor`.
    pub fn draw(
        &self,
        frame: &mut canvas::Frame,
        camera: &ScreenTransform,
        anchor: (f64, f64),
        colors: &CanvasColors,
    ) {
        if let Some(bounds) = &self.bounding_box {
            let at = Self::transform_at(camera, anchor);
            let top_left = at.world_to_screen((bounds.min_x, bounds.min_y));
            let bottom_right = at.world_to_screen((bounds.max_x, bounds.max_y));
            let outline = canvas::Path::rectangle(
                top_left,
                iced::Size::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y),
            );
            frame.stroke(
                &outline,
                canvas::Stroke::default()
                    .with_color(crate::render_config::to_iced(&colors.cursor))
                    .with_width(2.0),
            );
            return;
        }
        let mut built = self.built.borrow_mut();
        let key = BuiltFor {
            scale: camera.scale,
            colors: *colors,
        };
        if built.as_ref().is_none_or(|(built_for, _)| {
            (built_for.scale - key.scale).abs() >= 0.0001 || built_for.colors != key.colors
        }) {
            let origin = ScreenTransform {
                offset_x: 0.0,
                offset_y: 0.0,
                scale: camera.scale,
            };
            let reach = GHOST_BUILD_HALF_EXTENT_PX;
            let bounds = Rectangle::new(
                iced::Point::new(-reach, -reach),
                iced::Size::new(2.0 * reach, 2.0 * reach),
            );
            let scene = build_schematic_scene(
                &self.fragment,
                &origin,
                colors,
                bounds,
                None,
                Default::default(),
            );
            *built = Some((key, scene));
        }
        if let Some((_, Some(scene))) = built.as_ref() {
            draw_built_scene(frame, scene, &Self::transform_at(camera, anchor));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_ghost_is_drawn_while_placement_is_paused() {
        let ghost = PlacementGhost::new(&PlacementPayload::new(SchematicSheet::default()));
        assert!(visible_ghost(Some(&ghost), false).is_some());
        assert!(visible_ghost(Some(&ghost), true).is_none());
        assert!(visible_ghost(None, false).is_none());
    }

    #[test]
    fn the_anchor_snaps_to_the_grid_only_when_snapping_is_on() {
        assert_eq!(ghost_anchor((1.3, 2.6), true, 1.27), (1.27, 2.54));
        assert_eq!(ghost_anchor((1.3, 2.6), false, 1.27), (1.3, 2.6));
        assert_eq!(ghost_anchor((1.3, 2.6), true, 0.0), (1.3, 2.6));
    }

    #[test]
    fn the_shifted_transform_puts_the_fragment_origin_on_the_anchor() {
        let camera = ScreenTransform {
            offset_x: 40.0,
            offset_y: -10.0,
            scale: 4.0,
        };
        let anchor = (2.54, 5.08);
        let shifted = PlacementGhost::transform_at(&camera, anchor);
        let drawn_origin = shifted.world_to_screen((0.0, 0.0));
        let anchor_on_screen = camera.world_to_screen(anchor);
        assert!((drawn_origin.x - anchor_on_screen.x).abs() < 1e-4);
        assert!((drawn_origin.y - anchor_on_screen.y).abs() < 1e-4);
    }
}
