//! The one family of dialog footer buttons (#2397): every role has the
//! same corner radius and border width (the size and padding come from
//! `dialog_kit::button_metrics`). Roles differ only in fill and colour,
//! and every role steps rest, hover, pressed the same way: a stronger fill
//! at each step.

use iced::widget::button;
use iced::{Background, Border, Color, Theme};
use signex_types::theme::ThemeTokens;

use super::DialogButtonKind;
use super::radii::Radii;
use super::ti;
use super::tokens::{self, Overlay};

/// Border width of every dialog button, whether the border shows or not.
pub(super) const BORDER_WIDTH: f32 = 1.0;

/// Fill alpha of the danger tint at rest, hover and pressed.
const DANGER_TINT: [f32; 3] = [0.08, 0.14, 0.24];

/// Black laid over a solid fill (primary, solid danger) on hover and pressed.
const SOLID_STEP: [f32; 2] = [0.08, 0.16];

/// Rest, hover and pressed fill, border colour and label colour of a role.
struct Look {
    fills: [Color; 3],
    border: Color,
    text: Color,
}

/// A solid `fill` with `text` on it, darker on hover and darker still when
/// pressed; the border is the fill, so none shows.
fn solid(fill: Color, text: Color) -> Look {
    let step = |a: f32| tokens::composite_over(Color::from_rgba(0.0, 0.0, 0.0, a), fill);
    Look {
        fills: [fill, step(SOLID_STEP[0]), step(SOLID_STEP[1])],
        border: fill,
        text,
    }
}

fn look(kind: DialogButtonKind, tokens: &ThemeTokens) -> Look {
    let overlays =
        || [Overlay::Faint, Overlay::Hover, Overlay::Pressed].map(|o| tokens::overlay(tokens, o));
    match kind.resolved() {
        // The theme's primary: the ink on Graphite (mockup row 16, Save
        // All), the accent fill on a theme that sets none.
        DialogButtonKind::Primary => {
            let r = tokens::resolve(tokens);
            solid(r.primary, r.primary_ink)
        }
        DialogButtonKind::Accent => {
            let fill = tokens::accent_fill(tokens);
            Look {
                fills: [
                    fill,
                    tokens::accent_fill_hover(tokens),
                    tokens::accent_fill_pressed(tokens),
                ],
                // The border is the fill: none shows.
                border: fill,
                text: tokens::on_accent(tokens),
            }
        }
        DialogButtonKind::Danger => {
            let danger = tokens::danger(tokens);
            Look {
                fills: DANGER_TINT.map(|a| Color { a, ..danger }),
                border: danger,
                text: danger,
            }
        }
        DialogButtonKind::DangerSolid => {
            let danger = tokens::danger(tokens);
            // White on the light theme's deep red, the dark ink on the dark
            // theme's pale red.
            let text = tokens::keep_readable_on(tokens::resolve(tokens).primary_ink, danger);
            solid(danger, text)
        }
        DialogButtonKind::Ghost => Look {
            fills: [Color::TRANSPARENT, overlays()[1], overlays()[2]],
            border: Color::TRANSPARENT,
            text: ti(tokens.text),
        },
        DialogButtonKind::Link => Look {
            fills: [Color::TRANSPARENT, overlays()[1], overlays()[2]],
            border: Color::TRANSPARENT,
            text: tokens::resolve(tokens).accent_text,
        },
        DialogButtonKind::Secondary | DialogButtonKind::Cancel => Look {
            fills: overlays(),
            border: ti(tokens.border),
            text: ti(tokens.text),
        },
    }
}

/// The label colour of an enabled button of `kind`.
pub(super) fn dialog_label_color(kind: DialogButtonKind, tokens: &ThemeTokens) -> Color {
    look(kind, tokens).text
}

/// The style of an enabled dialog button of `kind`.
pub(super) fn dialog_button_style(
    kind: DialogButtonKind,
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let Look {
        fills,
        border,
        text,
    } = look(kind, tokens);
    move |_: &Theme, status: button::Status| {
        let fill = match status {
            button::Status::Hovered => fills[1],
            button::Status::Pressed => fills[2],
            button::Status::Active | button::Status::Disabled => fills[0],
        };
        button::Style {
            background: (fill.a > 0.0).then_some(Background::Color(fill)),
            text_color: text,
            border: dialog_border(border),
            ..button::Style::default()
        }
    }
}

/// The border every dialog button draws, in `color`.
pub(super) fn dialog_border(color: Color) -> Border {
    Border {
        width: BORDER_WIDTH,
        radius: Radii::GRAPHITE.button.into(),
        color,
        ..Border::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    const KINDS: [DialogButtonKind; 8] = [
        DialogButtonKind::Primary,
        DialogButtonKind::Secondary,
        DialogButtonKind::Danger,
        DialogButtonKind::DangerSolid,
        DialogButtonKind::Accent,
        DialogButtonKind::Ghost,
        DialogButtonKind::Link,
        DialogButtonKind::Cancel,
    ];

    /// Mockup row 71: a Link is accent text with no fill and no visible
    /// border at rest; hover gives it the Ghost's fill.
    #[test]
    fn a_link_is_accent_text_without_a_frame() {
        for &id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(id);
            let rest = style(DialogButtonKind::Link, id, button::Status::Active);
            assert_eq!(fill(&rest), None, "{id:?}");
            assert_eq!(rest.border.color, Color::TRANSPARENT, "{id:?}");
            assert_eq!(
                rest.text_color,
                tokens::resolve(&tokens).accent_text,
                "{id:?}"
            );
            assert_eq!(
                dialog_label_color(DialogButtonKind::Link, &tokens),
                tokens::resolve(&tokens).accent_text,
                "{id:?}"
            );
            let hover = |kind| fill(&style(kind, id, button::Status::Hovered));
            assert_eq!(
                hover(DialogButtonKind::Link),
                hover(DialogButtonKind::Ghost),
                "{id:?}"
            );
        }
    }

    fn style(kind: DialogButtonKind, id: ThemeId, status: button::Status) -> button::Style {
        let tokens = signex_types::theme::theme_tokens(id);
        dialog_button_style(kind, &tokens)(&Theme::Dark, status)
    }

    fn fill(style: &button::Style) -> Option<Color> {
        match style.background {
            Some(Background::Color(c)) => Some(c),
            _ => None,
        }
    }

    /// One family: every role, theme and state has the same border width
    /// and corner radius.
    #[test]
    fn every_role_shares_the_border_width_and_radius() {
        for &id in ThemeId::BUILTINS {
            for kind in KINDS {
                for status in [
                    button::Status::Active,
                    button::Status::Hovered,
                    button::Status::Pressed,
                ] {
                    let border = style(kind, id, status).border;
                    assert_eq!(border.width, BORDER_WIDTH, "{id:?} {kind:?} {status:?}");
                    assert_eq!(
                        border.radius,
                        iced::border::Radius::from(Radii::GRAPHITE.button),
                        "{id:?} {kind:?} {status:?}"
                    );
                }
            }
        }
    }

    /// Hover and pressed follow one rule for every role: each changes the
    /// fill, and pressed differs from hover.
    #[test]
    fn hover_and_pressed_step_the_fill_for_every_role() {
        for &id in ThemeId::BUILTINS {
            for kind in KINDS {
                let [rest, hover, pressed] = [
                    button::Status::Active,
                    button::Status::Hovered,
                    button::Status::Pressed,
                ]
                .map(|s| fill(&style(kind, id, s)));
                assert!(hover.is_some() && pressed.is_some(), "{id:?} {kind:?}");
                assert_ne!(rest, hover, "{id:?} {kind:?}");
                assert_ne!(hover, pressed, "{id:?} {kind:?}");
            }
        }
    }

    /// Cancel is a Secondary: a subtle fill and the standard border. Danger
    /// is a subtle red fill with a red border and red label. Primary is the
    /// theme's primary fill (mockup row 16), the accent fill on a theme that
    /// sets none.
    #[test]
    fn the_roles_differ_only_in_fill_and_colour() {
        for &id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(id);
            let rest = |kind| style(kind, id, button::Status::Active);

            let cancel = rest(DialogButtonKind::Cancel);
            assert_eq!(cancel, rest(DialogButtonKind::Secondary), "{id:?}");
            assert!(fill(&cancel).is_some(), "{id:?}: Cancel has a fill");
            assert_eq!(cancel.border.color, ti(tokens.border), "{id:?}");

            let danger = rest(DialogButtonKind::Danger);
            let red = tokens::danger(&tokens);
            assert_eq!(danger.border.color, red, "{id:?}");
            assert_eq!(danger.text_color, red, "{id:?}");
            let tint = fill(&danger).expect("a danger tint");
            assert_eq!((tint.r, tint.g, tint.b), (red.r, red.g, red.b), "{id:?}");
            assert!(tint.a > 0.0 && tint.a < 0.5, "{id:?}: a subtle tint");

            let primary = rest(DialogButtonKind::Primary);
            let r = tokens::resolve(&tokens);
            assert_eq!(fill(&primary), Some(r.primary), "{id:?}");
            assert_eq!(primary.border.color, r.primary, "{id:?}");
            assert_eq!(primary.text_color, r.primary_ink, "{id:?}");
        }
    }

    /// Mockup row 16: on Graphite the primary action (Save All) is the ink
    /// fill with the paper label, not the accent; the Accent role keeps the
    /// accent fill beside it.
    #[test]
    fn the_graphite_primary_is_the_ink_not_the_accent() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = signex_types::theme::theme_tokens(id);
            let r = tokens::resolve(&tokens);
            let primary = style(DialogButtonKind::Primary, id, button::Status::Active);
            let accent = style(DialogButtonKind::Accent, id, button::Status::Active);
            assert_eq!(fill(&primary), Some(r.primary), "{id:?}");
            assert_eq!(primary.text_color, r.primary_ink, "{id:?}");
            assert_ne!(fill(&primary), fill(&accent), "{id:?}");
            assert_eq!(fill(&accent), Some(tokens::accent_fill(&tokens)), "{id:?}");
        }
        let light = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let ink = style(
            DialogButtonKind::Primary,
            ThemeId::GraphiteLight,
            button::Status::Active,
        );
        let ink_fill = fill(&ink).expect("a fill");
        assert!(
            tokens::relative_luminance(ink_fill)
                < tokens::relative_luminance(tokens::accent_fill(&light)),
            "the light primary is darker than the accent: {ink_fill:?}"
        );
    }

    /// The solid danger role is the full `error` fill, darker on hover and
    /// darker still when pressed, with a label that reads on the fill: white
    /// on Graphite Light, the dark ink on Graphite Dark. The soft Danger role
    /// is unchanged beside it.
    #[test]
    fn solid_danger_is_the_error_fill_with_a_readable_label() {
        for &id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(id);
            let red = tokens::danger(&tokens);
            let [rest, hover, pressed] = [
                button::Status::Active,
                button::Status::Hovered,
                button::Status::Pressed,
            ]
            .map(|s| style(DialogButtonKind::DangerSolid, id, s));
            assert_eq!(fill(&rest), Some(red), "{id:?}");
            assert_eq!(rest.border.color, red, "{id:?}");
            let lum = |s: &button::Style| tokens::relative_luminance(fill(s).expect("a fill"));
            assert!(lum(&hover) < lum(&rest), "{id:?}: hover is darker");
            assert!(
                lum(&pressed) < lum(&hover),
                "{id:?}: pressed is darker still"
            );
            assert!(
                tokens::contrast_ratio(rest.text_color, red) >= tokens::UNREADABLE_CONTRAST,
                "{id:?}: the label reads on the fill"
            );
            assert_ne!(
                style(DialogButtonKind::Danger, id, button::Status::Active),
                rest,
                "{id:?}: soft and solid danger differ"
            );
        }
        let light = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let dark = signex_types::theme::theme_tokens(ThemeId::GraphiteDark);
        assert_eq!(
            dialog_label_color(DialogButtonKind::DangerSolid, &light),
            Color::WHITE
        );
        assert_eq!(
            dialog_label_color(DialogButtonKind::DangerSolid, &dark),
            tokens::resolve(&dark).primary_ink
        );
    }
}
