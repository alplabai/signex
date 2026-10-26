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

// ---------------------------------------------------------------------------
// Older builds (#510)
//
// A build that predates the field must not lose a family when it opens and
// re-saves a file. `written_by_510.snxsch` is this build's output for
// `every_kind_sheet()`; `resaved_by_older_build.snxsch` is that file after
// `SnxSchematic::parse` + `write_string` of trunk at 2dcb6169, which does not
// have the field (produced with a scratch worktree, see
// `regenerate_fixtures`).
// ---------------------------------------------------------------------------

const WRITTEN: &str = include_str!("../../tests/fixtures/font_family/written_by_510.snxsch");
const RESAVED: &str =
    include_str!("../../tests/fixtures/font_family/resaved_by_older_build.snxsch");

/// A sheet with every kind of family set.
fn every_kind_sheet() -> SchematicSheet {
    let mut s = sheet(Some("Sheet Face"), None);
    s.uuid = Uuid::from_u128(100);
    s.labels = vec![label(1, Some("Label Face")), label(2, None)];
    s.text_notes = vec![note(3, Some("Note Face"))];
    let mut sym = symbol(Some("Ref Face"));
    sym.val_text = Some(prop(Some("Val Face")));
    s.symbols = vec![sym];
    s
}

fn assert_every_family(sheet: &SchematicSheet) {
    assert_eq!(sheet.font_family.as_deref(), Some("Sheet Face"));
    assert_eq!(sheet.labels[0].font_family.as_deref(), Some("Label Face"));
    assert_eq!(sheet.labels[1].font_family, None);
    assert_eq!(
        sheet.text_notes[0].font_family.as_deref(),
        Some("Note Face")
    );
    let symbol = &sheet.symbols[0];
    assert_eq!(
        symbol.ref_text.as_ref().unwrap().font_family.as_deref(),
        Some("Ref Face")
    );
    assert_eq!(
        symbol.val_text.as_ref().unwrap().font_family.as_deref(),
        Some("Val Face")
    );
}

/// Writer: families sit only where an older build keeps unknown data.
#[test]
fn this_build_writes_the_fixture_bytes() {
    assert_eq!(write(&every_kind_sheet()), WRITTEN);
}

/// Reader: a file an older build re-saved still has every family.
#[test]
fn an_older_build_resave_keeps_every_family() {
    assert_every_family(&SnxSchematic::parse(RESAVED).unwrap().sheet);
}

/// An older build's re-save of our file is our file, byte for byte: it
/// dropped and re-ordered nothing. (If that stops being true, the families
/// are no longer where older builds keep unknown data.)
#[test]
fn an_older_builds_resave_of_our_file_is_byte_identical() {
    assert_eq!(RESAVED, WRITTEN);
}

/// Entries naming an item that is gone (an older build deleted it) load
/// cleanly, change nothing, and are not written again.
#[test]
fn entries_for_a_deleted_note_or_symbol_are_dropped_on_the_next_save() {
    let mut text = write(&every_kind_sheet());
    let ghost = Uuid::from_u128(999);
    text = text.replace(
        "[extras.sheet.note_font_families]\n",
        &format!("[extras.sheet.note_font_families]\n{ghost} = \"Ghost Face\"\n"),
    );
    assert!(text.contains("Ghost Face"), "fixture edit applied:\n{text}");
    let loaded = SnxSchematic::parse(&text).unwrap().sheet;
    assert_every_family(&loaded);
    assert!(!write(&loaded).contains("Ghost Face"));
}

const FONTLESS_BY_TRUNK: &str =
    include_str!("../../tests/fixtures/font_family/fontless_by_older_build.snxsch");

/// The same sheet with no family anywhere.
fn fontless_sheet() -> SchematicSheet {
    let mut s = every_kind_sheet();
    s.font_family = None;
    s.labels.iter_mut().for_each(|l| l.font_family = None);
    s.text_notes.iter_mut().for_each(|n| n.font_family = None);
    for sym in &mut s.symbols {
        for p in [&mut sym.ref_text, &mut sym.val_text].into_iter().flatten() {
            p.font_family = None;
        }
    }
    s
}

/// A file with no fonts is the bytes an older build writes for it.
#[test]
fn a_fontless_sheet_is_byte_identical_to_the_older_builds_output() {
    assert_eq!(write(&fontless_sheet()), FONTLESS_BY_TRUNK);
}

/// Regenerates the fixtures. They need a build of `signex-types` from
/// before #510 to parse and re-save the files, so run:
///
/// ```text
/// git worktree add --detach /tmp/pre510 <commit before the #510 merge>
/// mkdir -p /tmp/pre510/crates/signex-types/examples
/// cat > /tmp/pre510/crates/signex-types/examples/resave.rs <<'EOF'
/// fn main() {
///     let a: Vec<String> = std::env::args().collect();
///     let text = std::fs::read_to_string(&a[1]).unwrap();
///     let parsed = signex_types::format::SnxSchematic::parse(&text).unwrap();
///     std::fs::write(&a[2], parsed.write_string().unwrap()).unwrap();
/// }
/// EOF
/// (cd /tmp/pre510 && cargo build -p signex-types --example resave)
/// OLDER_BUILD_RESAVE=/tmp/pre510/target/debug/examples/resave \
///     cargo test -p signex-types regenerate_fixtures -- --ignored
/// ```
#[test]
#[ignore = "writes fixtures; needs a build of an older signex-types"]
fn regenerate_fixtures() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/font_family");
    let written = format!("{dir}/written_by_510.snxsch");
    std::fs::write(&written, write(&every_kind_sheet())).unwrap();
    let tool = std::env::var("OLDER_BUILD_RESAVE").expect("OLDER_BUILD_RESAVE");
    let status = std::process::Command::new(tool)
        .args([&written, &format!("{dir}/resaved_by_older_build.snxsch")])
        .status()
        .unwrap();
    assert!(status.success());

    let fontless = format!("{dir}/fontless_input.tmp");
    std::fs::write(&fontless, write(&fontless_sheet())).unwrap();
    let status = std::process::Command::new(std::env::var("OLDER_BUILD_RESAVE").unwrap())
        .args([&fontless, &format!("{dir}/fontless_by_older_build.snxsch")])
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::remove_file(fontless).unwrap();
}
