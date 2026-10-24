//! Font family persistence (#510): a family rides along only when set, so a
//! sheet that never chose one writes the bytes it always did.

use super::*;
use crate::schematic::{
    HAlign, Label, LabelType, Point as SchPoint, SchematicSheet, Symbol, TextNote, TextProp, VAlign,
};
use uuid::Uuid;

fn label(n: u128, family: Option<&str>) -> Label {
    Label {
        uuid: Uuid::from_u128(n),
        text: format!("NET{n}"),
        position: SchPoint { x: 1.0, y: 2.0 },
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: family.map(str::to_string),
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
    }
}

fn note(n: u128, family: Option<&str>) -> TextNote {
    TextNote {
        uuid: Uuid::from_u128(n),
        text: "note".into(),
        position: SchPoint { x: 3.0, y: 4.0 },
        rotation: 0.0,
        font_size: 1.27,
        font_family: family.map(str::to_string),
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn prop(family: Option<&str>) -> TextProp {
    TextProp {
        position: SchPoint { x: 5.0, y: 6.0 },
        rotation: 0.0,
        font_size: 1.27,
        font_family: family.map(str::to_string),
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        hidden: false,
    }
}

fn symbol(ref_family: Option<&str>) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(50),
        reference: "R1".into(),
        ref_text: Some(prop(ref_family)),
        ..Default::default()
    }
}

fn sheet(family: Option<&str>, item: Option<&str>) -> SchematicSheet {
    SchematicSheet {
        font_family: family.map(str::to_string),
        labels: vec![label(1, item), label(2, None)],
        text_notes: vec![note(3, item)],
        symbols: vec![symbol(item)],
        ..Default::default()
    }
}

fn write(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

#[test]
fn a_sheet_that_sets_no_family_writes_no_font_text_and_reloads_byte_identical() {
    let text = write(&sheet(None, None));
    assert!(
        !text.contains("font_family"),
        "an unset family must add nothing to the file:\n{text}"
    );
    let again = write(&SnxSchematic::parse(&text).unwrap().sheet);
    assert_eq!(again, text);
}

#[test]
fn families_set_on_every_item_kind_and_the_sheet_default_round_trip() {
    let original = sheet(Some("Sheet Face"), Some("Item Face"));
    let text = write(&original);
    let back = SnxSchematic::parse(&text).unwrap().sheet;

    assert_eq!(back.font_family.as_deref(), Some("Sheet Face"));
    assert_eq!(back.labels[0].font_family.as_deref(), Some("Item Face"));
    assert_eq!(
        back.labels[1].font_family, None,
        "a label in the same block that sets none must stay inheriting"
    );
    assert_eq!(back.text_notes[0].font_family.as_deref(), Some("Item Face"));
    let ref_text = back.symbols[0].ref_text.as_ref().unwrap();
    assert_eq!(ref_text.font_family.as_deref(), Some("Item Face"));
    assert_eq!(write(&back), text, "the encoding must be stable");
}

#[test]
fn clearing_every_family_returns_the_file_to_its_unset_bytes() {
    let unset = write(&sheet(None, None));
    let mut set = SnxSchematic::parse(&write(&sheet(Some("A"), Some("B"))))
        .unwrap()
        .sheet;
    set.font_family = None;
    set.labels[0].font_family = None;
    set.text_notes[0].font_family = None;
    set.symbols[0].ref_text.as_mut().unwrap().font_family = None;
    assert_eq!(write(&set), unset);
}

#[test]
fn a_family_with_spaces_and_quotes_survives_the_label_column() {
    let mut s = sheet(None, None);
    s.labels[0].font_family = Some("Iosevka \"Term\" Slab".into());
    let back = SnxSchematic::parse(&write(&s)).unwrap().sheet;
    assert_eq!(
        back.labels[0].font_family.as_deref(),
        Some("Iosevka \"Term\" Slab")
    );
}
