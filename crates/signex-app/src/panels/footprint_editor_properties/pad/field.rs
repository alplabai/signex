//! Row chrome of the Graphite pad form: a labelled row, a pair of fields,
//! the segmented control and a number field that shows refused text with a
//! red outline and a one-line reason (CX review 2026-11-19).

use iced::widget::{Column, container, row};
use iced::{Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, SegmentedStyle, text, text_input};

use super::super::super::PanelMsg;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Why the text in a pad field was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refusal {
    NotANumber,
    NotPositive,
    /// The size leaves a trapezoid with no copper.
    NoCopper,
}

impl Refusal {
    /// Why `text` was not taken by a field: a number at all, above zero
    /// when `positive`, and accepted by `fits`. `None` when it would be
    /// taken (a stale entry shows as plain text).
    pub(super) fn of(text: &str, positive: bool, fits: impl Fn(f64) -> bool) -> Option<Self> {
        let Some(value) = crate::numeric_field::parse_field_f64(text) else {
            return Some(Refusal::NotANumber);
        };
        if positive && value <= 0.0 {
            Some(Refusal::NotPositive)
        } else if !fits(value) {
            Some(Refusal::NoCopper)
        } else {
            None
        }
    }

    fn message(self) -> MessageId {
        match self {
            Refusal::NotANumber => ids::EDITORS_FP_PAD_INVALID_NUMBER,
            Refusal::NotPositive => ids::EDITORS_FP_PAD_INVALID_POSITIVE,
            Refusal::NoCopper => ids::EDITORS_FP_PAD_INVALID_NO_COPPER,
        }
    }
}

/// What a number field shows: the refused text while there is one, else
/// the stored value.
pub(super) struct Shown {
    pub text: String,
    pub refusal: Option<Refusal>,
}

impl Shown {
    pub(super) fn value(text: String) -> Self {
        Self {
            text,
            refusal: None,
        }
    }

    /// `buffer` (refused text) when present, else `stored`.
    pub(super) fn from_buffer(
        buffer: Option<&String>,
        stored: String,
        positive: bool,
        fits: impl Fn(f64) -> bool,
    ) -> Self {
        match buffer {
            Some(text) => Self {
                refusal: Refusal::of(text, positive, fits),
                text: text.clone(),
            },
            None => Self::value(stored),
        }
    }
}

/// The segmented control look of the pad form, from the theme tokens.
pub(super) fn segment_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = resolve(tokens);
    SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(tokens.text),
        TS.caption,
    )
}

/// A number field showing `shown`, outlined in the error ink while its
/// text is refused.
pub(super) fn number_input<'a>(
    shown: &Shown,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let error = shown.refusal.map(|_| ti(tokens.error));
    text_input(UserText::new(String::new()), shown.text.clone())
        .size(TS.caption)
        .width(Length::Fill)
        .padding(SP.xs)
        .style(move |theme: &Theme, status| {
            let mut style = crate::panels::form_look::text_input_style(theme, status);
            if let Some(error) = error {
                style.border.color = error;
            }
            style
        })
        .on_input(on_input)
        .boxed()
}

/// The one-line reason under a refused field, in the error ink.
pub(super) fn refusal_line<'a>(
    refusal: Refusal,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    container(
        text(t!(i18n, refusal.message()))
            .size(TS.caption)
            .color(ti(tokens.error)),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// `label` on the left and `content` in the value column, the pad form's
/// row chrome; `fw` are the label and value widths.
pub(super) fn labelled_row<'a>(
    label: impl Into<Label<'a>>,
    content: Element<'a, PanelMsg>,
    fw: (Length, Length),
    muted: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(label).size(TS.caption).color(muted).width(fw.0),
            container(content).width(fw.1),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// Two fields side by side, sharing the value column.
pub(super) fn pair<'a>(
    a: Element<'a, PanelMsg>,
    b: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![a, b].spacing(SP.s).width(Length::Fill).boxed()
}

/// `row` with the reasons of its refused fields under it.
pub(super) fn with_refusals<'a>(
    row: Element<'a, PanelMsg>,
    refusals: &[Option<Refusal>],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill).push(row);
    for refusal in refusals.iter().flatten() {
        col = col.push(refusal_line(*refusal, tokens, i18n));
    }
    col.boxed()
}

/// A read-only value cell (a rule value, a position) on the sunken fill.
pub(super) fn read_only_cell<'a>(
    label: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let ink = resolve(tokens).ink3;
    container(
        text(label)
            .size(TS.caption)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding(SP.xs)
    .width(Length::Fill)
    .clip(true)
    .style(crate::styles::read_only_field(tokens))
    .boxed()
}

/// A caption line in the faint ink, inset like the rows.
pub(super) fn caption_line<'a>(label: impl Into<Label<'a>>, ink: Color) -> Element<'a, PanelMsg> {
    container(text(label).size(TS.caption).color(ink))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::Refusal;

    #[test]
    fn a_refusal_names_why_the_text_was_not_taken() {
        let any = |_: f64| true;
        assert_eq!(Refusal::of("1,5", true, any), Some(Refusal::NotANumber));
        assert_eq!(Refusal::of("", true, any), Some(Refusal::NotANumber));
        assert_eq!(Refusal::of("0", true, any), Some(Refusal::NotPositive));
        assert_eq!(Refusal::of("-0.05", false, any), None);
        assert_eq!(
            Refusal::of("0.4", true, |v| v > 1.0),
            Some(Refusal::NoCopper)
        );
        assert_eq!(Refusal::of("2", true, |v| v > 1.0), None);
    }
}
