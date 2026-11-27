//! The `.snxfpt` format version (#2242): `snxfpt/1.1` is written on every
//! save, `snxfpt/1` and `snxfpt/1.1` are read, a newer token is refused by
//! name; and a trapezoid delta that makes no copper is reported by pad.

use super::*;

fn file_text(format: &str) -> String {
    format!(
        r#"format = "{format}"
file_uuid = "00000000-0000-0000-0000-000000000001"
display_name = "Versioned"
created = "2026-11-17T00:00:00Z"
updated = "2026-11-17T00:00:00Z"
footprints = []
"#
    )
}

fn trapezoid_pad(number: &str, delta: [f64; 2]) -> Pad {
    Pad {
        number: number.into(),
        shape: PadShape::Trapezoid { delta },
        size: [1.0, 2.0],
        ..Pad::default()
    }
}

#[test]
fn a_saved_footprint_file_is_written_as_snxfpt_1_1() {
    let mut fp = Footprint::empty("TRAP");
    fp.pads.push(trapezoid_pad("1", [0.4, 0.0]));
    let text = FootprintFile::from_footprint(fp)
        .to_toml_string()
        .expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.1\"\n"), "{text}");
    let back = FootprintFile::from_toml_str(&text).expect("reads back");
    assert_eq!(back.format, "snxfpt/1.1");
    assert_eq!(
        back.footprints[0].pads[0].shape,
        PadShape::Trapezoid { delta: [0.4, 0.0] }
    );
}

#[test]
fn a_snxfpt_1_file_reads_and_its_next_save_writes_1_1() {
    let old = FootprintFile::from_toml_str(&file_text("snxfpt/1")).expect("1 reads");
    assert_eq!(old.format, "snxfpt/1", "the token it was read with");
    let saved = old.to_toml_string().expect("serialise");
    assert!(saved.starts_with("format = \"snxfpt/1.1\"\n"), "{saved}");
    FootprintFile::from_toml_str(&file_text("snxfpt/1.1")).expect("1.1 reads");
}

#[test]
fn a_newer_snxfpt_format_is_refused_by_name() {
    // 1.4 is this build's own since #2593 (a pad's own thermal relief) and
    // #3107/#3108 (embedded STEP models).
    for newer in ["snxfpt/1.5", "snxfpt/2"] {
        match FootprintFile::from_toml_str(&file_text(newer)) {
            Err(error @ FootprintFileError::UnsupportedFormat { .. }) => {
                let message = error.to_string();
                assert!(message.contains(newer), "{message}");
                assert!(message.contains("newer Signex"), "{message}");
            }
            other => panic!("{newer}: expected UnsupportedFormat, got {other:?}"),
        }
    }
}

/// #2207: a file with no token, or a garbled one, is refused, never read
/// as some version.
#[test]
fn a_missing_or_garbled_snxfpt_token_is_refused() {
    let tokenless = file_text("snxfpt/1").replacen("format = \"snxfpt/1\"\n", "", 1);
    assert!(!tokenless.contains("format"), "{tokenless}");
    match FootprintFile::from_toml_str(&tokenless) {
        Err(FootprintFileError::UnsupportedFormat(reason)) => {
            assert_eq!(reason.found(), None, "{reason}");
        }
        other => panic!("expected UnsupportedFormat, got {other:?}"),
    }
    for garbled in ["snxfpt/x", "snxsym/v1", "snxfpt/1.01", ""] {
        assert!(
            matches!(
                FootprintFile::from_toml_str(&file_text(garbled)),
                Err(FootprintFileError::UnsupportedFormat(_))
            ),
            "{garbled}"
        );
    }
}

#[test]
fn a_trapezoid_delta_without_copper_is_reported_by_pad() {
    let mut fp = Footprint::empty("TRAP");
    fp.pads.push(trapezoid_pad("1", [0.4, 0.0]));
    fp.pads.push(trapezoid_pad("2", [0.4, 0.4]));
    fp.pads.push(trapezoid_pad("3", [5.0, 0.0]));
    fp.pads.push(Pad {
        number: "4".into(),
        ..Pad::default()
    });
    let faults = fp.trapezoid_pad_faults();
    assert_eq!(faults.len(), 2, "{faults:?}");
    assert!(faults[0].starts_with("pad 2: "), "{faults:?}");
    assert!(faults[1].starts_with("pad 3: "), "{faults:?}");
}
