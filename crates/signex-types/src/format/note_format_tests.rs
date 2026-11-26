//! The format of a text note (#1114): a rich note is written in
//! `[extras.sheet.note_formats]`, beside the notes, and its Typst source
//! round-trips exactly; a sheet with only plain notes writes the bytes it
//! always did.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{HAlign, Point, SchematicSheet, TextNote, TextNoteFormat, VAlign};

/// Every character an escaping bug would eat: newlines, tabs, `#`, `$`,
/// quotes, a backslash, a TOML table header and a trailing newline.
const SOURCE: &str = "= Layout notes\n\t*Keep the hot loop small.*\n\
    $V_\"out\" = 0.8 \"V\" dot (1 + R_7 / R_8)$\n\
    #table(columns: 3, [*Net*], [VIN])\n\
    [extras.sheet]\n\\ \"\"\" ''' end\n";

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn note(n: u128, text: &str, format: TextNoteFormat) -> TextNote {
    TextNote {
        uuid: Uuid::from_u128(n),
        text: text.into(),
        position: Point::new(3.0, 4.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        locked: false,
        format,
        rich: Default::default(),
    }
}

fn sheet(rich: TextNoteFormat) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.text_notes = vec![
        note(1, "plain note", TextNoteFormat::Plain),
        note(2, SOURCE, rich),
    ];
    sheet
}

#[test]
fn a_sheet_of_plain_notes_writes_no_format_and_resaves_byte_identical() {
    let text = save(&sheet(TextNoteFormat::Plain));
    assert!(!text.contains("note_formats"), "{text}");
    assert!(text.starts_with("format = \"snxsch/1.1\""), "{text}");
    let back = load(&text);
    assert!(back.text_notes.iter().all(|n| n.format.is_plain()));
    assert_eq!(save(&back), text);
}

#[test]
fn a_rich_note_keeps_its_format_and_its_source_exactly() {
    let text = save(&sheet(TextNoteFormat::Typst));
    assert!(text.contains("note_formats"), "{text}");
    assert!(
        text.starts_with("format = \"snxsch/1.1\""),
        "the table needs no new token: {text}"
    );
    let back = load(&text);
    assert_eq!(back.text_notes[0].format, TextNoteFormat::Plain);
    assert_eq!(back.text_notes[1].format, TextNoteFormat::Typst);
    assert_eq!(back.text_notes[1].text, SOURCE, "the source is the text");
    assert_eq!(save(&back), text, "a second save is identical");
}

/// The format lives beside the notes: the typed `text_notes` rows carry no
/// key an older build would drop on its next save.
#[test]
fn the_format_is_written_once_beside_the_notes() {
    let text = save(&sheet(TextNoteFormat::Typst));
    assert_eq!(text.matches("typst").count(), 1, "{text}");
    let typed_rows = text
        .lines()
        .filter(|line| line.trim_start().starts_with("format = "))
        .count();
    assert_eq!(typed_rows, 1, "only the file's own token: {text}");
}

/// An older build keeps the table as an unknown key; a newer build's word
/// reads as plain here and is written back while the note stays plain.
#[test]
fn a_format_this_build_does_not_know_reads_plain_and_is_kept() {
    let text = save(&sheet(TextNoteFormat::Typst)).replace("\"typst\"", "\"slides\"");
    let back = load(&text);
    assert!(back.text_notes.iter().all(|n| n.format.is_plain()));
    assert_eq!(save(&back), text, "the entry comes back as it was read");

    let mut chosen = back.clone();
    chosen.text_notes[1].format = TextNoteFormat::Typst;
    let resaved = save(&chosen);
    assert!(resaved.contains("\"typst\"") && !resaved.contains("slides"));
}

#[test]
fn a_format_entry_for_a_deleted_note_is_not_written_back() {
    let mut back = load(&save(&sheet(TextNoteFormat::Typst)));
    back.text_notes.remove(1);
    let text = save(&back);
    assert!(!text.contains("note_formats"), "{text}");
    assert!(!text.contains(&Uuid::from_u128(2).to_string()), "{text}");

    let mut unknown = load(&save(&sheet(TextNoteFormat::Typst)).replace("typst", "slides"));
    unknown.text_notes.remove(1);
    assert!(!save(&unknown).contains("slides"));
}
