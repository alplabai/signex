//! Via: the via pad with its drill, a neighbouring pad at the via-to-pad
//! gap, and the tented mask drawn over the via only.

use iced::{Point, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::ids;
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::Tint;

use super::scene::{BoardInk, Handle, Scene, Values, rect};

const NOMINAL_DIAMETER_MM: f64 = 0.45;
const NOMINAL_DRILL_MM: f64 = 0.2;
const NOMINAL_GAP_MM: f64 = 0.2;
/// The neighbouring pad, in millimetres.
const PAD_MM: (f64, f64) = (0.6, 1.0);
/// The pad is cut at this height, so its label stays in view.
const PAD_MAX_UNITS: f32 = 140.0;
/// The tented mask reaches this far past the via copper, in millimetres.
const MASK_MARGIN_MM: f64 = 0.05;
/// Via, gap and pad together take this many view units.
const SPAN_UNITS: f64 = 330.0;
const MAX_SCALE: f64 = 900.0;
const VIA_AT: Point = Point::new(200.0, 160.0);
/// Distance from an object to its arrow, and from the arrow to its label.
const ARROW_OFFSET: f32 = 30.0;
const LABEL_OFFSET: f32 = 18.0;

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let diameter = values.size_or(LimitField::MinDiameter, NOMINAL_DIAMETER_MM);
    let drill = values
        .size_or(LimitField::MinDrill, NOMINAL_DRILL_MM)
        .min(diameter * 0.9);
    let gap = values.size_or(LimitField::MinPadGap, NOMINAL_GAP_MM);
    let scale = (SPAN_UNITS / (diameter + gap + PAD_MM.0)).min(MAX_SCALE);
    let (c, s) = (VIA_AT, scale as f32);
    let via_r = diameter as f32 * s / 2.0;
    let drill_r = drill as f32 * s / 2.0;
    let gap_units = gap as f32 * s;

    let pad_x = c.x + via_r + gap_units;
    let pad_w = PAD_MM.0 as f32 * s;
    let pad_h = (PAD_MM.1 as f32 * s).min(PAD_MAX_UNITS);
    let pad = rect(pad_x, c.y - pad_h / 2.0, pad_w, pad_h);
    scene.outline_circle(c, via_r + gap_units, ink.dimension, true, Some(Tint::Wash));
    scene.solid_rect(pad, 0.08 * s, ink.copper);
    scene.solid_circle(c, via_r, ink.copper);
    scene.plated_hole(c, drill_r);
    let mask_r = via_r + MASK_MARGIN_MM as f32 * s;
    scene.outline_circle(c, mask_r, ink.mask, true, Some(Tint::Strong));

    // Diameter under the via.
    let y = c.y + via_r + ARROW_OFFSET;
    scene.extension(
        Point::new(c.x - via_r, c.y),
        Point::new(c.x - via_r, y + 4.0),
    );
    scene.extension(
        Point::new(c.x + via_r, c.y),
        Point::new(c.x + via_r, y + 4.0),
    );
    scene.dimension(Point::new(c.x - via_r, y), Point::new(c.x + via_r, y));
    scene.value(
        Some(LimitField::MinDiameter),
        ids::RULES_PREVIEW_CAP_DIAMETER,
        values.get(LimitField::MinDiameter),
        Point::new(c.x, y + LABEL_OFFSET),
        LabelAnchor::Middle,
        Some(Handle {
            at: Point::new(c.x + via_r, c.y),
            axis: Vector::new(1.0, 0.0),
            mm_per_unit: 2.0 / scale,
        }),
    );

    // Drill above it.
    let y = c.y - via_r - ARROW_OFFSET;
    scene.extension(
        Point::new(c.x - drill_r, c.y),
        Point::new(c.x - drill_r, y - 4.0),
    );
    scene.extension(
        Point::new(c.x + drill_r, c.y),
        Point::new(c.x + drill_r, y - 4.0),
    );
    scene.dimension(Point::new(c.x - drill_r, y), Point::new(c.x + drill_r, y));
    scene.value(
        Some(LimitField::MinDrill),
        ids::RULES_PREVIEW_CAP_DRILL,
        values.get(LimitField::MinDrill),
        Point::new(c.x, y - LABEL_OFFSET),
        LabelAnchor::Middle,
        Some(Handle {
            at: Point::new(c.x - drill_r, c.y),
            axis: Vector::new(-1.0, 0.0),
            mm_per_unit: 2.0 / scale,
        }),
    );

    // Via to pad, above the pad.
    let gap_from = Point::new(c.x + via_r, c.y);
    scene.dimension(gap_from, Point::new(pad_x, c.y));
    let label_x = (gap_from.x + pad_x) / 2.0;
    let label_y = c.y - pad_h / 2.0 - ARROW_OFFSET;
    scene.extension(
        Point::new(label_x, c.y - 4.0),
        Point::new(label_x, label_y + 8.0),
    );
    scene.value(
        Some(LimitField::MinPadGap),
        ids::RULES_PREVIEW_CAP_PAD_GAP,
        values.get(LimitField::MinPadGap),
        Point::new(label_x, label_y),
        LabelAnchor::Start,
        Some(Handle {
            at: Point::new(pad_x, c.y),
            axis: Vector::new(1.0, 0.0),
            mm_per_unit: 1.0 / scale,
        }),
    );
    scene.title(ids::RULES_PREVIEW_TITLE_VIA, ids::RULES_PREVIEW_NOTE_VIA);
    scene
}
