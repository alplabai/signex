use iced::{Color, Point, Rectangle, Size};
use signex_types::theme::{ThemeId, theme_tokens};

use super::geometry::{Fit, callout_geometry};
use super::*;

const W: FieldId = FieldId::new("w");
const GAP: FieldId = FieldId::new("gap");
const PASTE_ON: FieldId = FieldId::new("pasteon");

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn approx_point(a: Point, b: Point) -> bool {
    approx(a.x, b.x) && approx(a.y, b.y)
}

// ---- hover model ----

#[test]
fn field_hover_sets_and_its_leave_clears() {
    let mut hover = IllustrationHover::default();
    hover.update(IllustrationMsg::FieldHovered(W));
    assert_eq!(hover.hovered(), Some(&W));
    assert!(hover.is_field_highlighted(&W));
    hover.update(IllustrationMsg::FieldLeft(W));
    assert_eq!(hover.hovered(), None);
}

#[test]
fn late_leave_of_previous_row_keeps_new_hover() {
    let mut hover = IllustrationHover::default();
    hover.update(IllustrationMsg::FieldHovered(W));
    hover.update(IllustrationMsg::FieldHovered(GAP));
    hover.update(IllustrationMsg::FieldLeft(W));
    assert_eq!(hover.hovered(), Some(&GAP));
}

#[test]
fn field_hover_highlights_every_linked_part() {
    let band = Part::new(
        Primitive::Line {
            from: Point::ORIGIN,
            to: Point::new(10.0, 0.0),
        },
        Style::edge(),
    )
    .linked_all(["w", "net", "layer"]);
    let mut hover = IllustrationHover::default();
    hover.update(IllustrationMsg::FieldHovered(FieldId::new("net")));
    assert!(hover.is_part_highlighted(&band));
    assert_eq!(band.primary_field(), Some(&W));
    hover.update(IllustrationMsg::FieldHovered(GAP));
    assert!(!hover.is_part_highlighted(&band));
}

// ---- features ----

fn paste_outline() -> Part {
    Part::new(
        Primitive::Rect {
            rect: Rectangle::new(Point::new(10.0, 10.0), Size::new(20.0, 20.0)),
        },
        Style::dashed(),
    )
    .linked("paste")
    .requires(PASTE_ON)
}

#[test]
fn a_part_is_dropped_while_its_feature_is_off() {
    let ill = Illustration::new(100.0, 50.0)
        .part(paste_outline())
        .feature(PASTE_ON, false);
    assert!(!ill.is_feature_on(&PASTE_ON));
    assert_eq!(ill.visible_parts().count(), 0);
    let ill = ill.feature(PASTE_ON, true);
    assert_eq!(ill.visible_parts().count(), 1);
}

#[test]
fn switching_a_feature_off_twice_records_it_once() {
    let ill = Illustration::new(100.0, 50.0)
        .part(paste_outline())
        .feature(PASTE_ON, false)
        .feature(PASTE_ON, false)
        .feature(PASTE_ON, true);
    assert!(ill.is_feature_on(&PASTE_ON));
}

// ---- geometry ----

#[test]
fn fit_is_uniform_and_centred() {
    let fit = Fit::new(Size::new(200.0, 100.0), Size::new(400.0, 400.0));
    assert!(approx(fit.scale, 2.0));
    assert!(approx(fit.offset.x, 0.0));
    assert!(approx(fit.offset.y, 100.0));
    assert!(approx_point(
        fit.point(Point::new(100.0, 50.0)),
        Point::new(200.0, 200.0)
    ));
}

fn unit_fit() -> Fit {
    Fit::new(Size::new(100.0, 100.0), Size::new(100.0, 100.0))
}

#[test]
fn long_callout_points_heads_outward_from_its_ends() {
    let metrics = Metrics::new(10.0);
    let callout =
        Callout::new(Point::new(10.0, 50.0), Point::new(90.0, 50.0)).label(crate::raw("Width"));
    let g = callout_geometry(&callout, &unit_fit(), &metrics);
    assert!(approx_point(g.line.0, Point::new(10.0, 50.0)));
    assert!(approx_point(g.line.1, Point::new(90.0, 50.0)));
    // Tip at the end, base inside the span.
    assert!(approx_point(g.heads[0][0], Point::new(10.0, 50.0)));
    assert!(g.heads[0][1].x > 10.0);
    assert!(g.heads[1][1].x < 90.0);
    let label = g.label.expect("label placed");
    assert!(approx(label.at.x, 50.0));
    assert!(label.at.y < 50.0, "label sits above a horizontal span");
}

#[test]
fn short_callout_puts_heads_outside_pointing_in() {
    let metrics = Metrics::new(10.0);
    let callout = Callout::new(Point::new(50.0, 40.0), Point::new(50.0, 44.0));
    let g = callout_geometry(&callout, &unit_fit(), &metrics);
    assert!(
        g.line.0.y < 40.0 && g.line.1.y > 44.0,
        "line runs past both ends"
    );
    assert!(g.heads[0][1].y < 40.0, "first head base outside, above");
    assert!(g.heads[1][1].y > 44.0, "second head base outside, below");
    assert!(g.label.is_none());
}

// ---- metrics and palette ----

#[test]
fn every_size_scales_with_the_text_size() {
    let (small, large) = (Metrics::new(10.0), Metrics::new(20.0));
    assert!(approx(large.stroke(), 2.0 * small.stroke()));
    assert!(approx(large.arrow_length(), 2.0 * small.arrow_length()));
    assert!(approx(
        large.height(DEFAULT_HEIGHT_EM),
        2.0 * small.height(DEFAULT_HEIGHT_EM)
    ));
    assert!(large.text(TextRole::Note) < large.text(TextRole::Caption));
}

#[test]
fn fills_stay_at_low_alpha() {
    let palette = Palette::from_tokens(&theme_tokens(ThemeId::DEFAULT));
    let layer = Paint::Document(Color::from_rgb(0.8, 0.25, 0.25));
    for tint in [Tint::Wash, Tint::Light, Tint::Body, Tint::Strong] {
        assert!(palette.tinted(layer, tint).a <= 0.35);
    }
    assert!(approx(palette.tinted(layer, Tint::Solid).a, 1.0));
}

#[test]
fn palette_falls_back_when_extended_inks_are_unset() {
    let mut tokens = theme_tokens(ThemeId::DEFAULT);
    tokens.extended = signex_types::theme::ExtendedTokens::NONE;
    let palette = Palette::from_tokens(&tokens);
    let secondary = palette.ink(Ink::Secondary);
    let faint = palette.ink(Ink::Faint);
    assert_eq!(
        (faint.r, faint.g, faint.b),
        (secondary.r, secondary.g, secondary.b)
    );
    assert!(faint.a < secondary.a);
}
