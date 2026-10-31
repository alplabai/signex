//! #1229 — a text's stored size is the height of its capitals, in every
//! family. The PDF must size the em from the face it embeds.

use signex_types::schematic::{HAlign, Point, TextNote, VAlign};
use ttf_parser::Face;

use super::tests::{empty_sheet, sample_ctx};
use super::*;

const ROBOTO: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf");
const IOSEVKA: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Iosevka-Regular.ttf");
const LABEL_MM: f64 = 1.27;

/// `/Fn size Tf` of the one note exported in `family`: its alias and em, pt.
fn exported_em(family: &str) -> (String, f32) {
    let mut sheet = empty_sheet();
    sheet.text_notes.push(TextNote {
        uuid: uuid::Uuid::nil(),
        text: "HEH".to_string(),
        position: Point::new(20.0, 20.0),
        rotation: 0.0,
        font_size: LABEL_MM,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        font_family: Some(family.to_string()),
        locked: false,
    });
    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;
    let opts = PdfOptions {
        include_title_block: false,
        ..Default::default()
    };
    let bytes = PdfExporter.export(&ctx, &opts).expect("export").bytes;
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let line = text
        .lines()
        .find(|l| l.starts_with("/F") && l.ends_with(" Tf"))
        .expect("a font selection in the content stream");
    let mut parts = line.trim_start_matches('/').split(' ');
    let alias = parts.next().unwrap().to_string();
    (alias, parts.next().unwrap().parse().unwrap())
}

fn cap_mm(face_bytes: &[u8], em_pt: f32) -> f32 {
    let face = Face::parse(face_bytes, 0).expect("face");
    let cap = f32::from(face.capital_height().expect("cap height"));
    em_pt * cap / f32::from(face.units_per_em()) * 25.4 / 72.0
}

#[test]
fn capitals_of_a_label_measure_its_size_in_roboto_and_iosevka() {
    for (family, bytes) in [("Roboto", ROBOTO), ("Iosevka", IOSEVKA)] {
        let (_, em_pt) = exported_em(family);
        let cap = cap_mm(bytes, em_pt);
        assert!(
            (cap - LABEL_MM as f32).abs() < 0.0127,
            "{family}: capitals measure {cap} mm, want {LABEL_MM}"
        );
    }
    let (_, roboto) = exported_em("Roboto");
    let (_, iosevka) = exported_em("Iosevka");
    assert!(
        (roboto - iosevka).abs() > 0.05,
        "faces with different cap heights need different ems"
    );
}
