//! Courtyard spacing: two parts side by side, each inside its dashed
//! courtyard, with the gap arrow between the courtyards. The parts are not
//! to scale; only the gap follows the value.

use iced::{Point, Rectangle, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::ids;
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::{Tint, Weight};

use super::scene::{BoardInk, Handle, Scene, Values, rect};

const NOMINAL_GAP_MM: f64 = 0.25;
/// View units per millimetre of gap, and the bounds the gap is kept in.
const GAP_SCALE: f64 = 320.0;
const GAP_UNITS: (f64, f64) = (16.0, 180.0);
const CENTRE: Point = Point::new(320.0, 165.0);
/// One courtyard, in view units, and the pads inside it.
const COURTYARD: (f32, f32) = (190.0, 130.0);
const PAD: (f32, f32) = (40.0, 64.0);
const LABEL_Y: f32 = 70.0;

/// The courtyard `r` with a part body and two pads inside.
fn part(scene: &mut Scene, r: Rectangle) {
    let ink = scene.ink;
    let body = super::scene::grown(r, -18.0);
    scene.outline_rect(body, 2.0, ink.note, false, Some(Tint::Light));
    let y = r.center_y() - PAD.1 / 2.0;
    for x in [body.x + 8.0, body.x + body.width - 8.0 - PAD.0] {
        scene.solid_rect(rect(x, y, PAD.0, PAD.1), 3.0, ink.copper);
    }
    scene.outline_rect(r, 0.0, ink.courtyard, true, None);
}

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let field = LimitField::MinCourtyardSpacing;
    let gap = values.size_or(field, NOMINAL_GAP_MM);
    let gap_units = (gap * GAP_SCALE).clamp(GAP_UNITS.0, GAP_UNITS.1);
    let mm_per_unit = gap / gap_units;
    let g = gap_units as f32;
    let (w, h) = COURTYARD;
    let top = CENTRE.y - h / 2.0;
    let left = rect(CENTRE.x - g / 2.0 - w, top, w, h);
    let right = rect(CENTRE.x + g / 2.0, top, w, h);
    part(&mut scene, left);
    part(&mut scene, right);
    scene.outline_rect(
        rect(CENTRE.x - g / 2.0, top, g, h),
        0.0,
        ink.dimension,
        true,
        Some(Tint::Wash),
    );
    let (from, to) = (
        Point::new(CENTRE.x - g / 2.0, CENTRE.y),
        Point::new(CENTRE.x + g / 2.0, CENTRE.y),
    );
    scene.dimension(from, to);
    scene.line(
        Point::new(CENTRE.x, top - 4.0),
        Point::new(CENTRE.x, LABEL_Y + 10.0),
        ink.dimension,
        Weight::Hairline,
    );
    scene.value(
        Some(field),
        ids::RULES_PREVIEW_CAP_GAP,
        values.get(field),
        Point::new(CENTRE.x, LABEL_Y),
        LabelAnchor::Middle,
        Some(Handle {
            at: to,
            axis: Vector::new(1.0, 0.0),
            mm_per_unit: 2.0 * mm_per_unit,
        }),
    );
    scene.title(
        ids::RULES_PREVIEW_TITLE_COURTYARD,
        ids::RULES_PREVIEW_NOTE_COURTYARD,
    );
    scene
}
