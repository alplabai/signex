//! Selection halos while a move-drag is under way: lowered and tessellated
//! once, then drawn translated by the drag delta. Rules in
//! `docs/RENDERING_RULES.md`, "sch-halo-reuse".

use iced::Vector;
use signex_types::schematic::{Aabb, SchematicSheet, SelectedItem};

use super::hit_cache::SheetKey;
use super::*;
use crate::schematic_runtime::selection::{draw_selection_boxes, selection_boxes};

/// How far, in mm, a box may differ from a pure shift and still count as one.
const SHIFT_EPS_MM: f64 = 1e-3;

/// The halo layers of one drag frame.
pub(super) struct DragSelection {
    /// The cached halos of the items that only moved, and the screen shift to
    /// draw them under.
    pub halos: Option<(canvas::Geometry, Vector)>,
    /// Everything else of the selection layer, drawn this frame.
    pub live: canvas::Geometry,
}

/// What the cached halo geometry was lowered from.
#[derive(PartialEq)]
struct HaloKey {
    /// The base boxes (document, selection) the geometry holds.
    base: u64,
    /// Which selected items the cached geometry holds.
    shifted: Vec<bool>,
    camera: (f32, f32, f32),
    device_px_per_logical_px: f32,
    colors: signex_types::theme::CanvasColors,
    graphite: Option<GraphiteCanvasInk>,
}

/// The selected boxes before the drag moved anything.
struct BaseBoxes {
    generation: u64,
    sheet: SheetKey,
    selected: Vec<SelectedItem>,
    boxes: Vec<Option<Aabb>>,
}

#[derive(Default)]
pub(super) struct HaloCache {
    base: Option<BaseBoxes>,
    generations: u64,
    key: Option<HaloKey>,
    cache: canvas::Cache,
    /// Test-only: how often the halos were lowered and tessellated.
    #[cfg(test)]
    pub tessellations: std::cell::Cell<u32>,
}

impl HaloCache {
    pub(super) fn clear(&mut self) {
        self.base = None;
        self.key = None;
        self.cache.clear();
    }
}

/// How a drag frame's boxes relate to the ones before the drag.
#[derive(Debug, PartialEq)]
pub(super) struct ShiftPlan {
    /// The translation the items that moved rigidly share, in mm.
    pub shift: (f64, f64),
    /// Per selected item: moved by exactly `shift` with its size unchanged.
    /// An item that changed shape, held still or moved on its own is `false`.
    pub shifted: Vec<bool>,
}

/// The shared shift of the boxes in `now` against `base`, and which items took
/// it. The shift is the first rigid move that is not zero, so a locked item
/// held in place at the front of the selection does not decide it.
pub(super) fn shift_plan(base: &[Option<Aabb>], now: &[Option<Aabb>]) -> ShiftPlan {
    let step = |base: &Option<Aabb>, now: &Option<Aabb>| -> Option<(f64, f64)> {
        let (a, b) = (base.as_ref()?, now.as_ref()?);
        let same_size = ((b.max_x - b.min_x) - (a.max_x - a.min_x)).abs() < SHIFT_EPS_MM
            && ((b.max_y - b.min_y) - (a.max_y - a.min_y)).abs() < SHIFT_EPS_MM;
        same_size.then_some((b.min_x - a.min_x, b.min_y - a.min_y))
    };
    let steps: Vec<Option<(f64, f64)>> = base.iter().zip(now).map(|(a, b)| step(a, b)).collect();
    let shift = steps
        .iter()
        .flatten()
        .find(|(dx, dy)| dx.abs() >= SHIFT_EPS_MM || dy.abs() >= SHIFT_EPS_MM)
        .copied()
        .unwrap_or((0.0, 0.0));
    let shifted = steps
        .iter()
        .map(|s| {
            s.is_some_and(|(dx, dy)| {
                (dx - shift.0).abs() < SHIFT_EPS_MM && (dy - shift.1).abs() < SHIFT_EPS_MM
            })
        })
        .collect();
    ShiftPlan { shift, shifted }
}

impl CanvasSlot {
    /// Test-only: how often the drag halos were lowered and tessellated.
    #[cfg(test)]
    pub(super) fn halo_tessellations(&self) -> u32 {
        self.halo_cache.borrow().tessellations.get()
    }
}

impl SchematicCanvas<'_> {
    /// The selection layer of a drag frame: the halos of the items that moved
    /// rigidly from a cache lowered once, the rest drawn as they stand in
    /// `moving`. `None` when the document revision is unknown, which the
    /// caller draws the whole layer for.
    pub(in crate::canvas) fn draw_drag_selection(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        moving: &SchematicSheet,
    ) -> Option<DragSelection> {
        let base_sheet = self.active_snapshot()?;
        let revision = self.render_cache.as_ref().map_or(0, |c| c.revision());
        if revision == 0 {
            return None;
        }
        let cam = self.camera();
        let widths = self.prefs.wire_widths();
        let class_revision = self.prefs.wire_class_revision;
        let now = selection_boxes(moving, &self.selected, &self.designators, widths);

        let mut halos = self.halo_cache.borrow_mut();
        let base_is_current = halos.base.as_ref().is_some_and(|b| {
            b.selected == self.selected
                && b.sheet.matches(
                    revision,
                    class_revision,
                    &widths.defaults,
                    &self.designators,
                )
        });
        if !base_is_current {
            halos.generations += 1;
            halos.base = Some(BaseBoxes {
                generation: halos.generations,
                sheet: SheetKey::new(revision, class_revision, widths.defaults, &self.designators),
                selected: self.selected.clone(),
                boxes: selection_boxes(base_sheet, &self.selected, &self.designators, widths),
            });
        }
        let base_boxes = halos.base.as_ref().expect("base boxes were just set");
        let (generation, base) = (base_boxes.generation, &base_boxes.boxes);
        let plan = shift_plan(base, &now);
        let cached: Vec<Aabb> = base
            .iter()
            .zip(&plan.shifted)
            .filter_map(|(b, &shifted)| b.filter(|_| shifted))
            .collect();
        let live: Vec<Aabb> = now
            .iter()
            .zip(&plan.shifted)
            .filter_map(|(b, &shifted)| b.filter(|_| !shifted))
            .collect();

        let transform = crate::schematic_runtime::ScreenTransform {
            offset_x: cam.offset.x,
            offset_y: cam.offset.y,
            scale: cam.scale,
            device_px_per_logical_px: self.prefs.device_px_per_logical_px,
        };
        let colors = &self.prefs.canvas_colors;
        let graphite = self.prefs.graphite.as_ref();
        let key = HaloKey {
            base: generation,
            shifted: plan.shifted,
            camera: (cam.offset.x, cam.offset.y, cam.scale),
            device_px_per_logical_px: self.prefs.device_px_per_logical_px,
            colors: *colors,
            graphite: graphite.copied(),
        };
        if halos.key.as_ref() != Some(&key) {
            halos.cache.clear();
            halos.key = Some(key);
        }
        let halo_geometry = (!cached.is_empty()).then(|| {
            let geometry = halos.cache.draw(renderer, bounds.size(), |frame| {
                #[cfg(test)]
                halos.tessellations.set(halos.tessellations.get() + 1);
                draw_selection_boxes(frame, &cached, &transform, colors, graphite);
            });
            let scale = f64::from(cam.scale);
            let shift = Vector::new((plan.shift.0 * scale) as f32, (plan.shift.1 * scale) as f32);
            (geometry, shift)
        });

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        draw_selection_boxes(&mut frame, &live, &transform, colors, graphite);
        self.draw_selection_extras(&mut frame, moving, bounds, &transform);
        Some(DragSelection {
            halos: halo_geometry,
            live: frame.into_geometry(),
        })
    }
}
