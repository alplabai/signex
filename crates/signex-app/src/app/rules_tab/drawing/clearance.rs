//! Copper clearance: two objects of different nets at the gap, with the
//! dashed clearance zone around the first and a gap arrow between them.
//! The clearance rule, a class pair and an object pair all draw this.

use iced::{Point, Rectangle, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::{MessageId, ids};
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::{Anchor, Tint};

use super::scene::{BoardInk, Handle, NoteText, Scene, Values, grown, rect};

/// The gap drawn while the rule sets none, in millimetres.
const NOMINAL_GAP_MM: f64 = 0.15;
/// The gap takes about this many view units at the nominal scale.
const GAP_UNITS: f64 = 48.0;
/// Scale bounds, in view units per millimetre.
const MIN_SCALE: f64 = 70.0;
const MAX_SCALE: f64 = 180.0;
const CENTRE: Point = Point::new(320.0, 155.0);
/// Where the gap label and the object names sit.
const LABEL_Y: f32 = 34.0;
const NAME_Y: f32 = 284.0;
const NAME_MARGIN: f32 = 60.0;

/// A copper object a clearance is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Object {
    Track,
    Pad,
    Via,
    Pour,
}

impl Object {
    /// The object a rule word names; an unknown word draws a track.
    pub fn parse(word: &str) -> Self {
        match word.trim().to_ascii_lowercase().as_str() {
            "pad" | "smd" | "th" => Object::Pad,
            "via" => Object::Via,
            "pour" | "zone" | "copper" | "plane" => Object::Pour,
            _ => Object::Track,
        }
    }

    pub fn name(self) -> MessageId {
        match self {
            Object::Track => ids::RULES_PREVIEW_OBJECT_TRACK,
            Object::Pad => ids::RULES_PREVIEW_OBJECT_PAD,
            Object::Via => ids::RULES_PREVIEW_OBJECT_VIA,
            Object::Pour => ids::RULES_PREVIEW_OBJECT_POUR,
        }
    }
}

/// Which side of the gap an object sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// The outline of one drawn object, for its clearance zone.
enum Shape {
    Rect(Rectangle, f32),
    Circle(Point, f32),
}

/// Draws `object` with its near edge on `edge`; returns its outline and
/// the x its name is centred on.
fn object(scene: &mut Scene, object: Object, scale: f32, edge: f32, side: Side) -> (Shape, f32) {
    let ink = scene.ink;
    let cy = CENTRE.y;
    let place = |width: f32| match side {
        Side::Left => edge - width,
        Side::Right => edge,
    };
    match object {
        Object::Track => {
            let width = (0.2 * scale).max(5.0);
            let r = rect(place(width), 0.0, width, super::scene::VIEW_H);
            scene.solid_rect(r, 0.0, ink.copper);
            (Shape::Rect(r, 0.0), r.center_x())
        }
        Object::Pad => {
            let (width, height) = (0.6 * scale, 1.0 * scale);
            let r = rect(place(width), cy - height / 2.0, width, height);
            let radius = 0.08 * scale;
            scene.solid_rect(r, radius, ink.copper);
            (Shape::Rect(r, radius), r.center_x())
        }
        Object::Via => {
            let radius = 0.25 * scale;
            let center = Point::new(place(2.0 * radius) + radius, cy);
            scene.solid_circle(center, radius, ink.copper);
            scene.plated_hole(center, radius / 2.0);
            (Shape::Circle(center, radius), center.x)
        }
        Object::Pour => {
            let (width, height) = (420.0, 230.0);
            let r = rect(place(width), cy - height / 2.0, width, height);
            let radius = 6.0;
            scene.solid_rect(r, radius, ink.copper);
            let name_x = match side {
                Side::Left => edge - 70.0,
                Side::Right => edge + 70.0,
            };
            (Shape::Rect(r, radius), name_x)
        }
    }
}

/// The dashed clearance zone: `shape` grown by the gap.
fn zone(scene: &mut Scene, shape: &Shape, gap: f32) {
    let color = scene.ink.dimension;
    match shape {
        Shape::Rect(r, radius) => {
            scene.outline_rect(grown(*r, gap), radius + gap, color, true, Some(Tint::Wash));
        }
        Shape::Circle(center, radius) => {
            scene.outline_circle(*center, radius + gap, color, true, Some(Tint::Wash));
        }
    }
}

/// Two objects at `gap`. `gap` is the value shown; the drawing uses a
/// nominal gap while it is unset or not positive.
pub(super) fn pair_scene(
    ink: BoardInk,
    objects: [Object; 2],
    gap: Option<f64>,
    field: Option<LimitField>,
    caption: MessageId,
) -> Scene {
    let mut scene = Scene::new(ink);
    let drawn = gap
        .filter(|g| g.is_finite() && *g > 0.0)
        .unwrap_or(NOMINAL_GAP_MM);
    let scale = (GAP_UNITS / drawn).clamp(MIN_SCALE, MAX_SCALE);
    let gap_units = (drawn * scale) as f32;
    let (left_edge, right_edge) = (CENTRE.x - gap_units / 2.0, CENTRE.x + gap_units / 2.0);
    let (left, left_x) = object(&mut scene, objects[0], scale as f32, left_edge, Side::Left);
    zone(&mut scene, &left, gap_units);
    let (_, right_x) = object(
        &mut scene,
        objects[1],
        scale as f32,
        right_edge,
        Side::Right,
    );
    scene.dimension(
        Point::new(left_edge, CENTRE.y),
        Point::new(right_edge, CENTRE.y),
    );
    scene.extension(
        Point::new(CENTRE.x, CENTRE.y - 6.0),
        Point::new(CENTRE.x, LABEL_Y + 10.0),
    );
    let handle = Handle {
        at: Point::new(right_edge, CENTRE.y),
        axis: Vector::new(1.0, 0.0),
        mm_per_unit: 2.0 / scale,
    };
    scene.value(
        field,
        caption,
        gap,
        Point::new(CENTRE.x, LABEL_Y),
        LabelAnchor::Middle,
        Some(handle),
    );
    let note = ink.note;
    for (x, object) in [(left_x, objects[0]), (right_x, objects[1])] {
        let at = Point::new(
            x.clamp(NAME_MARGIN, super::scene::VIEW_W - NAME_MARGIN),
            NAME_Y,
        );
        scene.note(at, NoteText::Id(object.name()), Anchor::Middle, note);
    }
    scene
}

/// A clearance rule: a track and a pad at its minimum clearance.
pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let field = LimitField::MinClearance;
    let mut scene = pair_scene(
        ink,
        [Object::Track, Object::Pad],
        values.get(field),
        Some(field),
        ids::RULES_PREVIEW_CAP_CLEARANCE,
    );
    scene.title(
        ids::RULES_PREVIEW_TITLE_CLEARANCE,
        ids::RULES_PREVIEW_NOTE_CLEARANCE,
    );
    scene
}
