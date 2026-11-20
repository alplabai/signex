//! Matrix pairs: a class pair drawn as two tracks at its gap, an object
//! pair as those two objects. Their gap label writes no rule column; the
//! caller writes it to the pinned matrix cell or object-pair row.

use iced::Point;
use signex_i18n::ids;
use signex_widgets::property_illustration::Anchor;

use super::clearance::{Object, pair_scene};
use super::scene::{BoardInk, NoteText, Scene};

/// Where the class names sit, over each track.
const CLASS_Y: f32 = 262.0;
const CLASS_OFFSET: f32 = 70.0;
const CENTRE_X: f32 = 320.0;

pub(super) fn class_pair(a: &str, b: &str, gap_mm: f64, ink: BoardInk) -> Scene {
    let mut scene = pair_scene(
        ink,
        [Object::Track, Object::Track],
        Some(gap_mm),
        None,
        ids::RULES_PREVIEW_CAP_GAP,
    );
    for (x, name, anchor) in [
        (CENTRE_X - CLASS_OFFSET, a, Anchor::End),
        (CENTRE_X + CLASS_OFFSET, b, Anchor::Start),
    ] {
        scene.note(
            Point::new(x, CLASS_Y),
            NoteText::User(name.to_owned()),
            anchor,
            ink.title,
        );
    }
    scene.title(
        ids::RULES_PREVIEW_TITLE_CLEARANCE,
        ids::RULES_PREVIEW_NOTE_CLEARANCE,
    );
    scene
}

pub(super) fn object_pair(objects: &[String; 2], gap_mm: f64, ink: BoardInk) -> Scene {
    let mut scene = pair_scene(
        ink,
        [Object::parse(&objects[0]), Object::parse(&objects[1])],
        Some(gap_mm),
        None,
        ids::RULES_PREVIEW_CAP_GAP,
    );
    scene.title(
        ids::RULES_PREVIEW_TITLE_CLEARANCE,
        ids::RULES_PREVIEW_NOTE_CLEARANCE,
    );
    scene
}
