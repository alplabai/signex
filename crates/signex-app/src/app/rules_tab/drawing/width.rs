//! Track width: the narrowest track solid, the widest as a dashed band,
//! each with a width arrow and a handle on its top edge.

use iced::{Point, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::{MessageId, ids};
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::Tint;

use super::scene::{BoardInk, Handle, Scene, Values, rect};

const NOMINAL_MIN_MM: f64 = 0.15;
const NOMINAL_MAX_MM: f64 = 1.0;
/// The widest track takes this many view units.
const WIDEST_UNITS: f64 = 150.0;
const MAX_SCALE: f64 = 700.0;
const CENTRE_Y: f32 = 150.0;
const TRACK_X: (f32, f32) = (60.0, 580.0);
const MIN_ARROW_X: f32 = 200.0;
const MAX_ARROW_X: f32 = 440.0;
/// Labels sit this far under the widest band, and no lower than the floor.
const LABEL_GAP: f32 = 26.0;
const LABEL_FLOOR: f32 = 284.0;

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let min = values.size_or(LimitField::MinWidth, NOMINAL_MIN_MM);
    let max = values
        .size_or(LimitField::MaxWidth, NOMINAL_MAX_MM)
        .max(min);
    let scale = (WIDEST_UNITS / max).min(MAX_SCALE);
    let band = |width: f64| {
        let height = ((width * scale) as f32).max(1.0);
        (
            rect(
                TRACK_X.0,
                CENTRE_Y - height / 2.0,
                TRACK_X.1 - TRACK_X.0,
                height,
            ),
            height,
        )
    };
    let label_y = (CENTRE_Y + (max * scale) as f32 / 2.0 + LABEL_GAP).min(LABEL_FLOOR);
    if values.get(LimitField::MaxWidth).is_some() {
        let (r, height) = band(max);
        scene.outline_rect(r, height / 2.0, ink.note, true, Some(Tint::Light));
    }
    let (r, height) = band(min);
    scene.solid_rect(r, height / 2.0, ink.copper);
    let arrows = [
        Arrow {
            x: MIN_ARROW_X,
            field: LimitField::MinWidth,
            width: min,
            caption: ids::RULES_PREVIEW_CAP_MIN,
        },
        Arrow {
            x: MAX_ARROW_X,
            field: LimitField::MaxWidth,
            width: max,
            caption: ids::RULES_PREVIEW_CAP_MAX,
        },
    ];
    for spec in arrows {
        arrow(&mut scene, values, spec, scale, label_y);
    }
    scene.title(
        ids::RULES_PREVIEW_TITLE_TRACK,
        ids::RULES_PREVIEW_NOTE_TRACK,
    );
    scene
}

/// One width arrow: where it stands, the column it writes and the width
/// it spans.
struct Arrow {
    x: f32,
    field: LimitField,
    width: f64,
    caption: MessageId,
}

/// A vertical width arrow, its label below the band and a handle on the
/// top edge; dragging it up widens the track on both sides.
fn arrow(scene: &mut Scene, values: &Values, spec: Arrow, scale: f64, label_y: f32) {
    let Arrow {
        x,
        field,
        width,
        caption,
    } = spec;
    let half = (width * scale) as f32 / 2.0;
    let top = Point::new(x, CENTRE_Y - half);
    scene.dimension(top, Point::new(x, CENTRE_Y + half));
    let handle = Handle {
        at: top,
        axis: Vector::new(0.0, -1.0),
        mm_per_unit: 2.0 / scale,
    };
    scene.value(
        Some(field),
        caption,
        values.get(field),
        Point::new(x, label_y),
        LabelAnchor::Middle,
        Some(handle),
    );
}
