//! Component height and 3D body clearance (#2939): two parts seen from the
//! side, standing on the board, with the height arrow up the tall one and
//! the gap arrow between their bodies. Only the height and the gap follow
//! the values.

use iced::{Point, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::ids;
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::{Tint, Weight};

use super::scene::{BoardInk, Handle, Scene, Values, rect};

const NOMINAL_HEIGHT_MM: f64 = 5.0;
const NOMINAL_GAP_MM: f64 = 0.25;
/// View units per millimetre, and the bounds each length is kept in.
const HEIGHT_SCALE: f64 = 30.0;
const HEIGHT_UNITS: (f64, f64) = (60.0, 180.0);
const GAP_SCALE: f64 = 320.0;
const GAP_UNITS: (f64, f64) = (16.0, 140.0);
const CENTRE_X: f32 = 340.0;
const BOARD_Y: f32 = 250.0;
const PART_W: f32 = 170.0;
/// The short part's height, view units: not to scale.
const SHORT_H: f32 = 60.0;

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let height_field = LimitField::MaxHeight;
    let gap_field = LimitField::MinBodyClearance;
    let height = values.size_or(height_field, NOMINAL_HEIGHT_MM);
    let height_units = (height * HEIGHT_SCALE).clamp(HEIGHT_UNITS.0, HEIGHT_UNITS.1);
    let gap = values.size_or(gap_field, NOMINAL_GAP_MM);
    let gap_units = (gap * GAP_SCALE).clamp(GAP_UNITS.0, GAP_UNITS.1);
    let (h, g) = (height_units as f32, gap_units as f32);
    let left_x = CENTRE_X - g / 2.0 - PART_W;
    let right_x = CENTRE_X + g / 2.0;
    scene.line(
        Point::new(40.0, BOARD_Y),
        Point::new(600.0, BOARD_Y),
        ink.edge,
        Weight::Heavy,
    );
    scene.outline_rect(
        rect(left_x, BOARD_Y - h, PART_W, h),
        2.0,
        ink.note,
        false,
        Some(Tint::Light),
    );
    scene.outline_rect(
        rect(right_x, BOARD_Y - SHORT_H, PART_W, SHORT_H),
        2.0,
        ink.note,
        false,
        Some(Tint::Light),
    );
    // The height, up the tall part.
    let arrow_x = left_x - 24.0;
    let top = Point::new(arrow_x, BOARD_Y - h);
    scene.dimension(Point::new(arrow_x, BOARD_Y), top);
    scene.value(
        Some(height_field),
        ids::RULES_PREVIEW_CAP_MAX,
        values.get(height_field),
        Point::new(arrow_x - 8.0, BOARD_Y - h / 2.0),
        LabelAnchor::End,
        Some(Handle {
            at: top,
            axis: Vector::new(0.0, -1.0),
            mm_per_unit: height / height_units,
        }),
    );
    // The gap, between the bodies.
    let gap_y = BOARD_Y - SHORT_H / 2.0;
    let to = Point::new(right_x, gap_y);
    scene.dimension(Point::new(CENTRE_X - g / 2.0, gap_y), to);
    let label = Point::new(right_x + PART_W / 2.0, BOARD_Y - SHORT_H - 40.0);
    scene.extension(Point::new(CENTRE_X, gap_y - 4.0), label);
    scene.value(
        Some(gap_field),
        ids::RULES_PREVIEW_CAP_GAP,
        values.get(gap_field),
        label,
        LabelAnchor::Middle,
        Some(Handle {
            at: to,
            axis: Vector::new(1.0, 0.0),
            mm_per_unit: 2.0 * gap / gap_units,
        }),
    );
    scene.title(
        ids::RULES_PREVIEW_TITLE_COMPONENT,
        ids::RULES_PREVIEW_NOTE_COMPONENT,
    );
    scene
}
