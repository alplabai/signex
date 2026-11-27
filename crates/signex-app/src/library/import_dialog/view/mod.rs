//! The Import Library dialog (mockup row 211): the header with the step
//! note, the three-step stepper, the step's body and its footer. A pure
//! read of the dialog state and the running import; every colour comes
//! from theme tokens.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text};

use super::{ImportDialogMsg, ImportDialogState, Step, msg};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::header_with_note_closing;
use crate::library_import::run_state::ImportRun;
use crate::styles::{HAIRLINE, PillTone, Radii, Spacing, TypeScale, pill, pill_colors, ti, tokens};

mod preview;
mod progress;
mod source;

#[cfg(test)]
pub(super) use progress::end_words;
#[cfg(test)]
pub(super) use source::blocker_line;

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// Width of a form row's label column (the mock's `.frm .l`).
pub(super) const LABEL_W: f32 = 96.0;
/// Size of a stepper number disc.
const STEP_DISC: f32 = 20.0;

/// What the view reads besides the dialog state.
#[derive(Clone, Copy)]
pub struct ImportDialogContext<'a> {
    /// The import the engine is running, for step 2's and step 3's progress.
    pub running: Option<&'a ImportRun>,
    /// A project is open, so "This project" can be chosen.
    pub project_open: bool,
}

pub fn view<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let title = t!(i18n, ids::DIALOG_TITLE_IMPORT_LIBRARY).into_string();
    let note = step_note(state.step, i18n).into_string();
    let header = header_with_note_closing(
        &title,
        Some(note),
        msg(ImportDialogMsg::Closed),
        windowed,
        tokens,
    );
    let (body, footer) = match state.step {
        Step::Source => (
            source::body(state, ctx, tokens, i18n),
            source::footer(state, tokens, i18n),
        ),
        Step::Preview => (
            preview::body(state, ctx, tokens, i18n),
            preview::footer(state, tokens, i18n),
        ),
        Step::Import => (
            progress::body(state, ctx, tokens, i18n),
            progress::footer(state, tokens, i18n),
        ),
    };
    let content = scrollable(container(body).padding([SP.l, SP.xl]).width(Length::Fill))
        .height(Length::Fill)
        .width(Length::Fill);
    let height = if windowed {
        Length::Fill
    } else {
        Length::Fixed(super::CARD_H)
    };
    container(
        column![header, stepper(state.step, tokens, i18n), content, footer]
            .width(crate::library::card_len(windowed, super::CARD_W))
            .height(height),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

fn step_name(step: Step, i18n: &I18n) -> Localized {
    match step {
        Step::Source => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_STEP_SOURCE),
        Step::Preview => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_STEP_PREVIEW),
        Step::Import => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_STEP_IMPORT),
    }
}

fn step_number(step: Step) -> u32 {
    match step {
        Step::Source => 1,
        Step::Preview => 2,
        Step::Import => 3,
    }
}

/// The header note, "Step 2 of 3 · Preview".
pub(crate) fn step_note(step: Step, i18n: &I18n) -> Localized {
    t!(
        i18n,
        ids::LIBRARY_IMPORT_DIALOG_STEP_NOTE,
        "step" => step_number(step),
        "name" => step_name(step, i18n).into_string()
    )
}

/// The stepper under the header: a disc and a name per step, joined by
/// lines; steps before the current one are done.
fn stepper<'a>(current: Step, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    let r = tokens::resolve(tokens);
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> =
        row![].spacing(SP.m).align_y(Alignment::Center);
    for (i, step) in [Step::Source, Step::Preview, Step::Import]
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            let rule = r.line_soft;
            line = line.push(
                container(Space::new())
                    .width(Length::Fixed(SP.xl * 2.0))
                    .height(Length::Fixed(HAIRLINE))
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(rule)),
                        ..container::Style::default()
                    })
                    .boxed(),
            );
        }
        let n = step_number(step);
        let reached = n <= step_number(current);
        let (fill, ink) = if step == current {
            (ti(tokens.accent), crate::styles::tokens::on_accent(tokens))
        } else if reached {
            (r.accent_soft, r.accent_soft_ink)
        } else {
            (r.sunken, r.ink4)
        };
        let disc = container(
            text(UserText::new(n.to_string()))
                .size(TS.caption)
                .color(ink),
        )
        .center_x(Length::Fixed(STEP_DISC))
        .center_y(Length::Fixed(STEP_DISC))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: (STEP_DISC / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
        let name_ink = if reached { ti(tokens.text) } else { r.ink4 };
        line = line.push(
            row![
                disc,
                text(step_name(step, i18n)).size(TS.body).color(name_ink)
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    container(line)
        .padding([SP.m, SP.xl])
        .width(Length::Fill)
        .boxed()
}

/// A form row: the muted label in its column, the content beside it.
pub(super) fn form_row<'a>(
    label: Localized,
    content: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    row![
        container(text(label).size(TS.body).color(ti(tokens.text_secondary)))
            .width(Length::Fixed(LABEL_W)),
        container(content).width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// A non-pressable chip in `tone` (the mock's `.r211-kind`).
pub(super) fn tag<'a>(
    label: impl Into<Label<'a>>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(text(label).size(TS.caption).wrapping(Wrapping::None))
        .padding([SP.xs, SP.m])
        .style(pill(tokens, tone))
        .boxed()
}

/// A pressable chip: filled in `tone` while `on`, outlined while off.
pub(super) fn chip<'a>(
    label: impl Into<Label<'a>>,
    on: bool,
    tone: PillTone,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (fill, ink) = pill_colors(tokens, tone);
    let (line, off_ink) = (
        tokens::resolve(tokens).line_strong,
        ti(tokens.text_secondary),
    );
    button(text(label).size(TS.caption).wrapping(Wrapping::None))
        .padding([SP.xs, SP.m])
        .on_press(message)
        .style(move |_: &Theme, _| signex_widgets::button::Style {
            background: on.then_some(Background::Color(fill)),
            text_color: if on { ink } else { off_ink },
            border: Border {
                width: if on { 0.0 } else { HAIRLINE },
                color: line,
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..signex_widgets::button::Style::default()
        })
        .boxed()
}

/// User data (a path, a name) in the mono face on one line, cut in the
/// middle when it does not fit.
pub(super) fn mono<'a>(value: String, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let font = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    text(UserText::new(value))
        .size(TS.body)
        .font(font)
        .color(ti(tokens.text))
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::Middle)
        .boxed()
}

/// A muted caption line.
pub(super) fn caption<'a>(
    label: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    text(label)
        .size(TS.caption)
        .color(tokens::muted_text(tokens))
        .boxed()
}

/// A line in the danger ink: why a primary is off.
pub(super) fn problem<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    text(label)
        .size(TS.body)
        .color(tokens::danger(tokens))
        .boxed()
}

/// An outlined small button (Browse…, Change…).
pub(super) fn outline_btn<'a>(
    label: Localized,
    message: Option<LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let mut b = button(text(label).size(TS.body))
        .padding([SP.s, SP.l])
        .style(crate::styles::outline_button(tokens));
    if let Some(message) = message {
        b = b.on_press(message);
    }
    b.boxed()
}

/// One count tile (the mock's `.r211-ct`): the number over its label, the
/// number in `tone`'s ink.
pub(super) fn count_tile<'a>(
    count: u64,
    label: Localized,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ink = match tone {
        PillTone::Neutral => ti(tokens.text),
        other => pill_colors(tokens, other).1,
    };
    let line = ti(tokens.border);
    container(
        column![
            text(UserText::new(count.to_string()))
                .size(TS.title)
                .color(ink),
            caption(label, tokens),
        ]
        .spacing(SP.xs),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
