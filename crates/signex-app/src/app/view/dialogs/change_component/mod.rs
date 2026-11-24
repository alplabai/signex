//! Change Component, one dialog (mockup row 155, DECISIONS 2026-11-19):
//!
//! ```text
//! ┌─[Change Component   R5 · buck_channel.snxsch ─────────────────── X]─┐
//! │ [⌕ Search parts       ] │ [Placed now] › [Replace with]             │
//! │ (Same symbol)(Same value│ PARAMETERS  Parameter Now  After  Keep now│
//! │ RC0603FR-0731K6L  same  │ Kept: Designator R5 · …                   │
//! │ YC164-FR-…  4 pins, not │ Apply to: (•) Only R5  ( ) Every …        │
//! ├─────────────────────────┴───────────────────────────────────────────┤
//! │ One undo step. …                    [ Cancel ] [ Change 1 part ]    │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Pure: the session, the comparison and the hidden rows are built in
//! `update` (`app::handlers::placed_part`).

mod compare;
mod list;

use iced::widget::{column, container, row};
use iced::{Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::library::PickerState;
use crate::library::messages::{LibraryMessage, PlacedPartMsg};
use crate::library::modal_chrome::{
    cancel_btn, footer_strip, header_with_note_closing, primary_btn_if,
};
use crate::library::state::{ChangePreview, ChangeSession, PickerMode};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// The card as mockup row 155 draws it.
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

/// Whether Change applies: a comparison is shown and the list still shows
/// and picks the part it compares.
fn can_apply(picker: &PickerState, change: Option<&ChangePreview>) -> bool {
    let Some(change) = change else {
        return false;
    };
    picker.shown_selection().is_some_and(|(_, summary)| {
        summary.row_id == change.resolved.row_id && !change.session.hidden.contains(&summary.row_id)
    })
}

/// The dialog; `None` outside the Change Component stages.
pub(crate) fn view<'a>(
    picker: &'a PickerState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    windowed: bool,
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
    let r = crate::styles::tokens::resolve(tokens);
    let rule = r.line_soft;
    let left = container(list::view(picker, session, tokens, i18n))
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().right(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        });
    let right = container(compare::view(session, change, tokens, i18n))
        .width(Length::FillPortion(5))
        .height(Length::Fill);
    let body = row![left, right].height(Length::Fill);
    let footer = footer(picker, change, tokens, i18n);
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

/// The caption, Cancel and "Change N parts" (with "anyway" when the change
/// loses connections).
fn footer<'a>(
    picker: &PickerState,
    change: Option<&ChangePreview>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let caption = text(t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_FOOTER))
        .size(TS.caption)
        .color(r.ink4)
        .boxed();
    let count = change.map_or(1, ChangePreview::part_count);
    let clean = change.is_none_or(|c| c.preview.is_clean());
    let label = t!(
        i18n,
        if clean {
            ids::LIBRARY_CHANGE_COMPONENT_APPLY
        } else {
            ids::LIBRARY_CHANGE_COMPONENT_APPLY_ANYWAY
        },
        "count" => count
    );
    footer_strip(
        vec![caption],
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::ClosePicker,
                tokens,
            ),
            primary_btn_if(
                label,
                LibraryMessage::PlacedPart(PlacedPartMsg::ChangeConfirmed),
                can_apply(picker, change),
                tokens,
            ),
        ],
        tokens,
    )
}

#[cfg(test)]
mod tests;
