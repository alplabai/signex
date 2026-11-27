//! Change Component in two steps (mockup row 129, #3018):
//!
//! ```text
//! ┌─[Change Component   R5 · buck_channel.snxsch ─────────────────── X]─┐
//! │ [⌕ Search parts                           ] │ [SYMBOL             ] │
//! │ (Same symbol)(Same value)(In stock)         │                       │
//! │ RC0603FR-0731K6L                same pins   │ [FOOTPRINT          ] │
//! │ YC164-FR-0731K6L           4 pins, not 2    │                       │
//! ├─────────────────────────────────────────────┴───────────────────────┤
//! │ Pick a part in the list to compare it …      [ Cancel ] [ Next ]    │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Next leads to the comparison: Placed now › Replace with, the Now /
//! After table, what is kept and Apply to, with Back, Cancel and
//! "Change N parts". Pure: the session, the comparison, the step and the
//! hidden rows are built in `update` (`app::handlers::placed_part`).

mod compare;
mod list;

use iced::widget::{column, container, row};
use iced::{Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::library::PickerState;
use crate::library::browser::PreviewLook;
use crate::library::messages::{LibraryMessage, PlacedPartMsg};
use crate::library::modal_chrome::{
    cancel_btn, footer_strip, header_with_note_closing, primary_btn_if, secondary_btn,
};
use crate::library::state::{ChangePreview, ChangeSession, ChangeStep, PickerMode};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// The card as mockup row 129 draws it, the same size on both steps.
const CARD_W: f32 = 880.0;
const CARD_H: f32 = 600.0;

/// The session the dialog draws and the comparison, when a part is read.
fn stage(mode: &PickerMode) -> Option<(&ChangeSession, Option<&ChangePreview>)> {
    match mode {
        PickerMode::ChangeComponent(session) => Some((&**session, None)),
        PickerMode::ChangePreview(change) => Some((&change.session, Some(&**change))),
        PickerMode::Place | PickerMode::Datasheets(_) => None,
    }
}

/// Whether Next and Change act: a comparison is read and the list still
/// shows and picks the part it compares.
fn can_apply(picker: &PickerState, change: Option<&ChangePreview>) -> bool {
    let shown = picker.shown_selection().map(|(_, summary)| summary.row_id);
    change.is_some_and(|change| change.compares_shown(shown))
}

/// The dialog; `None` outside the Change Component stages.
pub(crate) fn view<'a>(
    picker: &'a PickerState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    windowed: bool,
    look: PreviewLook,
) -> Option<Element<'a, LibraryMessage>> {
    let (session, change) = stage(&picker.mode)?;
    let note = t!(
        i18n,
        ids::LIBRARY_CHANGE_COMPONENT_NOTE,
        "reference" => session.reference.as_str(),
        "sheet" => session.sheet_name.as_str()
    )
    .into_string();
    let header = header_with_note_closing(
        &crate::library::picker::title(&picker.mode, i18n),
        Some(note),
        LibraryMessage::ClosePicker,
        windowed,
        tokens,
    );
    let compared = change.filter(|c| c.step == ChangeStep::Compare);
    let (body, footer) = match compared {
        Some(change) => (
            compare::view(change, tokens, i18n),
            compare_footer(picker, change, tokens, i18n),
        ),
        None => (
            pick_body(picker, session, look, tokens, i18n),
            pick_footer(picker, session, change, tokens, i18n),
        ),
    };
    let body = container(body).width(Length::Fill).height(Length::Fill);
    Some(
        container(
            column![header, body, footer]
                .width(crate::library::card_len(windowed, CARD_W))
                .height(crate::library::card_len(windowed, CARD_H)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed(),
    )
}

/// Step 1: the list, a hairline, and the picked row's Symbol and Footprint
/// tiles (the Place Component preview column).
fn pick_body<'a>(
    picker: &'a PickerState,
    session: &'a ChangeSession,
    look: PreviewLook,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let rule = crate::styles::tokens::resolve(tokens).line_soft;
    let left = container(list::view(picker, session, tokens, i18n))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().right(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        });
    let tiles = crate::library::picker::preview_column(picker, look, tokens, i18n);
    row![left, tiles].height(Length::Fill).boxed()
}

/// Step 1's footer: the pick prompt, Cancel and Next, enabled once the
/// picked row is read.
fn pick_footer<'a>(
    picker: &PickerState,
    session: &ChangeSession,
    change: Option<&ChangePreview>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let prompt = t!(
        i18n,
        ids::LIBRARY_CHANGE_COMPONENT_PICK_PROMPT,
        "reference" => session.reference.as_str()
    );
    let caption = text(prompt).size(TS.caption).color(r.ink4).boxed();
    footer_strip(
        vec![caption],
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::ClosePicker,
                tokens,
            ),
            primary_btn_if(
                t!(i18n, ids::LIBRARY_PICKER_NEXT),
                LibraryMessage::PlacedPart(PlacedPartMsg::ChangeNextPressed),
                can_apply(picker, change),
                tokens,
            ),
        ],
        tokens,
    )
}

/// Step 2's footer: the caption, Back, Cancel and "Change N parts" (with
/// "anyway" when the change loses connections).
fn compare_footer<'a>(
    picker: &PickerState,
    change: &ChangePreview,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let caption = text(t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_FOOTER))
        .size(TS.caption)
        .color(r.ink4)
        .boxed();
    let label = t!(
        i18n,
        if change.preview.is_clean() {
            ids::LIBRARY_CHANGE_COMPONENT_APPLY
        } else {
            ids::LIBRARY_CHANGE_COMPONENT_APPLY_ANYWAY
        },
        "count" => change.part_count()
    );
    footer_strip(
        vec![caption],
        vec![
            secondary_btn(
                t!(i18n, ids::DIALOG_BUTTON_BACK),
                LibraryMessage::PlacedPart(PlacedPartMsg::ChangeBackPressed),
                tokens,
            ),
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::ClosePicker,
                tokens,
            ),
            primary_btn_if(
                label,
                LibraryMessage::PlacedPart(PlacedPartMsg::ChangeConfirmed),
                can_apply(picker, Some(change)),
                tokens,
            ),
        ],
        tokens,
    )
}

#[cfg(test)]
mod tests;
