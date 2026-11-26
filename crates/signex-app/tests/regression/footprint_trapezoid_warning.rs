//! #2242 — a hand-edited `.snxfpt` whose trapezoid delta makes no copper
//! opens with a Messages warning naming the footprint and the pad, instead
//! of drawing silently as a rectangle.

use signex_app::app::FootprintDocument;
use signex_app::diagnostics;
use signex_library::{Footprint, FootprintFile, Pad, PadShape};
use std::path::PathBuf;

fn trapezoid(number: &str, delta: [f64; 2]) -> Pad {
    Pad {
        number: number.into(),
        shape: PadShape::Trapezoid { delta },
        size: [1.0, 2.0],
        ..Pad::default()
    }
}

/// Opens `fp` from the bytes a saved file holds and returns the warnings
/// logged after the open that name `path`.
fn open_and_collect(fp: Footprint, path: &str) -> Vec<String> {
    let _ = diagnostics::init_logging();
    let saved = FootprintFile::from_footprint(fp)
        .to_toml_string()
        .expect("serialise");
    let file = FootprintFile::from_toml_str(&saved).expect("reparse");
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    drop(FootprintDocument::new(PathBuf::from(path), file));
    diagnostics::recent_entries()
        .into_iter()
        .filter(|e| e.id > since && e.message.contains(path))
        .map(|e| e.message.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn a_trapezoid_delta_without_copper_is_named_when_the_file_opens() {
    let mut fp = Footprint::empty("BAD_TRAP");
    fp.pads.push(trapezoid("1", [0.4, 0.0]));
    fp.pads.push(trapezoid("7", [0.4, 0.4]));
    let warnings = open_and_collect(fp, "bad_trap_2242.snxfpt");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("\"BAD_TRAP\""), "{warnings:?}");
    assert!(warnings[0].contains("pad 7: "), "{warnings:?}");
    assert!(warnings[0].contains("rectangle"), "{warnings:?}");
}

#[test]
fn a_good_trapezoid_opens_without_a_warning() {
    let mut fp = Footprint::empty("GOOD_TRAP");
    fp.pads.push(trapezoid("1", [0.4, 0.0]));
    let warnings = open_and_collect(fp, "good_trap_2242.snxfpt");
    assert!(warnings.is_empty(), "{warnings:?}");
}
