//! The Frame and Size sections of a rich note (#1114, mockup row 203): written
//! in `[extras.sheet.note_layouts]` beside the notes, only what differs from
//! the default, and keys and words this build does not know are kept.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{
    HAlign, NoteFrame, NoteSize, Point, RichNoteLayout, SchematicSheet, TextNote, TextNoteFormat,
    VAlign,
};

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn note(n: u128, rich: RichNoteLayout) -> TextNote {
    TextNote {
        uuid: Uuid::from_u128(n),
        text: "= Notes\n*bold*".into(),
        position: Point::new(3.0, 4.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        locked: false,
        format: TextNoteFormat::Typst,
        rich,
    }
}

fn framed() -> RichNoteLayout {
    RichNoteLayout {
        frame: NoteFrame::Border,
        border_mm: Some(0.25),
        padding_mm: 1.0,
        size: NoteSize::FixedWidth,
        width_mm: 104.14,
        height_mm: 0.0,
    }
}

fn sheet(rich: RichNoteLayout) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.text_notes = vec![note(1, RichNoteLayout::default()), note(2, rich)];
    sheet
}

#[test]
fn a_default_layout_writes_no_table() {
    let text = save(&sheet(RichNoteLayout::default()));
    assert!(!text.contains("note_layouts"), "{text}");
    assert!(text.starts_with("format = \"snxsch/1.1\""), "{text}");
    assert_eq!(save(&load(&text)), text);
}

#[test]
fn a_framed_note_round_trips_and_writes_only_what_it_set() {
    let text = save(&sheet(framed()));
    assert!(text.contains("note_layouts"), "{text}");
    assert!(
        text.starts_with("format = \"snxsch/1.1\""),
        "the table needs no new token: {text}"
    );
    assert!(
        !text.contains("height_mm"),
        "an unset height is not written: {text}"
    );
    let back = load(&text);
    assert_eq!(back.text_notes[0].rich, RichNoteLayout::default());
    assert_eq!(back.text_notes[1].rich, framed());
    assert_eq!(save(&back), text, "a second save is identical");
}

/// The typed `text_notes` rows never carry the layout, so an older build
/// keeps it as an unknown key of `[extras.sheet]`.
#[test]
fn the_layout_is_written_once_beside_the_notes() {
    let text = save(&sheet(framed()));
    assert_eq!(text.matches("fixed_width").count(), 1, "{text}");
    assert_eq!(text.matches("104.14").count(), 1, "{text}");
}

#[test]
fn keys_and_words_this_build_does_not_know_are_kept() {
    let text = save(&sheet(framed())).replace("\"border\"", "\"dashed\"");
    let back = load(&text);
    assert_eq!(
        back.text_notes[1].rich.frame,
        NoteFrame::None,
        "an unknown frame reads as none"
    );
    assert_eq!(back.text_notes[1].rich.size, NoteSize::FixedWidth);
    assert_eq!(save(&back), text, "the word comes back as it was read");

    let mut chosen = back.clone();
    chosen.text_notes[1].rich.frame = NoteFrame::Filled;
    let resaved = save(&chosen);
    assert!(resaved.contains("\"filled\"") && !resaved.contains("dashed"));
}

#[test]
fn a_layout_entry_for_a_deleted_note_is_not_written_back() {
    let mut back = load(&save(&sheet(framed())));
    back.text_notes.remove(1);
    let text = save(&back);
    assert!(!text.contains("note_layouts"), "{text}");
}

#[test]
fn a_length_that_is_not_a_finite_length_reads_as_unset() {
    let text = save(&sheet(framed())).replace("104.14", "-3.0");
    let back = load(&text);
    assert_eq!(back.text_notes[1].rich.width_mm, 0.0);
    assert_eq!(back.text_notes[1].rich.size, NoteSize::FixedWidth);
}
