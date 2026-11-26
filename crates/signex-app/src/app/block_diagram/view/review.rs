//! The Re-sync review side sheet (mockup row 210): what the Re-sync applied,
//! one checked row per block that would become unlinked, and Apply selected
//! (one undo step) or Cancel; "Nothing to change" when a Re-sync run while
//! it is open finds no block to unlink. It takes the place of Properties.
//! A pure view; its events are the tab's (`TabEvent::Review`).

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{CheckboxLabel as _, button, checkbox, text};

use super::{Look, SP, TS, caption};
use crate::app::block_diagram::state::ResyncReview;
use crate::app::block_diagram::{ReviewEvent, TabEvent};
use crate::styles::tokens::resolve;
use crate::styles::{DialogButtonKind, dialog_button, ti};

type Items<'a> = Column<Element<'a, TabEvent>>;

fn sent(event: ReviewEvent) -> TabEvent {
    TabEvent::Review(event)
}

fn note<'a>(look: Look<'a>, line: impl Into<signex_widgets::Label<'a>>) -> Element<'a, TabEvent> {
    text(line)
        .size(TS.dense)
        .color(ti(look.tokens.text_secondary))
        .boxed()
}

/// The heading with its close button.
fn heading<'a>(look: Look<'a>) -> Element<'a, TabEvent> {
    let close = button(text(look.t(ids::BLOCK_DIAGRAM_REVIEW_CLOSE_MARK)).size(TS.body))
        .padding([0.0, SP.s])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(sent(ReviewEvent::Closed));
    row![
        text(look.t(ids::BLOCK_DIAGRAM_REVIEW_TITLE))
            .size(TS.label)
            .color(ti(look.tokens.text))
            .width(Length::Fill)
            .boxed(),
        close.boxed(),
    ]
    .align_y(Alignment::Center)
    .boxed()
}

/// The side sheet for `review`.
pub(super) fn view<'a>(look: Look<'a>, review: &'a ResyncReview) -> Element<'a, TabEvent> {
    let mut list: Items<'a> = column![
        heading(look),
        note(look, look.t(ids::BLOCK_DIAGRAM_REVIEW_COMPARED))
    ]
    .spacing(SP.s);
    list = list.push(note(
        look,
        t!(look.i18n, ids::BLOCK_DIAGRAM_REVIEW_SUMMARY,
            "added" => review.added.len(),
            "moved" => review.moved.len(),
            "unlinked" => review.unlink.len(),
            "unchanged" => review.unchanged),
    ));
    if review.unlink.is_empty() {
        return sheet(look, nothing(look, list));
    }
    if !review.added.is_empty() || !review.moved.is_empty() {
        list = list.push(note(look, look.t(ids::BLOCK_DIAGRAM_REVIEW_APPLIED_NOTE)));
    }
    let all = review.unlink.iter().all(|row| row.checked);
    list = list.push(
        checkbox(all)
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.body)
            .labelled(t!(look.i18n, ids::BLOCK_DIAGRAM_REVIEW_UNLINKED_GROUP,
                "count" => review.unlink.len()))
            .text_size(TS.dense)
            .on_toggle(|on| sent(ReviewEvent::AllToggled(on)))
            .boxed(),
    );
    for unlink in &review.unlink {
        let block = unlink.block;
        let what = if unlink.checked {
            ids::BLOCK_DIAGRAM_REVIEW_UNLINK_ON
        } else {
            ids::BLOCK_DIAGRAM_REVIEW_UNLINK_OFF
        };
        let body = column![
            checkbox(unlink.checked)
                .style(crate::panels::form_look::checkbox_style)
                .size(TS.body)
                .labelled(UserText::new(unlink.name.clone()))
                .text_size(TS.dense)
                .on_toggle(move |on| sent(ReviewEvent::RowToggled(block, on)))
                .boxed(),
            text(UserText::new(unlink.path.clone()))
                .size(TS.dense)
                .font(iced::Font::MONOSPACE)
                .color(resolve(look.tokens).ink3)
                .boxed(),
            note(look, look.t(what)),
        ]
        .spacing(SP.xs);
        list = list.push(container(body).padding([0.0, SP.m]).boxed());
    }
    list = list.push(note(look, look.t(ids::BLOCK_DIAGRAM_REVIEW_MANUAL)));
    let checked = review.unlink.iter().filter(|row| row.checked).count();
    list = list.push(note(
        look,
        t!(look.i18n, ids::BLOCK_DIAGRAM_REVIEW_SELECTED,
            "checked" => checked, "total" => review.unlink.len()),
    ));
    let buttons = row![
        Space::new().width(Length::Fill),
        dialog_button(
            look.t(ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(sent(ReviewEvent::Closed)),
            look.tokens,
        ),
        dialog_button(
            look.t(ids::BLOCK_DIAGRAM_REVIEW_APPLY),
            DialogButtonKind::Primary,
            Some(sent(ReviewEvent::ApplyPressed)),
            look.tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed();
    sheet(look, list.push(buttons))
}

/// "Nothing to change", with Close.
fn nothing<'a>(look: Look<'a>, list: Items<'a>) -> Items<'a> {
    let close = dialog_button(
        look.t(ids::BLOCK_DIAGRAM_REVIEW_CLOSE),
        DialogButtonKind::Primary,
        Some(sent(ReviewEvent::Closed)),
        look.tokens,
    );
    list.push(caption(look, ids::BLOCK_DIAGRAM_REVIEW_NOTHING))
        .push(note(look, look.t(ids::BLOCK_DIAGRAM_REVIEW_NOTHING_NOTE)))
        .push(
            row![Space::new().width(Length::Fill), close]
                .align_y(Alignment::Center)
                .boxed(),
        )
}

fn sheet<'a>(look: Look<'a>, list: Items<'a>) -> Element<'a, TabEvent> {
    container(scrollable(list.padding(SP.s)).height(Length::Fill))
        .height(Length::Fill)
        .style(crate::styles::panel_region(look.tokens))
        .boxed()
}
