//! The one dialog kit: message, hint, list box, buttons and footer that
//! every dialog (app, modals, Library) builds from, so the look cannot
//! drift. Generic over the message type; sizes derive from the type scale.
//! Header and close come from the header helpers of each dialog family.

use iced::widget::{Space, Widget, button, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use super::{RADIUS, Spacing, TypeScale, accent_button, danger_button, outline_button, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Button height: the body size times this, so it follows the type scale.
const BUTTON_HEIGHT_EM: f32 = 2.2;
/// Narrowest button, in body-size units, so short labels align.
const BUTTON_MIN_WIDTH_EM: f32 = 7.5;
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
    let color = match kind {
        DialogButtonKind::Primary => tokens::on_accent(tokens),
        DialogButtonKind::Secondary => ti(tokens.text),
        DialogButtonKind::Danger => tokens::danger(tokens),
    };
    let label_w = label.chars().count() as f32 * TS.body * GLYPH_ADVANCE_EM;
    let width = (label_w + 2.0 * SP.l).max(TS.body * BUTTON_MIN_WIDTH_EM);
    let face = text(label.to_string())
        .size(TS.body)
        .color(color)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);
    let btn = button(face)
        .padding([0.0, SP.l])
        .width(Length::Fixed(width))
        .height(Length::Fixed(TS.body * BUTTON_HEIGHT_EM))
        .on_press_maybe(on_press);
    match kind {
        DialogButtonKind::Primary => btn.style(accent_button(tokens)),
        DialogButtonKind::Secondary => btn.style(outline_button(tokens)),
        DialogButtonKind::Danger => btn.style(danger_button(tokens)),
    }
    .boxed()
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

/// Height of a footer built by [`dialog_footer`]: the button plus padding.
pub const fn dialog_footer_height() -> f32 {
    TS.body * BUTTON_HEIGHT_EM + 2.0 * SP.l
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
}
