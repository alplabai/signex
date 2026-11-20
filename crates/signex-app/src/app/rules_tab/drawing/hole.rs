//! Holes: the allowed hole range on the left (smallest and largest drill),
//! and on the right two plated holes showing the annular ring and the
//! hole-to-hole spacing.

use iced::{Point, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::ids;
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::Tint;

use super::scene::{BoardInk, Handle, Scene, Values};

const NOMINAL_MIN_MM: f64 = 0.2;
const NOMINAL_MAX_MM: f64 = 3.2;
const NOMINAL_RING_MM: f64 = 0.13;
const NOMINAL_SPACING_MM: f64 = 0.25;
/// The largest hole takes this many view units across.
const RANGE_UNITS: f64 = 160.0;
const RANGE_MAX_SCALE: f64 = 1200.0;
const RANGE_AT: Point = Point::new(150.0, 180.0);
/// The pair of holes fits this many view units.
const PAIR_UNITS: f64 = 250.0;
const PAIR_MAX_SCALE: f64 = 900.0;
const PAIR_AT: Point = Point::new(485.0, 180.0);
const ARROW_OFFSET: f32 = 12.0;
const LABEL_OFFSET: f32 = 28.0;

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    range(&mut scene, values);
    pair(&mut scene, values);
    scene.title(ids::RULES_PREVIEW_TITLE_HOLE, ids::RULES_PREVIEW_NOTE_HOLE);
    scene
}

/// The smallest hole inside the dashed circle of the largest.
fn range(scene: &mut Scene, values: &Values) {
    let ink = scene.ink;
    let min = values.size_or(LimitField::MinHole, NOMINAL_MIN_MM);
    let max = values.size_or(LimitField::MaxHole, NOMINAL_MAX_MM).max(min);
    let scale = (RANGE_UNITS / max).min(RANGE_MAX_SCALE);
    let c = RANGE_AT;
    let max_r = max as f32 * scale as f32 / 2.0;
    let min_r = (min as f32 * scale as f32 / 2.0).max(1.0);
    scene.outline_circle(c, max_r, ink.note, true, Some(Tint::Wash));
    scene.plated_hole(c, min_r);
    scene.outline_circle(c, min_r, ink.silk, true, None);

    let y = c.y - max_r - ARROW_OFFSET;
    scene.dimension(Point::new(c.x - max_r, y), Point::new(c.x + max_r, y));
    scene.value(
        Some(LimitField::MaxHole),
        ids::RULES_PREVIEW_CAP_MAX_HOLE,
        values.get(LimitField::MaxHole),
        Point::new(c.x, y - LABEL_OFFSET + ARROW_OFFSET),
        LabelAnchor::Middle,
        Some(Handle {
            at: Point::new(c.x + max_r, c.y),
            axis: Vector::new(1.0, 0.0),
            mm_per_unit: 2.0 / scale,
        }),
    );
    let tip = Point::new(c.x + min_r, c.y);
    let label = Point::new(c.x + max_r * 0.4, c.y + max_r * 0.5 + LABEL_OFFSET);
    scene.extension(tip, label);
    scene.value(
        Some(LimitField::MinHole),
        ids::RULES_PREVIEW_CAP_MIN_HOLE,
        values.get(LimitField::MinHole),
        label,
        LabelAnchor::Start,
        None,
    );
}

/// Two plated holes of the smallest size: the ring on the left one, the
/// spacing between their edges.
fn pair(scene: &mut Scene, values: &Values) {
    let ink = scene.ink;
    let hole = values.size_or(LimitField::MinHole, NOMINAL_MIN_MM).max(0.3);
    let ring = values.size_or(LimitField::MinAnnular, NOMINAL_RING_MM);
    let spacing = values.size_or(LimitField::MinHoleSpacing, NOMINAL_SPACING_MM);
    let pad = hole + 2.0 * ring;
    let pitch = hole + spacing;
    let scale = (PAIR_UNITS / (pitch + pad)).min(PAIR_MAX_SCALE);
    let s = scale as f32;
    let (hole_r, pad_r) = (hole as f32 * s / 2.0, pad as f32 * s / 2.0);
    let c1 = Point::new(PAIR_AT.x - pitch as f32 * s / 2.0, PAIR_AT.y);
    let c2 = Point::new(PAIR_AT.x + pitch as f32 * s / 2.0, PAIR_AT.y);
    for c in [c1, c2] {
        scene.solid_circle(c, pad_r, ink.copper);
        scene.plated_hole(c, hole_r);
    }

    // Ring: hole edge to pad edge, on the left of the left hole.
    let (from, to) = (
        Point::new(c1.x - hole_r, c1.y),
        Point::new(c1.x - pad_r, c1.y),
    );
    scene.dimension(from, to);
    let ring_x = (from.x + to.x) / 2.0;
    let ring_label = Point::new(ring_x, c1.y + pad_r + LABEL_OFFSET);
    scene.extension(
        Point::new(ring_x, c1.y + 4.0),
        Point::new(ring_x, ring_label.y - 8.0),
    );
    scene.value(
        Some(LimitField::MinAnnular),
        ids::RULES_PREVIEW_CAP_ANNULAR,
        values.get(LimitField::MinAnnular),
        ring_label,
        LabelAnchor::Middle,
        Some(Handle {
            at: to,
            axis: Vector::new(-1.0, 0.0),
            mm_per_unit: 1.0 / scale,
        }),
    );

    // Spacing: hole edge to hole edge, labelled above the pair.
    let (from, to) = (
        Point::new(c1.x + hole_r, c1.y),
        Point::new(c2.x - hole_r, c2.y),
    );
    scene.dimension(from, to);
    let mid = (from.x + to.x) / 2.0;
    let label = Point::new(mid, c1.y - pad_r - LABEL_OFFSET);
    scene.extension(Point::new(mid, c1.y - 4.0), Point::new(mid, label.y + 8.0));
    scene.value(
        Some(LimitField::MinHoleSpacing),
        ids::RULES_PREVIEW_CAP_HOLE_SPACING,
        values.get(LimitField::MinHoleSpacing),
        label,
        LabelAnchor::Middle,
        Some(Handle {
            at: to,
            axis: Vector::new(1.0, 0.0),
            mm_per_unit: 1.0 / scale,
        }),
    );
}
