use iced::{Color, Point, Radians, Rectangle, Size};
use signex_types::theme::{ThemeId, theme_tokens};

use super::geometry::{Fit, angle_in_sweep, callout_geometry, contains_point, distance_to_segment};
use super::hit::hit_test;
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
    assert_eq!(hover.source(), Some(HoverSource::Field));
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
fn callout_leave_does_not_clear_a_field_hover() {
    let mut hover = IllustrationHover::default();
    hover.update(IllustrationMsg::FieldHovered(W));
    hover.update(IllustrationMsg::CalloutLeft);
    assert_eq!(hover.hovered(), Some(&W));
    assert_eq!(hover.hovered_callout(), None);
}

#[test]
fn callout_hover_highlights_its_field_row_until_left() {
    let mut hover = IllustrationHover::default();
    hover.update(IllustrationMsg::CalloutHovered(GAP));
    assert!(hover.is_field_highlighted(&GAP));
    assert_eq!(hover.hovered_callout(), Some(&GAP));
    hover.update(IllustrationMsg::FieldLeft(GAP));
    assert!(
        hover.is_field_highlighted(&GAP),
        "a row leave is not a callout leave"
    );
    hover.update(IllustrationMsg::CalloutLeft);
    assert_eq!(hover.hovered(), None);
}

#[test]
fn transition_publishes_only_changes() {
    assert_eq!(hover_transition(None, None), None);
    assert_eq!(hover_transition(Some(&W), Some(W)), None);
    assert_eq!(
        hover_transition(None, Some(W)),
        Some(IllustrationMsg::CalloutHovered(W))
    );
    assert_eq!(
        hover_transition(Some(&W), Some(GAP)),
        Some(IllustrationMsg::CalloutHovered(GAP))
    );
    assert_eq!(
        hover_transition(Some(&W), None),
        Some(IllustrationMsg::CalloutLeft)
    );
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
    let callout = Callout::new(Point::new(10.0, 50.0), Point::new(90.0, 50.0))
        .label(signex_i18n::UserText::new("Width"));
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

#[test]
fn segment_distance_and_polygon_tests() {
    let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 0.0));
    assert!(approx(distance_to_segment(Point::new(5.0, 3.0), a, b), 3.0));
    assert!(approx(
        distance_to_segment(Point::new(13.0, 4.0), a, b),
        5.0
    ));
    let square = [
        Point::new(0.0, 0.0),
        Point::new(4.0, 0.0),
        Point::new(4.0, 4.0),
        Point::new(0.0, 4.0),
    ];
    assert!(contains_point(&square, Point::new(2.0, 2.0)));
    assert!(!contains_point(&square, Point::new(5.0, 2.0)));
}

#[test]
fn sweep_test_wraps_and_handles_full_turns() {
    use std::f32::consts::{FRAC_PI_2, PI, TAU};
    assert!(angle_in_sweep(FRAC_PI_2, 0.0, PI));
    assert!(!angle_in_sweep(-FRAC_PI_2, 0.0, PI));
    assert!(
        angle_in_sweep(0.0, 3.0 * FRAC_PI_2, FRAC_PI_2),
        "sweep across zero"
    );
    assert!(angle_in_sweep(1.0, 0.0, TAU));
    assert!(!angle_in_sweep(1.0, 0.5, 0.5));
}

// ---- hit testing ----

fn track_with_callout() -> Illustration {
    Illustration::new(100.0, 100.0)
        .part(Part::new(
            Primitive::Line {
                from: Point::new(0.0, 90.0),
                to: Point::new(100.0, 90.0),
            },
            Style::outline(Ink::Rule, Weight::Hairline),
        ))
        .part(
            Part::new(
                Primitive::Band {
                    points: vec![Point::new(10.0, 50.0), Point::new(90.0, 50.0)],
                    width: 10.0,
                    cap: Cap::Round,
                },
                Style::edge().with_fill(
                    Paint::Document(Color::from_rgb(0.8, 0.25, 0.25)),
                    Tint::Body,
                ),
            )
            .linked_all(["w", "net"]),
        )
        .part(
            Part::callout(Callout::new(Point::new(50.0, 45.0), Point::new(50.0, 55.0))).linked("w"),
        )
        .part(paste_outline())
        .feature(PASTE_ON, false)
}

#[test]
fn hit_reports_the_primary_field_of_the_top_part() {
    let ill = track_with_callout();
    let metrics = Metrics::new(10.0);
    let fit = Fit::new(ill.view_size(), Size::new(100.0, 100.0));
    assert_eq!(
        hit_test(&ill, &fit, &metrics, Point::new(30.0, 52.0)),
        Some(W)
    );
}

#[test]
fn unlinked_and_hidden_parts_are_not_hover_targets() {
    let ill = track_with_callout();
    let metrics = Metrics::new(10.0);
    let fit = Fit::new(ill.view_size(), Size::new(100.0, 100.0));
    // The unlinked rule line.
    assert_eq!(hit_test(&ill, &fit, &metrics, Point::new(50.0, 90.0)), None);
    // The paste outline, whose feature is off.
    assert_eq!(hit_test(&ill, &fit, &metrics, Point::new(10.0, 20.0)), None);
}

#[test]
fn arc_hit_follows_its_sweep() {
    let ill = Illustration::new(100.0, 100.0).part(
        Part::new(
            Primitive::Arc {
                center: Point::new(50.0, 50.0),
                radius: 20.0,
                start: Radians(0.0),
                end: Radians(std::f32::consts::FRAC_PI_2),
            },
            Style::edge(),
        )
        .linked("r"),
    );
    let metrics = Metrics::new(10.0);
    let fit = Fit::new(ill.view_size(), Size::new(100.0, 100.0));
    // y down: a quarter turn clockwise from +x ends at +y.
    assert_eq!(
        hit_test(&ill, &fit, &metrics, Point::new(50.0, 70.0)),
        Some(FieldId::new("r"))
    );
    assert_eq!(hit_test(&ill, &fit, &metrics, Point::new(50.0, 30.0)), None);
}

// ---- metrics and palette ----

#[test]
fn every_size_scales_with_the_text_size() {
    let (small, large) = (Metrics::new(10.0), Metrics::new(20.0));
    assert!(approx(large.stroke(), 2.0 * small.stroke()));
    assert!(approx(large.arrow_length(), 2.0 * small.arrow_length()));
    assert!(approx(large.hit_tolerance(), 2.0 * small.hit_tolerance()));
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
