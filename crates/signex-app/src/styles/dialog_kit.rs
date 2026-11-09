//! The one dialog kit: message, hint, list box, buttons and footer that
//! every dialog (app, modals, Library) builds from, so the look cannot
//! drift. Generic over the message type; sizes derive from the type scale.
//! Header and close come from the header helpers of each dialog family.

use iced::widget::{Space, Widget, button, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use super::{
    ChromeMetrics, RADIUS, Spacing, TypeScale, accent_button_rounded, danger_button, ghost_button,
    outline_button, primary_button, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Button height: the body size times this, so it follows the type scale.
const BUTTON_HEIGHT_EM: f32 = 2.2;
/// Average glyph advance, in body-size units, for sizing a button to its label.
const GLYPH_ADVANCE_EM: f32 = 0.62;

/// The role of a dialog button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButtonKind {
    /// The affirmative action; one per dialog (accent fill).
    Primary,
    /// Cancel, Close and other non-committal actions (outline).
    Secondary,
    /// Destructive actions: outline and text in the `error` colour.
    Danger,
    /// A copper accent fill, for a call to action beside a `Primary` (Graphite
    /// look; the accent fill on every theme).
    Accent,
    /// A quiet action with no fill or border ("Decide later").
    Ghost,
    /// The dismissing action of a footer (Cancel, Close, Later): drawn as
    /// Ghost, it sits first in the right-aligned row.
    Cancel,
}

impl DialogButtonKind {
    /// The role `self` is drawn with.
    pub fn resolved(self) -> Self {
        match self {
            Self::Cancel => Self::Ghost,
            other => other,
        }
    }
}

/// The main message of a dialog: primary text, body size.
pub fn dialog_message<'a, M: 'a>(message: impl ToString, tokens: &ThemeTokens) -> Element<'a, M> {
    text(message.to_string())
        .size(TS.body)
        .color(ti(tokens.text))
        .boxed()
}

/// A hint or note under the message: muted text, body size.
pub fn dialog_hint<'a, M: 'a>(hint: impl ToString, tokens: &ThemeTokens) -> Element<'a, M> {
    text(hint.to_string())
        .size(TS.body)
        .color(tokens::muted_text(tokens))
        .boxed()
}

/// Dialog body above a [`dialog_footer`]: [`SP`]`.l` on top and sides; the
/// footer's own padding supplies the bottom gap.
pub fn dialog_body<'a, M: 'a>(
    content: impl Widget<M, Theme, iced::Renderer> + 'a,
) -> Element<'a, M> {
    container(content.boxed())
        .padding(iced::Padding {
            top: SP.l,
            right: SP.l,
            bottom: 0.0,
            left: SP.l,
        })
        .boxed()
}

/// Standard dialog body padding around `content`.
pub fn dialog_section<'a, M: 'a>(
    content: impl Widget<M, Theme, iced::Renderer> + 'a,
) -> Element<'a, M> {
    container(content.boxed()).padding(SP.l).boxed()
}

/// A bordered list box (input background, 1 px border) holding `items`;
/// scrolls when the rows outgrow the space it is given.
pub fn dialog_list<'a, M: 'a>(items: Vec<Element<'a, M>>, tokens: &ThemeTokens) -> Element<'a, M> {
    let bg = tokens::input_bg(tokens);
    let border = tokens::input_border(tokens);
    container(
        scrollable(column(items).width(Length::Fill))
            .height(Length::Shrink)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(SP.s)
    .style(move |_: &Theme| container::Style {
        background: Some(bg.into()),
        border: Border {
            width: 1.0,
            radius: RADIUS.into(),
            color: border,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// A dialog button of the given role: compact, height and min width from
/// the type scale. `on_press: None` draws it disabled.
pub fn dialog_button<'a, M: Clone + 'a>(
    label: &str,
    kind: DialogButtonKind,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let kind = kind.resolved();
    let disabled = on_press.is_none();
    let color = label_color(kind, disabled, tokens);
    let (height, size, width) = button_metrics(label, kind);
    let face = text(label.to_string())
        .size(size)
        .color(color)
        .width(Length::Fill)
        .height(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);
    let btn = button(face)
        .padding(0)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .on_press_maybe(on_press);
    match kind {
        _ if disabled && kind == DialogButtonKind::Ghost => btn.style(ghost_button(tokens)),
        _ if disabled => btn.style(disabled_button(tokens)),
        DialogButtonKind::Primary => btn.style(primary_button(tokens)),
        DialogButtonKind::Accent => btn.style(accent_button_rounded(tokens)),
        DialogButtonKind::Ghost => btn.style(ghost_button(tokens)),
        DialogButtonKind::Secondary => btn.style(outline_button(tokens)),
        DialogButtonKind::Danger => btn.style(danger_button(tokens)),
        // `resolved` above never leaves `Cancel`.
        DialogButtonKind::Cancel => btn.style(outline_button(tokens)),
    }
    .boxed()
}

/// Horizontal label padding of a dense button (14 px; Ghost 10 px).
const DENSE_PAD: f32 = 14.0 * 12.0 / 11.0;
const DENSE_GHOST_PAD: f32 = 10.0 * 12.0 / 11.0;

/// `(height, label size, width)` of a dialog button. The buttons are dense:
/// one height for every role, the width hugging the label.
fn button_metrics(label: &str, kind: DialogButtonKind) -> (f32, f32, f32) {
    let chars = label.chars().count() as f32;
    let size = TS.label;
    let pad = if kind == DialogButtonKind::Ghost {
        DENSE_GHOST_PAD
    } else {
        DENSE_PAD
    };
    let width = (chars * size * GLYPH_ADVANCE_EM + 2.0 * pad).ceil();
    (ChromeMetrics::DEFAULT.button, size, width)
}

/// Label colour of a dialog button: muted when it has no action, whatever its role.
fn label_color(kind: DialogButtonKind, disabled: bool, tokens: &ThemeTokens) -> iced::Color {
    match kind.resolved() {
        _ if disabled => ti(tokens.text_secondary),
        DialogButtonKind::Primary => tokens::resolve(tokens).primary_ink,
        DialogButtonKind::Accent => tokens::on_accent(tokens),
        DialogButtonKind::Secondary | DialogButtonKind::Ghost | DialogButtonKind::Cancel => {
            ti(tokens.text)
        }
        DialogButtonKind::Danger => tokens::danger(tokens),
    }
}

/// A button without an action: no fill, a half-strength border, the muted
/// label set by [`dialog_button`]. The same for every role, so none reads as
/// clickable.
fn disabled_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let border = iced::Color {
        a: 0.5,
        ..ti(tokens.border)
    };
    let text = ti(tokens.text_secondary);
    move |_: &Theme, _: button::Status| button::Style {
        background: None,
        text_color: text,
        border: Border {
            width: 1.0,
            radius: RADIUS.into(),
            color: border,
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Right-aligned button row with the standard gaps and padding.
pub fn dialog_footer<'a, M: 'a>(buttons: Vec<Element<'a, M>>) -> Element<'a, M> {
    dialog_footer_split(Vec::new(), buttons)
}

/// [`dialog_footer`] with extra actions at the left end.
pub fn dialog_footer_split<'a, M: 'a>(
    extras: Vec<Element<'a, M>>,
    buttons: Vec<Element<'a, M>>,
) -> Element<'a, M> {
    let extras = row(extras).spacing(SP.s).align_y(Alignment::Center);
    let buttons = row(buttons).spacing(SP.s).align_y(Alignment::Center);
    container(row![extras, Space::new().width(Length::Fill), buttons].align_y(Alignment::Center))
        .padding(SP.l)
        .width(Length::Fill)
        .boxed()
}

/// Height of the footer the base dialog sizes were measured against: a
/// body-size button plus padding.
pub const fn dialog_footer_height() -> f32 {
    TS.body * BUTTON_HEIGHT_EM + 2.0 * SP.l
}

/// Height of a footer built by [`dialog_footer`]: the dense button plus
/// padding.
pub const fn dense_footer_height() -> f32 {
    ChromeMetrics::DEFAULT.button + 2.0 * SP.l
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    /// The kit builds from tokens alone for every built-in theme, and the
    /// footer height follows the button height.
    #[test]
    fn kit_builds_for_every_theme() {
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            for kind in [
                DialogButtonKind::Primary,
                DialogButtonKind::Secondary,
                DialogButtonKind::Danger,
                DialogButtonKind::Accent,
                DialogButtonKind::Ghost,
                DialogButtonKind::Cancel,
            ] {
                let _: Element<'_, ()> = dialog_button("OK", kind, Some(()), &tokens);
                let _: Element<'_, ()> = dialog_button("OK", kind, None, &tokens);
            }
            let _: Element<'_, ()> = dialog_message("message", &tokens);
            let _: Element<'_, ()> = dialog_hint("hint", &tokens);
            let _: Element<'_, ()> = dialog_list(vec![dialog_hint("row", &tokens)], &tokens);
            let _: Element<'_, ()> = dialog_footer(vec![dialog_hint("x", &tokens)]);
        }
        assert!(dialog_footer_height() > TS.body * BUTTON_HEIGHT_EM);
    }

    /// The footer height is the dense button plus padding.
    #[test]
    fn the_footer_height_follows_the_buttons() {
        assert_eq!(
            dense_footer_height(),
            ChromeMetrics::DEFAULT.button + 2.0 * SP.l
        );
    }

    /// Cancel is the quiet Ghost; no other role changes.
    #[test]
    fn cancel_is_ghost() {
        assert_eq!(DialogButtonKind::Cancel.resolved(), DialogButtonKind::Ghost);
        for kind in [
            DialogButtonKind::Primary,
            DialogButtonKind::Secondary,
            DialogButtonKind::Danger,
            DialogButtonKind::Accent,
            DialogButtonKind::Ghost,
        ] {
            assert_eq!(kind.resolved(), kind);
        }
    }

    /// Footer buttons are dense: every role has the same height and the
    /// width hugs the label.
    #[test]
    fn graphite_buttons_share_one_height_and_hug_their_label() {
        for kind in [
            DialogButtonKind::Primary,
            DialogButtonKind::Danger,
            DialogButtonKind::Accent,
            DialogButtonKind::Ghost,
            DialogButtonKind::Secondary,
        ] {
            assert_eq!(
                button_metrics("Cancel", kind).0,
                ChromeMetrics::DEFAULT.button,
                "{kind:?}"
            );
        }
        let ok = button_metrics("OK", DialogButtonKind::Primary).2;
        let long = button_metrics("New Library\u{2026}", DialogButtonKind::Primary).2;
        assert!(ok < long, "{ok} < {long}");
        assert!(ok < TS.label * 7.5, "{ok}");
    }

    /// A button without an action never looks clickable: no fill, no hover
    /// fill, and the muted label, for every role and theme.
    #[test]
    fn a_disabled_button_has_no_fill_and_a_muted_label_for_every_theme() {
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            for status in [
                button::Status::Disabled,
                button::Status::Hovered,
                button::Status::Pressed,
            ] {
                let style = disabled_button(&tokens)(&Theme::Dark, status);
                assert!(style.background.is_none(), "{id:?} {status:?}");
                assert_eq!(style.text_color, ti(tokens.text_secondary), "{id:?}");
            }
            for kind in [
                DialogButtonKind::Primary,
                DialogButtonKind::Secondary,
                DialogButtonKind::Danger,
                DialogButtonKind::Accent,
                DialogButtonKind::Ghost,
            ] {
                assert_eq!(
                    label_color(kind, true, &tokens),
                    ti(tokens.text_secondary),
                    "{id:?} {kind:?}"
                );
                assert_ne!(
                    label_color(kind, false, &tokens),
                    ti(tokens.text_secondary),
                    "{id:?} {kind:?}: an enabled label is not the muted one"
                );
            }
        }
    }
}
