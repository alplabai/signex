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

/// Rest, hover and pressed fill, border colour and label colour of a role.
struct Look {
    fills: [Color; 3],
    border: Color,
    text: Color,
}

fn look(kind: DialogButtonKind, tokens: &ThemeTokens) -> Look {
    let overlays =
        || [Overlay::Faint, Overlay::Hover, Overlay::Pressed].map(|o| tokens::overlay(tokens, o));
    match kind.resolved() {
        DialogButtonKind::Primary | DialogButtonKind::Accent => {
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
        DialogButtonKind::Ghost => Look {
            fills: [Color::TRANSPARENT, overlays()[1], overlays()[2]],
            border: Color::TRANSPARENT,
            text: ti(tokens.text),
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

    const KINDS: [DialogButtonKind; 6] = [
        DialogButtonKind::Primary,
        DialogButtonKind::Secondary,
        DialogButtonKind::Danger,
        DialogButtonKind::Accent,
        DialogButtonKind::Ghost,
        DialogButtonKind::Cancel,
    ];

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
    /// accent fill.
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
            assert_eq!(fill(&primary), Some(tokens::accent_fill(&tokens)), "{id:?}");
            assert_eq!(primary.border.color, tokens::accent_fill(&tokens), "{id:?}");
        }
    }
}
