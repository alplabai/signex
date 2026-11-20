//! Silkscreen: a silk line above a pad at the silk-to-pad clearance, and a
//! second silk line above it at the silk-to-silk spacing.

use iced::{Point, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::ids;
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::Weight;

use super::scene::{BoardInk, Handle, Scene, Values, rect};

const NOMINAL_PAD_GAP_MM: f64 = 0.15;
const NOMINAL_SPACING_MM: f64 = 0.15;
/// The pad, in millimetres.
const PAD_MM: (f64, f64) = (1.2, 0.9);
/// The larger clearance takes about this many view units.
const GAP_UNITS: f64 = 60.0;
const MAX_SCALE: f64 = 150.0;
const PAD_CENTRE: Point = Point::new(320.0, 215.0);
/// How far the silk lines run past the pad, and where the arrows stand.
const LINE_OVERHANG: f32 = 110.0;
const ARROW_INSET: f32 = 60.0;
const LABEL_GAP: f32 = 10.0;

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let to_pad = values.size_or(LimitField::MinSilkPad, NOMINAL_PAD_GAP_MM);
    let spacing = values.size_or(LimitField::MinSilkSpacing, NOMINAL_SPACING_MM);
    let scale = (GAP_UNITS / to_pad.max(spacing)).min(MAX_SCALE);
    let s = scale as f32;
    let (pad_w, pad_h) = (PAD_MM.0 as f32 * s, PAD_MM.1 as f32 * s);
    let pad = rect(
        PAD_CENTRE.x - pad_w / 2.0,
        PAD_CENTRE.y - pad_h / 2.0,
        pad_w,
        pad_h,
    );
    scene.solid_rect(pad, 0.05 * s, ink.copper);
    let near = pad.y - to_pad as f32 * s;
    let far = near - spacing as f32 * s;
    let (x0, x1) = (pad.x - LINE_OVERHANG, pad.x + pad.width + LINE_OVERHANG);
    for y in [near, far] {
        scene.line(
            Point::new(x0, y),
            Point::new(x1, y),
            ink.silk,
            Weight::Heavy,
        );
    }

    // Silk to pad, right of the pad.
    let x = pad.x + pad.width + ARROW_INSET;
    scene.extension(
        Point::new(pad.x + pad.width, pad.y),
        Point::new(x + 6.0, pad.y),
    );
    scene.dimension(Point::new(x, near), Point::new(x, pad.y));
    scene.value(
        Some(LimitField::MinSilkPad),
        ids::RULES_PREVIEW_CAP_SILK_PAD,
        values.get(LimitField::MinSilkPad),
        Point::new(x + LABEL_GAP, (near + pad.y) / 2.0),
        LabelAnchor::Start,
        Some(Handle {
            at: Point::new(x, near),
            axis: Vector::new(0.0, -1.0),
            mm_per_unit: 1.0 / scale,
        }),
    );

    // Silk to silk, left of the pad.
    let x = pad.x - ARROW_INSET;
    scene.dimension(Point::new(x, far), Point::new(x, near));
    scene.value(
        Some(LimitField::MinSilkSpacing),
        ids::RULES_PREVIEW_CAP_SILK_SPACING,
        values.get(LimitField::MinSilkSpacing),
        Point::new(x - LABEL_GAP, (far + near) / 2.0),
        LabelAnchor::End,
        Some(Handle {
            at: Point::new(x, far),
            axis: Vector::new(0.0, -1.0),
            mm_per_unit: 1.0 / scale,
        }),
    );
    scene.title(ids::RULES_PREVIEW_TITLE_SILK, ids::RULES_PREVIEW_NOTE_SILK);
    scene
}
