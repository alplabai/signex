//! #33 (mockup m33): the Libraries location is cut from the start so the
//! file name stays, and each mount state has its pill tone.

use super::view::LOCATION_CHARS;
use super::{LibraryMount, path_tail};
use crate::styles::PillTone;

/// A path that fits is shown whole; a longer one keeps its tail, file name
/// included, behind one `…`, in at most the given number of characters.
#[test]
fn path_tail_keeps_the_file_name_of_a_long_path() {
    assert_eq!(path_tail("./dcdc-buck.snxlib", 40), "./dcdc-buck.snxlib");
    let shared = r"\\nas\eda\shared\team-passives.snxlib";
    assert_eq!(path_tail(shared, shared.chars().count()), shared);
    let cut = path_tail(shared, 24);
    assert_eq!(cut, r"…ed\team-passives.snxlib");
    assert_eq!(cut.chars().count(), 24);
    assert!(cut.starts_with('…'));
    assert!(cut.ends_with("team-passives.snxlib"));
}

/// The cut counts characters, not bytes, so a path with non-ASCII folder
/// names is never split inside a character.
#[test]
fn path_tail_counts_characters() {
    let path = "C:/Kütüphaneler/Ortak/güç.snxlib";
    let cut = path_tail(path, 12);
    assert_eq!(cut, "…/güç.snxlib");
    assert_eq!(cut.chars().count(), 12);
    assert_eq!(path_tail(path, 0), "…");
    let long = format!("{}/x.snxlib", "d".repeat(LOCATION_CHARS * 2));
    assert_eq!(
        path_tail(&long, LOCATION_CHARS).chars().count(),
        LOCATION_CHARS
    );
}

/// Mounted is ok, Missing is an error, read-only and not mounted are
/// neutral.
#[test]
fn each_mount_state_has_its_pill_tone() {
    assert_eq!(LibraryMount::Mounted.tone(), PillTone::Ok);
    assert_eq!(LibraryMount::ReadOnly.tone(), PillTone::Neutral);
    assert_eq!(LibraryMount::Missing.tone(), PillTone::Err);
    assert_eq!(LibraryMount::NotMounted.tone(), PillTone::Neutral);
}
