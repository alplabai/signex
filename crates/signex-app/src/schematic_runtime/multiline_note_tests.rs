//! #2120 - a multi-line note's hit and selection box cover every drawn line.

use super::*;

fn note(text: &str, v_align: VAlign, rotation: f64) -> TextNote {
    TextNote {
        uuid: uuid::Uuid::now_v7(),
        text: text.into(),
        position: Point::new(10.0, 10.0),
        rotation,
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: v_align,
        locked: false,
    }
}

fn line_mm() -> f64 {
    signex_types::font::line_box_mm(signex_types::schematic::SCHEMATIC_TEXT_MM, None)
}

fn hits(note: &TextNote, x: f64, y: f64) -> bool {
    let mut sheet = SchematicSheet::default();
    sheet.text_notes.push(note.clone());
    hit_test::hit_test(&sheet, &Default::default(), x, y).is_some()
}

#[test]
fn a_three_line_note_is_three_line_boxes_tall() {
    let n = note("AAAA\nBBBB\nCCCC", VAlign::Top, 0.0);
    let box_ = note_aabb(&n, None);
    assert!(((box_.max_y - box_.min_y) - 3.0 * line_mm()).abs() < 1e-9);
    assert_eq!(box_.min_y, 10.0);
}

#[test]
fn a_click_on_the_third_line_hits_the_note() {
    let n = note("AAAA\nBBBB\nCCCC", VAlign::Top, 0.0);
    let third = 10.0 + 2.5 * line_mm();
    assert!(hits(&n, 10.5, third));
    assert!(!hits(&n, 10.5, 10.0 + 3.0 * line_mm() + 1.0), "below it");
}

#[test]
fn vertical_alignment_places_the_box_over_the_drawn_lines() {
    let h = 3.0 * line_mm();
    for (v, top) in [
        (VAlign::Top, 10.0),
        (VAlign::Center, 10.0 - h * 0.5),
        (VAlign::Bottom, 10.0 - h),
    ] {
        let b = note_aabb(&note("AAAA\nBBBB\nCCCC", v, 0.0), None);
        assert!((b.min_y - top).abs() < 1e-9 && (b.max_y - top - h).abs() < 1e-9);
    }
}

#[test]
fn a_rotated_note_box_covers_its_lines() {
    let h = 3.0 * line_mm();
    let b = note_aabb(&note("AAAA\nBBBB\nCCCC", VAlign::Top, 90.0), None);
    assert!((b.max_x - 10.0).abs() < 1e-9 && (b.min_x - (10.0 - h)).abs() < 1e-9);
    assert!(hits(
        &note("AAAA\nBBBB\nCCCC", VAlign::Top, 90.0),
        10.0 - 2.5 * line_mm(),
        10.5
    ));
}

#[test]
fn a_single_line_note_keeps_its_box() {
    let n = note("AAAA", VAlign::Center, 0.0);
    let b = note_aabb(&n, None);
    let want = signex_types::schematic::text_aabb_with_width(
        b.max_x - b.min_x,
        signex_types::schematic::SCHEMATIC_TEXT_MM,
        (10.0, 10.0),
        0.0,
        HAlign::Left,
        VAlign::Center,
        None,
    );
    assert_eq!(b, want);
}
