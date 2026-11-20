//! Copper to board edge: a track running beside the board outline, with
//! the dashed keep-back zone and the gap arrow between them.

use iced::{Point, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::ids;
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::{Anchor, Tint, Weight};

use super::scene::{BoardInk, Handle, NoteText, Scene, VIEW_H, Values, rect};

const NOMINAL_GAP_MM: f64 = 0.3;
/// The gap takes about this many view units.
const GAP_UNITS: f64 = 120.0;
const MIN_SCALE: f64 = 40.0;
const MAX_SCALE: f64 = 400.0;
/// The board outline, and the track width in millimetres.
const EDGE_X: f32 = 470.0;
const TRACK_MM: f64 = 0.25;
const ARROW_Y: f32 = 160.0;
const LABEL_Y: f32 = 70.0;

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let field = LimitField::MinEdgeClearance;
    let gap = values.size_or(field, NOMINAL_GAP_MM);
    let scale = (GAP_UNITS / gap).clamp(MIN_SCALE, MAX_SCALE);
    let gap_units = gap as f32 * scale as f32;
    let track_w = (TRACK_MM as f32 * scale as f32).max(6.0);
    let track_right = EDGE_X - gap_units;

    scene.outline_rect(
        rect(track_right, 0.0, gap_units, VIEW_H),
        0.0,
        ink.dimension,
        true,
        Some(Tint::Wash),
    );
    scene.solid_rect(
        rect(track_right - track_w, 0.0, track_w, VIEW_H),
        0.0,
        ink.copper,
    );
    scene.line(
        Point::new(EDGE_X, 0.0),
        Point::new(EDGE_X, VIEW_H),
        ink.edge,
        Weight::Heavy,
    );
    scene.dimension(
        Point::new(track_right, ARROW_Y),
        Point::new(EDGE_X, ARROW_Y),
    );
    let mid = track_right + gap_units / 2.0;
    scene.extension(
        Point::new(mid, ARROW_Y - 4.0),
        Point::new(mid, LABEL_Y + 10.0),
    );
    scene.value(
        Some(field),
        ids::RULES_PREVIEW_CAP_EDGE,
        values.get(field),
        Point::new(mid, LABEL_Y),
        LabelAnchor::Middle,
        Some(Handle {
            at: Point::new(track_right, ARROW_Y),
            axis: Vector::new(-1.0, 0.0),
            mm_per_unit: 1.0 / scale,
        }),
    );
    scene.note(
        Point::new(EDGE_X + 10.0, VIEW_H - 24.0),
        NoteText::Id(ids::RULES_PREVIEW_OBJECT_EDGE),
        Anchor::Start,
        ink.edge,
    );
    scene.note(
        Point::new(track_right - track_w - 10.0, VIEW_H - 24.0),
        NoteText::Id(ids::RULES_PREVIEW_OBJECT_TRACK),
        Anchor::End,
        ink.note,
    );
    scene.title(ids::RULES_PREVIEW_TITLE_EDGE, ids::RULES_PREVIEW_NOTE_EDGE);
    scene
}
