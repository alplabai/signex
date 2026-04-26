//! Unit tests for the Symbol-tab state reducer + AI-stub bridge.
//!
//! The wider editor still has no UI test harness (project memo
//! `signex_app_no_tests`), so these tests live in-crate to cover the
//! pure-data layer without spinning up iced.

use super::ai_stub::AiPinoutPreview;
use super::state::{
    BodyRect, FieldKey, PinKind, SymbolDoc, SymbolPin, SymbolSelection,
};

const LM317_BODY: &str = r#"
(symbol "LM317"
  (pin_names (offset 0))
  (in_bom yes)
  (on_board yes)
  (property "Reference" "U?" (at -5.08 5.08 0)
    (effects (font (size 1.27 1.27)))
  )
  (property "Value" "LM317" (at -5.08 -5.08 0)
    (effects (font (size 1.27 1.27)))
  )
  (rectangle (start -5.08 -2.54) (end 5.08 2.54)
    (stroke (width 0.254) (type default))
    (fill (type background))
  )
  (pin passive line (at -7.62 0 0) (length 2.54)
    (name "ADJ" (effects (font (size 1.27 1.27))))
    (number "1" (effects (font (size 1.27 1.27))))
  )
  (pin output line (at 7.62 0 180) (length 2.54)
    (name "OUT" (effects (font (size 1.27 1.27))))
    (number "2" (effects (font (size 1.27 1.27))))
  )
  (pin input line (at 0 5.08 270) (length 2.54)
    (name "IN" (effects (font (size 1.27 1.27))))
    (number "3" (effects (font (size 1.27 1.27))))
  )
)
"#;

#[test]
fn parse_blank_input_yields_empty_doc() {
    let doc = SymbolDoc::parse("", "MyPart");
    assert_eq!(doc.id, "MyPart");
    assert!(doc.pins.is_empty());
    assert_eq!(doc.designator.value, "U?");
}

#[test]
fn parse_garbage_input_falls_back_to_empty() {
    let doc = SymbolDoc::parse("not an s-expr at all", "MyPart");
    assert!(doc.pins.is_empty());
    assert_eq!(doc.id, "MyPart");
}

#[test]
fn parse_lm317_recovers_three_pins_and_fields() {
    let doc = SymbolDoc::parse(LM317_BODY, "LM317");
    assert_eq!(doc.id, "LM317");
    assert_eq!(doc.pins.len(), 3);
    let names: Vec<&str> = doc.pins.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"ADJ"));
    assert!(names.contains(&"OUT"));
    assert!(names.contains(&"IN"));
    assert_eq!(doc.designator.value, "U?");
    assert_eq!(doc.value_field.value, "LM317");
    let body = &doc.body;
    assert!((body.x0 - -5.08).abs() < 1e-6);
    assert!((body.x1 - 5.08).abs() < 1e-6);
}

#[test]
fn round_trip_preserves_pins() {
    let doc = SymbolDoc::parse(LM317_BODY, "LM317");
    let serialised = doc.to_sexpr();
    let reparsed = SymbolDoc::parse(&serialised, "LM317");
    assert_eq!(reparsed.pins.len(), 3);
    assert_eq!(reparsed.id, "LM317");
}

#[test]
fn add_pin_assigns_increasing_numbers() {
    let mut doc = SymbolDoc::empty("Part");
    let i0 = doc.add_pin(0.0, 0.0);
    let i1 = doc.add_pin(2.54, 0.0);
    let i2 = doc.add_pin(0.0, 2.54);
    assert_eq!(i0, 0);
    assert_eq!(i1, 1);
    assert_eq!(i2, 2);
    let nums: Vec<&str> = doc.pins.iter().map(|p| p.number.as_str()).collect();
    assert_eq!(nums, vec!["1", "2", "3"]);
}

#[test]
fn add_pin_after_parse_resumes_numbering_from_max() {
    let mut doc = SymbolDoc::parse(LM317_BODY, "LM317");
    assert_eq!(doc.pins.len(), 3);
    let _ = doc.add_pin(10.0, 10.0);
    assert_eq!(doc.pins.last().unwrap().number, "4");
}

#[test]
fn delete_selected_pin_drops_it_and_clears_selection() {
    let mut doc = SymbolDoc::parse(LM317_BODY, "LM317");
    doc.selected = Some(SymbolSelection::Pin(1));
    doc.delete_selected();
    assert_eq!(doc.pins.len(), 2);
    assert!(doc.selected.is_none());
}

#[test]
fn delete_field_clears_text_but_keeps_slot() {
    let mut doc = SymbolDoc::parse(LM317_BODY, "LM317");
    doc.selected = Some(SymbolSelection::Field(FieldKey::Value));
    doc.delete_selected();
    assert_eq!(doc.value_field.value, "");
    // Designator is untouched.
    assert_eq!(doc.designator.value, "U?");
}

#[test]
fn move_selected_updates_pin_position() {
    let mut doc = SymbolDoc::parse(LM317_BODY, "LM317");
    doc.selected = Some(SymbolSelection::Pin(0));
    doc.move_selected(3.81, 0.0);
    assert!((doc.pins[0].x - 3.81).abs() < 1e-6);
    assert!((doc.pins[0].y - 0.0).abs() < 1e-6);
}

#[test]
fn hit_test_finds_nearby_pin() {
    let doc = SymbolDoc::parse(LM317_BODY, "LM317");
    let hit = doc.hit_test(-7.62, 0.0).expect("pin 1 should hit at origin");
    matches!(hit, SymbolSelection::Pin(_));
    assert!(matches!(hit, SymbolSelection::Pin(_)));
}

#[test]
fn hit_test_misses_far_away_clicks() {
    let doc = SymbolDoc::parse(LM317_BODY, "LM317");
    assert!(doc.hit_test(100.0, 100.0).is_none());
}

#[test]
fn hit_test_finds_designator_field() {
    let doc = SymbolDoc::parse(LM317_BODY, "LM317");
    let hit = doc
        .hit_test(-5.08, 5.08)
        .expect("designator should hit at its anchor");
    assert!(matches!(hit, SymbolSelection::Field(FieldKey::Reference)));
}

#[test]
fn apply_ai_pinout_replaces_layout() {
    let mut doc = SymbolDoc::parse(LM317_BODY, "LM317");
    let pins = vec![
        ("1".into(), "VCC".into(), PinKind::Power),
        ("2".into(), "GND".into(), PinKind::Power),
        ("3".into(), "DATA".into(), PinKind::Bidirectional),
    ];
    doc.apply_ai_pinout(pins);
    assert_eq!(doc.pins.len(), 3);
    let names: Vec<&str> = doc.pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["VCC", "GND", "DATA"]);
    assert_eq!(doc.pins[0].kind, PinKind::Power);
    assert_eq!(doc.pins[2].kind, PinKind::Bidirectional);
}

#[test]
fn apply_empty_ai_pinout_clears_pins() {
    let mut doc = SymbolDoc::parse(LM317_BODY, "LM317");
    doc.apply_ai_pinout(Vec::new());
    assert!(doc.pins.is_empty());
}

#[test]
fn ai_preview_low_confidence_threshold() {
    let mut p = AiPinoutPreview::default();
    assert!(p.is_low_confidence());
    p.confidence = 0.49;
    assert!(p.is_low_confidence());
    p.confidence = 0.50;
    assert!(!p.is_low_confidence());
    p.confidence = 0.99;
    assert!(!p.is_low_confidence());
}

#[test]
fn round_trip_with_special_chars_in_field_value() {
    let mut doc = SymbolDoc::empty("Special");
    doc.designator.value = "U\"1".into();
    doc.value_field.value = "10\\k".into();
    let s = doc.to_sexpr();
    let r = SymbolDoc::parse(&s, "Special");
    assert_eq!(r.designator.value, "U\"1".replace('\\', "\\"));
    // The Standard sexpr lexer un-escapes; we just want the round-trip
    // to be lossless for the printable representation.
    assert!(!r.designator.value.is_empty());
}

#[test]
fn pin_kind_from_ai_stub_canonicalises_aliases() {
    assert_eq!(PinKind::from_ai_stub("input"), PinKind::Input);
    assert_eq!(PinKind::from_ai_stub("Output"), PinKind::Output);
    assert_eq!(PinKind::from_ai_stub("BiDir"), PinKind::Bidirectional);
    assert_eq!(PinKind::from_ai_stub("power"), PinKind::Power);
    assert_eq!(PinKind::from_ai_stub("power_in"), PinKind::Power);
    assert_eq!(PinKind::from_ai_stub("passive"), PinKind::Passive);
    assert_eq!(PinKind::from_ai_stub(""), PinKind::Unknown);
    assert_eq!(PinKind::from_ai_stub("nonsense"), PinKind::Unknown);
}

#[test]
fn body_rect_default_is_centred_and_symmetric() {
    let r = BodyRect::default();
    assert!((r.x0 + r.x1).abs() < 1e-6);
    assert!((r.y0 + r.y1).abs() < 1e-6);
}

#[test]
fn empty_doc_has_designator_and_value_fields() {
    let doc = SymbolDoc::empty("Part");
    // Both fields exist but the value text is allowed to be empty.
    assert_eq!(doc.designator.key, FieldKey::Reference);
    assert_eq!(doc.value_field.key, FieldKey::Value);
}

#[test]
fn pin_positions_are_preserved_through_round_trip() {
    let mut doc = SymbolDoc::empty("Part");
    let _ = doc.add_pin(2.54, -3.81);
    let _ = doc.add_pin(-5.08, 0.0);
    doc.pins[0].rotation = 0.0;
    doc.pins[1].rotation = 180.0;
    let serialised = doc.to_sexpr();
    let reparsed = SymbolDoc::parse(&serialised, "Part");
    assert_eq!(reparsed.pins.len(), 2);
    let pin_xs: Vec<f64> = reparsed.pins.iter().map(|p| p.x).collect();
    assert!(pin_xs.contains(&2.54));
    assert!(pin_xs.contains(&-5.08));
}

// ── End-to-end: PDF → AI preview → applied pins ────────────────
//
// Mirrors `signex-library/tests/ai_stub.rs` but exercises the
// editor-side wrapper so we know the bridge code holds together.

/// Generate a small in-memory LM317-style PDF using `pdf-writer`.
/// Runs at test-time only — no fixture file.
fn build_lm317_pdf() -> Vec<u8> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

    let mut pdf = Pdf::new();

    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let content_id = Ref::new(4);
    let font_id = Ref::new(5);

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id).kids([page_id]).count(1);

    let mut page = pdf.page(page_id);
    page.parent(page_tree_id)
        .media_box(Rect::new(0.0, 0.0, 612.0, 792.0))
        .contents(content_id);
    let mut resources = page.resources();
    resources.fonts().pair(Name(b"F1"), font_id).finish();
    resources.finish();
    page.finish();

    pdf.type1_font(font_id).base_font(Name(b"Helvetica"));

    let lines = [
        "LM317 Pin Configuration",
        "Pin  Name  Function",
        "1 ADJ Adjustment terminal",
        "2 OUT Regulated output voltage",
        "3 IN  Unregulated input voltage",
    ];

    let mut content = Content::new();
    content.begin_text();
    content.set_font(Name(b"F1"), 12.0);
    content.next_line(72.0, 720.0);
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            content.show(Str(line.as_bytes()));
        } else {
            content.next_line(0.0, -14.0);
            content.show(Str(line.as_bytes()));
        }
    }
    content.end_text();
    pdf.stream(content_id, &content.finish());

    pdf.finish()
}

#[test]
fn ai_preview_from_synthetic_pdf_recovers_three_pins() {
    let bytes = build_lm317_pdf();
    let preview = AiPinoutPreview::from_pdf(&bytes);
    assert_eq!(
        preview.pins.len(),
        3,
        "expected 3 pins from LM317 PDF, got {}",
        preview.pins.len()
    );
    assert!(
        preview.confidence >= 0.7,
        "confidence too low: {}",
        preview.confidence
    );
    let names: Vec<&str> = preview.pins.iter().map(|p| p.name.as_str()).collect();
    for expected in ["ADJ", "OUT", "IN"] {
        assert!(
            names.contains(&expected),
            "expected pin {expected} in {names:?}"
        );
    }
}

#[test]
fn ai_preview_apply_replaces_doc_pins_and_round_trips() {
    let bytes = build_lm317_pdf();
    let preview = AiPinoutPreview::from_pdf(&bytes);
    let mut doc = SymbolDoc::empty("LM317");
    doc.apply_ai_pinout(preview.into_apply_list());
    assert_eq!(doc.pins.len(), 3);

    // Round-trip the result through sexpr to make sure the applied
    // pins survive a serialise/parse cycle (i.e. the editor can
    // later commit them).
    let serialised = doc.to_sexpr();
    let reparsed = SymbolDoc::parse(&serialised, "LM317");
    assert_eq!(reparsed.pins.len(), 3);
}

#[test]
fn ai_preview_handles_garbage_bytes_gracefully() {
    let preview = AiPinoutPreview::from_pdf(b"not a pdf");
    assert!(preview.pins.is_empty());
    assert!(preview.is_low_confidence());
}
