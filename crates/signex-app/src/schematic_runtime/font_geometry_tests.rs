//! #510 - hit boxes and label flags are sized for the face the text is drawn
//! in, not for a monospace guess.

use super::*;

const TEXT: &str = "SYSTEM_RESET_REQUEST_N";

fn label(label_type: LabelType, family: Option<&str>) -> Label {
    Label {
        uuid: uuid::Uuid::new_v4(),
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
        / f64::from(signex_gfx::primitive::text::MM_PER_EM)
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
fn iosevka_geometry_is_the_monospace_estimate_it_always_was() {
    roboto_em();
    let flag = label(LabelType::Global, None);
    let em = em_mm(&flag);
    let want = TEXT.chars().count() as f64 * em * 0.58 + 2.0 * em * 0.325;
    // Both `em_mm` and the geometry's own estimate divide by
    // `f64::from(MM_PER_EM)`; the tolerance only absorbs the different
    // order of float operations between the two sums.
    assert!((flag_body_width(&flag, None) - want).abs() < 1e-6);
    assert_eq!(
        flag_body_width(&label(LabelType::Global, Some("Iosevka")), None),
        flag_body_width(&flag, None),
        "naming the bundled default is the default"
    );
}

#[test]
fn default_face_text_box_is_the_shared_types_box() {
    use signex_types::schematic::text_aabb;
    let got = drawn_text_aabb(
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
    );
    assert_eq!(
        (got.min_x, got.min_y, got.max_x, got.max_y),
        (want.min_x, want.min_y, want.max_x, want.max_y)
    );
}
