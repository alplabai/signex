use super::*;
use crate::schematic::{HAlign, Label, LabelType, Point, Symbol, TextNote, TextProp, VAlign};

fn label(n: u128, text: &str, family: Option<&str>, size: f64) -> Label {
    Label {
        uuid: Uuid::from_u128(n),
        text: text.into(),
        position: Point { x: 1.0, y: 2.0 },
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: size,
        font_family: family.map(str::to_string),
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    }
}

fn prop(size: f64) -> TextProp {
    TextProp {
        position: Point { x: 5.0, y: 6.0 },
        rotation: 0.0,
        font_size: size,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        hidden: false,
    }
}

fn sheet() -> SchematicSheet {
    SchematicSheet {
        font_family: Some("Sheet Face".into()),
        labels: vec![label(1, "NET1", None, 1.27)],
        text_notes: vec![TextNote {
            uuid: Uuid::from_u128(2),
            text: "note".into(),
            position: Point { x: 3.0, y: 4.0 },
            rotation: 0.0,
            font_size: 1.27,
            font_family: Some("Note Face".into()),
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked: false,
        }],
        symbols: vec![Symbol {
            uuid: Uuid::from_u128(3),
            reference: "R1".into(),
            value: "10k".into(),
            ref_text: Some(prop(1.27)),
            val_text: Some(prop(1.27)),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Every probe measures 2 mm wide.
fn measured(sheet: &SchematicSheet) -> TextExtents {
    TextExtents::default().refreshed(sheet, |_| Some(2.0))
}

fn first_probe(sheet: &SchematicSheet) -> TextProbe<'_> {
    probes(sheet).into_iter().next().unwrap()
}

#[test]
fn the_hash_is_sixteen_lowercase_hex_chars_of_sha256() {
    // SHA-256("abc") starts ba7816bf8f01cfea.
    assert_eq!(text_hash("abc"), "ba7816bf8f01cfea");
}

#[test]
fn every_kind_of_text_gets_a_record_with_the_requested_family() {
    let sheet = sheet();
    let extents = measured(&sheet);
    assert_eq!(extents.len(), 4);
    let family = |id: u128, field| {
        extents
            .iter()
            .find(|((u, f), _)| *u == Uuid::from_u128(id) && *f == field)
            .unwrap()
            .1
            .family
            .clone()
    };
    assert_eq!(family(1, TextField::Label).as_deref(), Some("Sheet Face"));
    assert_eq!(family(2, TextField::Note).as_deref(), Some("Note Face"));
    assert_eq!(
        family(3, TextField::SymbolValue).as_deref(),
        Some("Sheet Face")
    );
}

#[test]
fn a_changed_text_family_or_size_drops_the_record() {
    let base = sheet();
    let extents = measured(&base);
    assert_eq!(extents.retained(&base), extents, "unchanged keeps all");

    let mut text = base.clone();
    text.labels[0].text = "OTHER".into();
    let mut family = base.clone();
    family.text_notes[0].font_family = Some("Other Face".into());
    let mut size = base.clone();
    size.symbols[0].val_text.as_mut().unwrap().font_size = 2.54;
    let mut default_face = base.clone();
    default_face.font_family = None;
    for (changed, dropped) in [(&text, 1), (&family, 1), (&size, 1), (&default_face, 3)] {
        assert_eq!(extents.retained(changed).len(), 4 - dropped);
    }
}

#[test]
fn a_record_for_a_missing_item_is_dropped() {
    let base = sheet();
    let extents = measured(&base);
    let mut gone = base.clone();
    gone.labels.clear();
    assert_eq!(extents.retained(&gone).len(), 3);
}

#[test]
fn a_fresh_measurement_overwrites_and_none_keeps_the_loaded_record() {
    let base = sheet();
    let loaded = measured(&base);
    let live = loaded.refreshed(&base, |_| Some(3.0));
    assert!(live.iter().all(|(_, rec)| rec.width_nm == 3_000_000));
    let kept = loaded.refreshed(&base, |_| None);
    assert_eq!(kept, loaded);
}

#[test]
fn a_non_finite_width_skips_that_entry_without_failing() {
    let base = sheet();
    let loaded = measured(&base);
    let out = loaded.refreshed(&base, |p| {
        Some(if p.field == TextField::Label {
            f64::NAN
        } else {
            3.0
        })
    });
    let label = out
        .iter()
        .find(|((_, f), _)| *f == TextField::Label)
        .unwrap();
    assert_eq!(label.1.width_nm, 2_000_000, "NaN kept the loaded record");
    assert_eq!(out.len(), 4);
    let none = TextExtents::default().refreshed(&base, |_| Some(f64::INFINITY));
    assert!(none.is_empty());
}

#[test]
fn a_face_that_is_installed_and_final_measures_live() {
    let sheet = sheet();
    let probe = first_probe(&sheet);
    let stored = measured(&sheet);
    // requested "Sheet Face", resolved to itself.
    let got = decide_width_mm(&probe, Some("sheet face"), &stored, || (5.0, true));
    assert_eq!(got, (5.0, true));
}

#[test]
fn a_missing_face_with_a_matching_record_uses_the_stored_width() {
    let sheet = sheet();
    let probe = first_probe(&sheet);
    let stored = measured(&sheet);
    let got = decide_width_mm(&probe, None, &stored, || (5.0, true));
    assert_eq!(got, (2.0, false), "stored, and never cached");
}

#[test]
fn a_missing_face_without_a_usable_record_uses_the_substitute() {
    let sheet = sheet();
    let probe = first_probe(&sheet);
    let none = TextExtents::default();
    assert_eq!(
        decide_width_mm(&probe, None, &none, || (5.0, true)),
        (5.0, true)
    );
    let mut stale = sheet.clone();
    stale.labels[0].text = "OTHER".into();
    let stored = measured(&stale);
    assert_eq!(
        decide_width_mm(&probe, None, &stored, || (5.0, true)),
        (5.0, true)
    );
}

#[test]
fn an_installed_face_before_its_measurement_is_final_falls_back_to_the_record() {
    let sheet = sheet();
    let probe = first_probe(&sheet);
    let stored = measured(&sheet);
    let got = decide_width_mm(&probe, Some("Sheet Face"), &stored, || (5.0, false));
    assert_eq!(got, (2.0, false));
}

#[test]
fn the_default_face_counts_as_installed_however_it_is_spelled() {
    let mut sheet = sheet();
    sheet.font_family = None;
    sheet.labels[0].font_family = Some("iosevka".into());
    let probe = first_probe(&sheet);
    let stored = measured(&sheet);
    assert_eq!(
        decide_width_mm(&probe, None, &stored, || (5.0, true)),
        (5.0, true)
    );
}

#[test]
fn an_installed_face_matches_its_request_under_unicode_case_folding() {
    assert!(face_is_available(Some("ÇEŞME"), Some("Çeşme")));
    assert!(!face_is_available(Some("ÇEŞME"), None));
}
