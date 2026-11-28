//! The one dialog kit: message, hint, list box, buttons and footer that
//! every dialog (app, modals, Library) builds from, so the look cannot
//! drift. Generic over the message type; sizes derive from the type scale.
//! Header and close come from the header helpers of each dialog family.

use iced::widget::{Space, Widget, column, container, row, scrollable};
use iced::{Alignment, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, raw, text};

use super::dialog_buttons::{dialog_border, dialog_button_style, dialog_label_color};
use super::{ChromeMetrics, RADIUS, Spacing, TypeScale, chrome_px, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Button height: the body size times this, so it follows the type scale.
const BUTTON_HEIGHT_EM: f32 = 2.2;
/// Average glyph advance, in body-size units, for sizing a button to its label.
const GLYPH_ADVANCE_EM: f32 = 0.62;

/// The role of a dialog button. Every role has the same height, radius,
/// border width and padding; they differ in fill and colour only (#2397,
/// see `dialog_buttons`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButtonKind {
    /// The affirmative action; one per dialog (the theme's primary fill: the
    /// ink on Graphite, the accent fill on a theme that sets none).
    Primary,
    /// Cancel, Close and other non-committal actions: a subtle fill and the
    /// standard border.
    Secondary,
    /// Destructive actions: a subtle `error` tint, border and label.
    Danger,
    /// The one destructive action a dialog exists for (Delete): the solid
    /// `error` fill with a label that reads on it.
    DangerSolid,
    /// The accent fill, for a call to action beside a `Primary`.
    Accent,
    /// A quiet action with no fill or visible border at rest ("Decide
    /// later"), its label drawn in the accent text, as every mockup's
    /// `.btn.ghost`.
    Ghost,
    /// A text action in the accent ink with no fill or border at rest, nor
    /// when it has no action ("Clear all colors", mockup row 71).
    Link,
    /// A secondary action that must not compete with Save: no fill or
    /// border at rest, its label in the secondary ink, as every mockup's
    /// `.btn.quiet` (Reload saved version, mockup row 130).
    Quiet,
    /// The dismissing action of a footer (Cancel, Close, Later): drawn as
    /// Secondary, it sits first in the right-aligned row.
    Cancel,
}

impl DialogButtonKind {
    /// The role `self` is drawn with.
    pub fn resolved(self) -> Self {
        match self {
            Self::Cancel => Self::Secondary,
            other => other,
        }
    }
}

/// The main message of a dialog: primary text, body size.
pub fn dialog_message<'a, M: 'a>(message: impl ToString, tokens: &ThemeTokens) -> Element<'a, M> {
    text(raw(message.to_string()))
        .size(TS.body)
        .color(ti(tokens.text))
        .boxed()
}

/// A hint or note under the message: muted text, body size.
pub fn dialog_hint<'a, M: 'a>(hint: impl ToString, tokens: &ThemeTokens) -> Element<'a, M> {
    text(raw(hint.to_string()))
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
/// the type scale. `on_press: None` draws it disabled. `label` is catalog
/// text, or data the caller wraps in `raw`.
pub fn dialog_button<'a, M: Clone + 'a>(
    label: impl Into<Label<'a>>,
    kind: DialogButtonKind,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let kind = kind.resolved();
    let disabled = on_press.is_none();
    let color = label_color(kind, disabled, tokens);
    let label: Label<'a> = label.into();
    let (height, size, width) = button_metrics(label.as_str());
    let face = text(label)
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
    if disabled {
        btn.style(disabled_button(kind, tokens)).boxed()
    } else {
        btn.style(dialog_button_style(kind, tokens)).boxed()
    }
}

/// Side of the glyph in front of a [`dialog_icon_button`] label (14 px at
/// the default base).
const BUTTON_GLYPH: f32 = chrome_px(14.0 * 11.0 / 12.0);
/// Gap between that glyph and the label (5 px at the default base).
const BUTTON_GLYPH_GAP: f32 = chrome_px(5.0 * 11.0 / 12.0);

/// [`dialog_button`] with `glyph` (a monochrome SVG, drawn in the label's
/// colour) in front of its label, the button widened by the glyph.
pub fn dialog_icon_button<'a, M: Clone + 'a>(
    glyph: iced::widget::svg::Handle,
    label: impl Into<Label<'a>>,
    kind: DialogButtonKind,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let kind = kind.resolved();
    let disabled = on_press.is_none();
    let color = label_color(kind, disabled, tokens);
    let label: Label<'a> = label.into();
    let (height, size, width) = button_metrics(label.as_str());
    let icon = super::tinted_glyph(glyph, BUTTON_GLYPH, color);
    let face = container(
        row![
            icon,
            text(label)
                .size(size)
                .color(color)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(BUTTON_GLYPH_GAP)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center);
    let btn = button(face)
        .padding(0)
        .width(Length::Fixed(width + BUTTON_GLYPH + BUTTON_GLYPH_GAP))
        .height(Length::Fixed(height))
        .on_press_maybe(on_press);
    if disabled {
        btn.style(disabled_button(kind, tokens)).boxed()
    } else {
        btn.style(dialog_button_style(kind, tokens)).boxed()
    }
}

/// Alpha of a [`primary_dialog_button`]'s ink while it has no action.
const PRIMARY_DISABLED_INK_ALPHA: f32 = 0.7;

/// A dialog's call to action on the theme's primary fill (dark ink in the
/// light theme), as Annotate's Accept (mockup row 75) and Assign
/// Footprints' Apply (mockup row 80) draw it. `face` builds the content in
/// the ink it is given; `on_press: None` fades that ink and the fill.
pub fn primary_dialog_button<'a, M: Clone + 'a>(
    face: impl FnOnce(iced::Color) -> Element<'a, M>,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let ink = tokens::resolve(tokens).primary_ink;
    let ink = if on_press.is_some() {
        ink
    } else {
        iced::Color {
            a: PRIMARY_DISABLED_INK_ALPHA,
            ..ink
        }
    };
    button(face(ink))
        .padding([0.0, SP.l])
        .height(Length::Fixed(ChromeMetrics::DEFAULT.button))
        .on_press_maybe(on_press)
        .style(super::primary_button(tokens))
        .boxed()
}

/// Horizontal label padding of a dialog button, the same for every role.
const DENSE_PAD: f32 = 14.0 * 12.0 / 11.0;

/// `(height, label size, width)` of a dialog button. The buttons are dense:
/// one height and padding for every role, the width hugging the label. The
/// isolation marks catalog text carries around its values are invisible and
/// do not count.
fn button_metrics(label: &str) -> (f32, f32, f32) {
    let chars = label
        .chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .count() as f32;
    let size = TS.label;
    let width = (chars * size * GLYPH_ADVANCE_EM + 2.0 * DENSE_PAD).ceil();
    (ChromeMetrics::DEFAULT.button, size, width)
}

/// Label colour of a dialog button: muted when it has no action, whatever its role.
fn label_color(kind: DialogButtonKind, disabled: bool, tokens: &ThemeTokens) -> iced::Color {
    if disabled {
        ti(tokens.text_secondary)
    } else {
        dialog_label_color(kind, tokens)
    }
}

/// A button without an action: no fill, a half-strength border, the muted
/// label set by [`dialog_button`]. The same for every role, so none reads as
/// clickable, except that a [`DialogButtonKind::Link`] stays borderless text.
fn disabled_button(
    kind: DialogButtonKind,
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let border = if kind == DialogButtonKind::Link {
        iced::Color::TRANSPARENT
    } else {
        iced::Color {
            a: 0.5,
            ..ti(tokens.border)
        }
    };
    let text = ti(tokens.text_secondary);
    move |_: &Theme, _: button::Status| button::Style {
        background: None,
        text_color: text,
        border: dialog_border(border),
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
    container(footer_line(extras, buttons, SP.s))
        .padding(SP.l)
        .width(Length::Fill)
        .boxed()
}

/// Gap between the items of a [`dialog_footer_band`] (8 px at the default
/// base).
const BAND_GAP: f32 = chrome_px(8.0 * 11.0 / 12.0);

/// `extras` at the left end, `buttons` at the right, `gap` between items.
fn footer_line<'a, M: 'a>(
    extras: Vec<Element<'a, M>>,
    buttons: Vec<Element<'a, M>>,
    gap: f32,
) -> iced::widget::Row<Element<'a, M>> {
    let extras = row(extras).spacing(gap).align_y(Alignment::Center);
    let buttons = row(buttons).spacing(gap).align_y(Alignment::Center);
    row![extras, Space::new().width(Length::Fill), buttons].align_y(Alignment::Center)
}

/// [`dialog_footer_split`] on a band: the chrome fill under a hairline,
/// padded by `padding` (the Help cards). In the main window its lower
/// corners follow the card's; in a dialog window it is flat.
pub fn dialog_footer_band<'a, M: 'a>(
    extras: Vec<Element<'a, M>>,
    buttons: Vec<Element<'a, M>>,
    padding: iced::Padding,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let fill = ti(theme_tokens.toolbar_bg);
    let rule = tokens::resolve(theme_tokens).line_soft;
    let corner = super::Radii::GRAPHITE.dialog;
    container(footer_line(extras, buttons, BAND_GAP))
        .padding(padding)
        .width(Length::Fill)
        .style(move |theme: &Theme| {
            let radius = if super::is_dialog_window(theme) {
                iced::border::Radius::default()
            } else {
                iced::border::Radius::default()
                    .bottom_left(corner)
                    .bottom_right(corner)
            };
            let mut border = Border {
                color: rule,
                radius,
                ..Border::default()
            };
            border.top = iced::border::Side::default().width(1.0);
            container::Style {
                background: Some(iced::Background::Color(fill)),
                border,
                ..container::Style::default()
            }
        })
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

/// Vertical padding of a dialog's footer band (10 px at the default base).
const BAND_PAD_Y: f32 = SP.m + SP.xs;

/// Padding of a [`dialog_footer_band`] under a [`dialog_section`] body:
/// the band's own vertical padding, the body's side padding.
pub fn dialog_band_padding() -> iced::Padding {
    iced::Padding::from([BAND_PAD_Y, SP.l])
}

/// Height of a [`dialog_footer_band`] padded by [`dialog_band_padding`]:
/// the dense button and the padding. The hairline is the container's top
/// border, drawn inside the padded bounds, so it takes no layout room.
pub const fn dense_band_footer_height() -> f32 {
    ChromeMetrics::DEFAULT.button + 2.0 * BAND_PAD_Y
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    /// The kit builds from tokens alone for every built-in theme, and the
    /// footer height follows the button height.
    #[test]
    fn kit_builds_for_every_theme() {
        let i18n = signex_i18n::I18n::en_us_ref();
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            for kind in [
                DialogButtonKind::Primary,
                DialogButtonKind::Secondary,
                DialogButtonKind::Danger,
                DialogButtonKind::DangerSolid,
                DialogButtonKind::Accent,
                DialogButtonKind::Ghost,
                DialogButtonKind::Link,
                DialogButtonKind::Quiet,
                DialogButtonKind::Cancel,
            ] {
                let ok = || signex_i18n::t!(i18n, signex_i18n::ids::DIALOG_BUTTON_OK);
                let _: Element<'_, ()> = dialog_button(ok(), kind, Some(()), &tokens);
                let _: Element<'_, ()> = dialog_button(ok(), kind, None, &tokens);
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

    /// The band footer is the dense button and its own padding (its
    /// hairline border takes no layout room), and its sides line up with
    /// the body's (mockup row 23).
    #[test]
    fn the_band_footer_height_follows_its_padding() {
        let pad = dialog_band_padding();
        assert_eq!(pad.left, SP.l);
        assert_eq!(pad.right, SP.l);
        assert_eq!(
            dense_band_footer_height(),
            ChromeMetrics::DEFAULT.button + pad.top + pad.bottom
        );
    }

    /// Cancel is drawn as a Secondary (#2397); no other role changes.
    #[test]
    fn cancel_is_secondary() {
        assert_eq!(
            DialogButtonKind::Cancel.resolved(),
            DialogButtonKind::Secondary
        );
        for kind in [
            DialogButtonKind::Primary,
            DialogButtonKind::Secondary,
            DialogButtonKind::Danger,
            DialogButtonKind::DangerSolid,
            DialogButtonKind::Accent,
            DialogButtonKind::Ghost,
            DialogButtonKind::Link,
            DialogButtonKind::Quiet,
        ] {
            assert_eq!(kind.resolved(), kind);
        }
    }

    /// Mockup row 71: a Link with no action is muted text with no border;
    /// every other role keeps the half-strength border that says "not now".
    #[test]
    fn a_disabled_link_draws_no_border() {
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            let style = |kind| disabled_button(kind, &tokens)(&Theme::Dark, button::Status::Active);
            let link = style(DialogButtonKind::Link);
            assert_eq!(link.border.color, iced::Color::TRANSPARENT, "{id:?}");
            assert_eq!(link.background, None, "{id:?}");
            assert_eq!(link.text_color, ti(tokens.text_secondary), "{id:?}");
            let ghost = style(DialogButtonKind::Ghost);
            assert!(ghost.border.color.a > 0.0, "{id:?}: the Ghost is unchanged");
        }
    }

    /// The isolation marks catalog text puts around a value do not count
    /// toward a button's width; the catalog English is drawn unchanged.
    #[test]
    fn isolation_marks_do_not_widen_a_button() {
        assert_eq!(
            button_metrics("Save \u{2068}R1\u{2069}"),
            button_metrics("Save R1")
        );
        let ok = signex_i18n::t!(
            signex_i18n::I18n::en_us_ref(),
            signex_i18n::ids::DIALOG_BUTTON_OK
        );
        let ok = Label::from(ok);
        assert_eq!(ok.as_str(), "OK");
    }

    /// Footer buttons are dense: one height, the width hugging the label.
    #[test]
    fn graphite_buttons_share_one_height_and_hug_their_label() {
        assert_eq!(button_metrics("Cancel").0, ChromeMetrics::DEFAULT.button);
        let ok = button_metrics("OK").2;
        let long = button_metrics("New Library\u{2026}").2;
        assert!(ok < long, "{ok} < {long}");
        assert!(ok < TS.label * 7.5, "{ok}");
    }

    /// #2397: the Exit dialog's footer laid out. Cancel, Discard All and
    /// Save All are one family: the same height, and the same width for the
    /// same label (so the same padding), whatever the role.
    #[test]
    fn footer_buttons_lay_out_as_one_family() {
        let tokens = signex_types::theme::theme_tokens(ThemeId::BUILTINS[0]);
        let i18n = signex_i18n::I18n::en_us_ref();
        use signex_i18n::{ids, t};
        let footer: Element<'_, ()> = dialog_footer(vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(()),
                &tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_UNSAVED_DISCARD_ALL),
                DialogButtonKind::Danger,
                Some(()),
                &tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_SAVE_ALL),
                DialogButtonKind::Primary,
                Some(()),
                &tokens,
            ),
        ]);
        let texts = crate::layout_probe::lay_out_texts(footer, iced::Size::new(800.0, 200.0));
        let heights: Vec<f32> = ["Cancel", "Discard All", "Save All"]
            .iter()
            .map(|label| crate::layout_probe::find(&texts, label).bounds.height)
            .collect();
        for height in &heights {
            assert_eq!(*height, ChromeMetrics::DEFAULT.button, "{heights:?}");
        }

        let roles = [
            DialogButtonKind::Primary,
            DialogButtonKind::Secondary,
            DialogButtonKind::Danger,
            DialogButtonKind::DangerSolid,
            DialogButtonKind::Accent,
            DialogButtonKind::Ghost,
            DialogButtonKind::Cancel,
        ];
        let buttons: Element<'_, ()> = row(roles
            .iter()
            .enumerate()
            .map(|(i, kind)| {
                dialog_button(
                    signex_i18n::UserText::new(format!("Save {i}")),
                    *kind,
                    Some(()),
                    &tokens,
                )
            })
            .collect::<Vec<_>>())
        .boxed();
        let texts = crate::layout_probe::lay_out_texts(buttons, iced::Size::new(2000.0, 200.0));
        let widths: Vec<f32> = (0..roles.len())
            .map(|i| {
                crate::layout_probe::find(&texts, &format!("Save {i}"))
                    .bounds
                    .width
            })
            .collect();
        assert!(widths.iter().all(|w| *w == widths[0]), "{widths:?}");
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
                for kind in [
                    DialogButtonKind::Primary,
                    DialogButtonKind::Secondary,
                    DialogButtonKind::Danger,
                    DialogButtonKind::DangerSolid,
                    DialogButtonKind::Accent,
                    DialogButtonKind::Ghost,
                    DialogButtonKind::Link,
                    DialogButtonKind::Quiet,
                    DialogButtonKind::Cancel,
                ] {
                    let style = disabled_button(kind, &tokens)(&Theme::Dark, status);
                    assert!(style.background.is_none(), "{id:?} {status:?} {kind:?}");
                    assert_eq!(
                        style.text_color,
                        ti(tokens.text_secondary),
                        "{id:?} {kind:?}"
                    );
                }
            }
            for kind in [
                DialogButtonKind::Primary,
                DialogButtonKind::Secondary,
                DialogButtonKind::Danger,
                DialogButtonKind::DangerSolid,
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
