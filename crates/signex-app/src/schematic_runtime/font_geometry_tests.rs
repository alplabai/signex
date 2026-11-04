//! #510 - hit boxes and label flags are sized for the face the text is drawn
//! in, not for a monospace guess.

use super::*;
use signex_types::schematic::text_aabb;

const TEXT: &str = "SYSTEM_RESET_REQUEST_N";

fn label(label_type: LabelType, family: Option<&str>) -> Label {
    Label {
        uuid: uuid::Uuid::now_v7(),
        text: TEXT.into(),
        position: Point::new(20.0, 30.0),
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        font_family: family.map(str::to_string),
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    }
}

/// Em width of `TEXT` as the shaper lays it out in Roboto.
fn roboto_em() -> f64 {
    crate::fonts::measure::install_hermetic();
    f64::from(crate::fonts::measure::shaped_advance_em(TEXT, "Roboto").expect("measurer"))
}

fn em_mm(label: &Label) -> f64 {
    label
        .font_size
        .max(signex_types::schematic::SCHEMATIC_TEXT_MM)
        / f64::from(signex_types::font::mm_per_em(label.font_family.as_deref()))
}

/// Width of the flag body (anchor to the far edge).
fn flag_body_width(label: &Label, document_family: Option<&str>) -> f64 {
    let vertices = label::marker_vertices(label, document_family);
    vertices[2].0 - label.position.x
}

#[test]
fn a_roboto_flag_is_as_wide_as_the_shaped_text_plus_its_padding() {
    let want_glyphs = roboto_em();
    let flag = label(LabelType::Global, Some("Roboto"));
    let em_mm = em_mm(&flag);
    let pad = em_mm * 0.325;

    let got = flag_body_width(&flag, None);
    assert!(
        (got - (want_glyphs * em_mm + 2.0 * pad)).abs() < 1e-6,
        "flag body {got} vs shaped {}",
        want_glyphs * em_mm + 2.0 * pad
    );

    // The old monospace estimate is visibly off for this text, which is what
    // let the text run past the flag.
    let old = TEXT.chars().count() as f64 * em_mm * 0.58 + 2.0 * pad;
    assert!(
        (got - old).abs() > 0.02 * old,
        "shaped {got} vs estimate {old}"
    );
}

#[test]
fn the_flag_of_a_roboto_label_holds_its_text() {
    let flag = label(LabelType::Global, Some("Roboto"));
    let (text_start, _) = label::flag_text_anchor(&flag, None);
    let text_end = text_start + roboto_em() * em_mm(&flag);
    let body_end = flag.position.x + flag_body_width(&flag, None);
    assert!(
        text_end <= body_end + 1e-9,
        "text ends {text_end}, flag ends {body_end}"
    );
}

#[test]
fn a_roboto_labels_hit_box_tracks_the_shaped_width() {
    let net = label(LabelType::Net, Some("Roboto"));
    let want = roboto_em() * em_mm(&net);
    let bbox = label_aabb(&net, None);
    assert!(
        ((bbox.max_x - bbox.min_x) - want).abs() < 1e-6,
        "{} vs {want}",
        bbox.max_x - bbox.min_x
    );

    let old = net
        .font_size
        .max(signex_types::schematic::SCHEMATIC_TEXT_MM)
        * 0.6
        * TEXT.chars().count() as f64;
    assert!((want - old).abs() > 0.02 * old);
}

#[test]
fn the_sheet_default_family_sizes_labels_that_name_none() {
    let inherits = label(LabelType::Global, None);
    let names_it = label(LabelType::Global, Some("Roboto"));
    roboto_em();
    assert_eq!(
        flag_body_width(&inherits, Some("Roboto")),
        flag_body_width(&names_it, None)
    );
}

#[test]
fn iosevka_geometry_is_the_shaped_advance() {
    roboto_em();
    let flag = label(LabelType::Global, None);
    let em = em_mm(&flag);
    let advance_em =
        f64::from(crate::fonts::measure::shaped_advance_em(TEXT, "Iosevka").expect("measurer"));
    let want = advance_em * em + 2.0 * em * 0.325;
    // The tolerance only absorbs the different order of float operations.
    assert!((flag_body_width(&flag, None) - want).abs() < 1e-6);
    assert_eq!(
        flag_body_width(&label(LabelType::Global, Some("Iosevka")), None),
        flag_body_width(&flag, None),
        "naming the bundled default is the default"
    );
}

#[test]
fn default_face_text_box_is_the_shared_types_box() {
    let got = drawn_text_aabb(
        None,
        "R101",
        None,
        1.27,
        (3.0, 4.0),
        90.0,
        HAlign::Center,
        VAlign::Bottom,
    );
    let want = text_aabb(
        "R101",
        1.27,
        (3.0, 4.0),
        90.0,
        HAlign::Center,
        VAlign::Bottom,
        None,
    );
    assert_eq!(
        (got.min_x, got.min_y, got.max_x, got.max_y),
        (want.min_x, want.min_y, want.max_x, want.max_y)
    );
}

/// #1229 — a flag is sized from the em its text is drawn at, so in units of
/// the label's own capital height it is the same shape in every bundled face.
#[test]
fn a_flag_is_as_tall_as_its_capitals_say_in_every_face() {
    let height_in_caps = |family: Option<&str>| {
        let flag = label(LabelType::Global, family);
        let vertices = label::marker_vertices(&flag, None);
        let cap_mm = flag
            .font_size
            .max(signex_types::schematic::SCHEMATIC_TEXT_MM);
        (vertices[3].1 - vertices[2].1) / cap_mm
    };
    let (iosevka, roboto) = (height_in_caps(None), height_in_caps(Some("Roboto")));
    // half_h is 0.62 em; an em is `cap / ratio`.
    assert!((iosevka - 2.0 * 0.62 / 0.735).abs() < 1e-6, "{iosevka}");
    assert!(
        (roboto - 2.0 * 0.62 / (1456.0 / 2048.0)).abs() < 1e-6,
        "{roboto}"
    );
    assert!(roboto > iosevka, "Roboto's capitals fill less of its em");
}

/// #2023 - a default-face box is as wide as the shaped advance, not a 0.6 em
/// guess.
#[test]
fn default_face_box_width_is_the_shaped_advance() {
    roboto_em();
    for text in ["R101", "SYSTEM_RESET_REQUEST_N", "10k", "e\u{301}"] {
        let size = 1.27;
        let advance_em =
            f64::from(crate::fonts::measure::shaped_advance_em(text, "Iosevka").expect("measurer"));
        let want = advance_em * size / f64::from(signex_types::font::mm_per_em(None));
        assert!(
            signex_types::font::shaped_advance_em(text, None).is_some(),
            "the shaper is registered with signex-types"
        );
        let b = text_aabb(text, size, (0.0, 0.0), 0.0, HAlign::Left, VAlign::Top, None);
        assert!((b.max_x - b.min_x - want).abs() < 1e-6, "{text}");
    }
}

fn note(font_family: Option<&str>) -> TextNote {
    TextNote {
        uuid: uuid::Uuid::now_v7(),
        text: TEXT.into(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        font_family: font_family.map(str::to_string),
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        locked: false,
    }
}

fn width_in(family: Option<&str>) -> f64 {
    let b = text_aabb(
        TEXT,
        signex_types::schematic::SCHEMATIC_TEXT_MM,
        (0.0, 0.0),
        0.0,
        HAlign::Left,
        VAlign::Top,
        family,
    );
    b.max_x - b.min_x
}

/// #2023 - a text in a non-default family gets that family's width.
#[test]
fn a_non_default_family_gets_its_own_width() {
    let em = roboto_em();
    let size = signex_types::schematic::SCHEMATIC_TEXT_MM;
    let want = em * size / f64::from(signex_types::font::mm_per_em(Some("Roboto")));
    assert!((width_in(Some("Roboto")) - want).abs() < 1e-6);
    assert!(
        (width_in(Some("Roboto")) - width_in(None)).abs() > 0.5,
        "Roboto and Iosevka measure differently"
    );
}

/// An uninstalled family resolves to the face the renderer substitutes (the
/// bundled default), so its box is the default face's box.
#[test]
fn a_missing_family_is_boxed_in_the_face_that_draws() {
    roboto_em();
    let resolved = resolve_family(Some("No Such Face 9f2"), None);
    assert!(resolved.is_none(), "the renderer substitutes the default");
    assert_eq!(width_in(resolved.as_deref()), width_in(None));
}

fn note_right_edge(sheet: &SchematicSheet) -> f64 {
    note_aabb(&sheet.text_notes[0], sheet.font_family.as_deref()).max_x
}

/// Changing the sheet default font re-measures every item without a font of
/// its own, at once, for the selection box and hit testing alike, and undo
/// and redo put the old and new boxes back.
#[test]
fn the_document_default_font_remeasures_unfonted_items_through_undo_and_redo() {
    roboto_em();
    let mut sheet = SchematicSheet::default();
    sheet.text_notes.push(note(None));
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    let edge = |engine: &signex_engine::Engine| note_right_edge(engine.document());
    let hit_at = |engine: &signex_engine::Engine, x: f64| {
        hit_test::hit_test(engine.document(), &Default::default(), x, 10.1).is_some()
    };

    let default_edge = edge(&engine);
    let (iosevka, roboto) = (width_in(None), width_in(Some("Roboto")));
    assert!((default_edge - (10.0 + iosevka)).abs() < 1e-6);

    engine
        .execute(signex_engine::Command::SetDocumentFontFamily {
            font_family: Some("Roboto".into()),
        })
        .expect("set default font");
    let roboto_edge = edge(&engine);
    assert!(
        (roboto_edge - (10.0 + roboto)).abs() < 1e-6,
        "new width at once"
    );
    let (inside_only_wider, past_both) = (
        10.0 + iosevka.max(roboto) - 0.05,
        10.0 + iosevka.max(roboto) + 0.5,
    );
    assert_eq!(hit_at(&engine, inside_only_wider), roboto >= iosevka);
    assert!(!hit_at(&engine, past_both));

    engine.undo().expect("undo");
    assert!((edge(&engine) - default_edge).abs() < 1e-6, "undo");
    assert_eq!(hit_at(&engine, inside_only_wider), iosevka >= roboto);

    engine.redo().expect("redo");
    assert!((edge(&engine) - roboto_edge).abs() < 1e-6, "redo");
    assert_eq!(hit_at(&engine, inside_only_wider), roboto >= iosevka);
}

/// Empty text keeps a one-glyph box, as before the shaper measured text.
#[test]
fn empty_text_keeps_a_one_glyph_box() {
    roboto_em();
    let empty = text_aabb("", 1.27, (0.0, 0.0), 0.0, HAlign::Left, VAlign::Top, None);
    let glyph = text_aabb("0", 1.27, (0.0, 0.0), 0.0, HAlign::Left, VAlign::Top, None);
    assert!(empty.max_x - empty.min_x > 0.1);
    assert_eq!(empty.max_x, glyph.max_x);
}

/// The default face is monospace at exactly 0.5 em, which is the advance the
/// measure falls back to before the shaper is installed, so a flag or box in
/// the default face is the same size on every frame (#621-class).
#[test]
fn the_default_face_fallback_equals_its_shaped_advance() {
    roboto_em();
    let shaped = crate::fonts::measure::shaped_advance_em(TEXT, "Iosevka").expect("measurer");
    assert!((shaped - TEXT.chars().count() as f32 * 0.5).abs() < 1e-4);
}

/// Release-build timing of hover and box select on a large sheet; run with
/// `cargo test --release -p signex-app --lib -- --ignored text_box_perf --nocapture`.
#[test]
#[ignore]
fn text_box_perf() {
    crate::fonts::measure::install_hermetic();
    let mut sheet = SchematicSheet::default();
    for i in 0..3000 {
        sheet.text_notes.push(TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: format!("NOTE_{i}_VALUE_{}", i * 7),
            position: Point::new((i % 60) as f64 * 40.0, (i / 60) as f64 * 8.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        });
        let mut l = label(LabelType::Net, None);
        l.text = format!("NET_{i}_SIG");
        l.position = Point::new((i % 60) as f64 * 40.0 + 20.0, (i / 60) as f64 * 8.0 + 4.0);
        sheet.labels.push(l);
    }
    let designators = Default::default();
    let hover = |sheet: &SchematicSheet| {
        let t = std::time::Instant::now();
        for i in 0..200 {
            let _ = hit_test::hit_test(sheet, &designators, (i % 60) as f64 * 40.0 + 1.0, 4.0);
        }
        t.elapsed() / 200
    };
    let boxed = |sheet: &SchematicSheet| {
        let t = std::time::Instant::now();
        let rect = signex_types::schematic::Aabb::new(0.0, 0.0, 1200.0, 200.0);
        let n = hit_test::hit_test_rect_mode(
            sheet,
            &designators,
            &rect,
            hit_test::SelectionMode::Touching,
        )
        .len();
        (t.elapsed(), n)
    };
    let _ = hover(&sheet);
    println!("hover per call (cold+warm): {:?}", hover(&sheet));
    let _ = boxed(&sheet);
    println!("box select 6000 items: {:?}", boxed(&sheet));
}

/// An uninstalled family's flag is the default face's flag: canvas and export
/// both pick the face that draws, not the unavailable one.
#[test]
fn a_missing_family_flag_is_the_default_face_flag() {
    roboto_em();
    assert_eq!(
        flag_body_width(&label(LabelType::Global, Some("No Such Face 9f2")), None),
        flag_body_width(&label(LabelType::Global, None), None)
    );
}

/// The stored widths of a sheet (`SchematicSheet::text_widths`) answer with the
/// new width at once after every kind of change, undo and redo included: an
/// entry is checked against the text, face and size it was measured from.
mod stored_widths {
    use super::*;

    fn engine_with_label() -> (signex_engine::Engine, uuid::Uuid) {
        let l = label(LabelType::Net, None);
        let id = l.uuid;
        let sheet = SchematicSheet {
            labels: vec![l],
            ..Default::default()
        };
        (signex_engine::Engine::new(sheet).expect("engine"), id)
    }

    /// (stored width, freshly shaped width) of the label.
    fn widths(engine: &signex_engine::Engine) -> (f64, f64) {
        let doc = engine.document();
        let family = doc.font_family.as_deref();
        let width = |b: Aabb| b.max_x - b.min_x;
        let stored = label_aabb_with(Some(&doc.text_widths), &doc.labels[0], family);
        let fresh = label_aabb(&doc.labels[0], family);
        (width(stored), width(fresh))
    }

    fn assert_current(engine: &signex_engine::Engine, was: f64, changed: bool, what: &str) -> f64 {
        let (stored, fresh) = widths(engine);
        assert!(
            (stored - fresh).abs() < 1e-9,
            "{what}: stored {stored} vs {fresh}"
        );
        assert_eq!((stored - was).abs() > 1e-6, changed, "{what}");
        stored
    }

    #[test]
    fn every_change_that_alters_a_width_is_seen_at_once_and_undo_redo_restore_it() {
        roboto_em();
        let (mut engine, id) = engine_with_label();
        let w0 = assert_current(&engine, 0.0, true, "first measure");
        assert_current(&engine, w0, false, "read back");

        engine
            .execute(signex_engine::Command::SetDocumentFontFamily {
                font_family: Some("Roboto".into()),
            })
            .unwrap();
        let w1 = assert_current(&engine, w0, true, "default font");

        engine
            .execute(signex_engine::Command::UpdateText {
                target: signex_engine::TextTarget::Label(id),
                value: "A_MUCH_LONGER_NET_NAME".into(),
            })
            .unwrap();
        let w2 = assert_current(&engine, w1, true, "text edit");

        engine
            .execute(signex_engine::Command::UpdateLabelProps {
                label_id: id,
                font_size_mm: Some(3.0),
                justify: None,
                rotation_degrees: None,
            })
            .unwrap();
        let w3 = assert_current(&engine, w2, true, "size");

        for (undone, back_to) in [("size", w2), ("text", w1), ("font", w0)] {
            engine.undo().unwrap();
            assert_current(&engine, back_to, false, &format!("undo {undone}"));
            assert!((widths(&engine).0 - back_to).abs() < 1e-6, "undo {undone}");
        }
        for (redone, to) in [("font", w1), ("text", w2), ("size", w3)] {
            engine.redo().unwrap();
            assert!((widths(&engine).0 - to).abs() < 1e-6, "redo {redone}");
        }
    }

    /// An item's own font beats the default: changing it changes the stored
    /// width without any change of the default.
    #[test]
    fn an_items_own_font_change_is_seen_at_once() {
        roboto_em();
        let (engine, _) = engine_with_label();
        let mut doc = engine.document().clone();
        let (before, _) = widths(&engine);
        let stored = |doc: &SchematicSheet| {
            let b = label_aabb_with(Some(&doc.text_widths), &doc.labels[0], None);
            b.max_x - b.min_x
        };
        assert!((stored(&doc) - before).abs() < 1e-9);
        doc.labels[0].font_family = Some("Roboto".into());
        let after = stored(&doc);
        assert!(
            (after - (label_aabb(&doc.labels[0], None).max_x - doc.labels[0].position.x)).abs()
                < 1e-9
        );
        assert!((after - before).abs() > 1e-6);
    }
}
