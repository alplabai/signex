//! Matrix pairs: a class pair drawn as two tracks at its gap, an object
//! pair as those two objects. Their gap label ("≥ 0.8 mm") writes no rule
//! column; the caller writes it to the pinned matrix cell or object-pair
//! row. The title names the two objects, and a class pair's note the two
//! classes, column class first as the mockup row 90 drawing reads.

use iced::Point;
use signex_i18n::ids;
use signex_widgets::property_illustration::Anchor;

use super::clearance::{Object, pair_scene, pair_title};
use super::scene::{BoardInk, NoteText, Scene};

/// Where the class names sit, over each track.
const CLASS_Y: f32 = 262.0;
const CLASS_OFFSET: f32 = 70.0;
const CENTRE_X: f32 = 320.0;

/// `a` is the matrix row's class, `b` the column's.
pub(super) fn class_pair(a: &str, b: &str, gap_mm: f64, ink: BoardInk) -> Scene {
    let objects = [Object::Track, Object::Track];
    let mut scene = pair_scene(
        ink,
        objects,
        Some(gap_mm),
        None,
        ids::RULES_PREVIEW_CAP_AT_LEAST,
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
    let classes = [NoteText::User(b.to_owned()), NoteText::User(a.to_owned())];
    scene.title_text(
        pair_title(objects),
        NoteText::Pair(ids::RULES_PREVIEW_NOTE_CLASS_PAIR, Box::new(classes)),
    );
    scene
}

pub(super) fn object_pair(objects: &[String; 2], gap_mm: f64, ink: BoardInk) -> Scene {
    let objects = [Object::parse(&objects[0]), Object::parse(&objects[1])];
    let mut scene = pair_scene(
        ink,
        objects,
        Some(gap_mm),
        None,
        ids::RULES_PREVIEW_CAP_AT_LEAST,
    );
    scene.title_text(
        pair_title(objects),
        NoteText::Id(ids::RULES_PREVIEW_NOTE_CLEARANCE),
    );
    scene
}
