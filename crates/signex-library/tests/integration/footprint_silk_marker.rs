//! #1718 — the silk `sketch_entity` marker is optional on the wire: a file
//! written before it existed round-trips byte for byte, and a build that
//! does not know the field still reads a file that has it.

use serde::Deserialize;
use signex_library::primitive::footprint::{FootprintFile, FpGraphic, FpGraphicKind};
use signex_sketch::id::SketchEntityId;

/// The stock polarised 0805 in its pre-#1718 shape (silk without markers),
/// with its layer token rewritten for #2195.
const PRE_1718: &str = include_str!("../fixtures/snxfpt/pre_1718_chip_0805_polarised.snxfpt");

#[test]
fn a_pre_1718_file_round_trips_byte_identical() {
    let file = FootprintFile::from_toml_str(PRE_1718).expect("parse");
    assert!(
        file.footprints[0].silk_f.iter().all(|g| !g.is_baked()),
        "no marker is invented on load"
    );
    assert_eq!(file.to_toml_string().expect("serialise"), PRE_1718);
}

/// `FpGraphic` as builds before #1718 declared it.
#[derive(Deserialize)]
#[allow(dead_code)]
struct OldFpGraphic {
    kind: FpGraphicKind,
    #[serde(default)]
    stroke_width: f64,
    #[serde(default)]
    filled: bool,
}

#[derive(Deserialize)]
struct OldFootprint {
    #[serde(default)]
    silk_f: Vec<OldFpGraphic>,
}

#[derive(Deserialize)]
struct OldFile {
    footprints: Vec<OldFootprint>,
}

#[test]
fn an_old_shaped_graphic_still_reads_a_marked_file() {
    let mut file = FootprintFile::from_toml_str(PRE_1718).expect("parse");
    let id = SketchEntityId::new();
    file.footprints[0].silk_f[0].sketch_entity = Some(id);
    file.footprints[0].silk_f.push(FpGraphic::drawn(
        FpGraphicKind::Line {
            from: [0.0, 3.0],
            to: [1.0, 3.0],
        },
        0.12,
        false,
    ));
    let text = file.to_toml_string().expect("serialise");
    assert_eq!(text.matches("sketch_entity = ").count(), 1, "{text}");

    let old: OldFile = toml::from_str(&text).expect("an older build parses it");
    assert_eq!(old.footprints[0].silk_f.len(), 2);
    let reread = FootprintFile::from_toml_str(&text).expect("reparse");
    assert_eq!(reread.footprints[0].silk_f[0].sketch_entity, Some(id));
    assert_eq!(reread.footprints[0].silk_f[1].sketch_entity, None);
}
