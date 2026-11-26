//! The card of a stale attachment in the margin lane (mockup row 199):
//! what was taken when, which part value changed, the readings taken with
//! it, and Re-measure…, Mark still valid and Show change.

use iced::widget::{Column, Row, container};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_notebook::report::AttachmentState;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::{GAP, PAD, TS};
use crate::styles::{DialogButtonKind, dialog_button, ti, tokens::resolve};

/// What the card's buttons publish.
pub(crate) struct StaleActions<M> {
    /// Re-measure…: a new reading that supersedes the stale ones.
    pub(crate) remeasure: M,
    /// Mark still valid: store the current values in the attachment.
    pub(crate) mark_valid: M,
    /// Show change: go to the part whose value changed.
    pub(crate) show_change: M,
}

/// The card of `state`; `None` when it is not stale.
pub(crate) fn stale_card<'a, M: Clone + 'a>(
    state: &'a AttachmentState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    actions: StaleActions<M>,
) -> Option<Element<'a, M>> {
    if !state.is_stale() {
        return None;
    }
    let r = resolve(tokens);
    let warn = ti(tokens.warning);
    let attachment = &state.attachment;
    let name = attachment.path_in_notebook();
    let mut card = Column::<Element<'a, M>>::new()
        .spacing(GAP)
        .width(Length::Fill)
        .push(
            text(t!(i18n, ids::NOTEBOOK_STALE_TITLE, "name" => name))
                .size(TS.body)
                .color(warn)
                .boxed(),
        )
        .push(
            text(t!(i18n, ids::NOTEBOOK_STALE_WHY))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    if let Some(taken) = attachment.taken {
        let date = taken.to_string();
        let line = if attachment.rev.is_empty() {
            t!(i18n, ids::NOTEBOOK_STALE_TAKEN, "date" => date)
        } else {
            let rev = attachment.rev.as_str();
            t!(i18n, ids::NOTEBOOK_STALE_TAKEN_REV, "date" => date, "rev" => rev)
        };
        card = card.push(text(line).size(TS.caption).color(ti(tokens.text)).boxed());
    }
    for change in &state.changes {
        let line = match &change.now {
            Some(now) => t!(
                i18n, ids::NOTEBOOK_STALE_CHANGED,
                "part" => change.part.as_str(),
                "was" => change.was.as_str(),
                "now" => now.as_str()
            ),
            None => t!(
                i18n, ids::NOTEBOOK_STALE_GONE,
                "part" => change.part.as_str(),
                "was" => change.was.as_str()
            ),
        };
        card = card.push(text(line).size(TS.caption).color(ti(tokens.text)).boxed());
    }
    if !state.readings.is_empty() {
        let readings: Vec<String> = state.readings.iter().map(ToString::to_string).collect();
        card = card.push(
            Row::new()
                .spacing(GAP)
                .push(
                    text(t!(i18n, ids::NOTEBOOK_STALE_ALSO))
                        .size(TS.caption)
                        .color(r.ink3),
                )
                .push(
                    text(UserText::new(readings.join(", ")))
                        .size(TS.caption)
                        .color(warn),
                )
                .boxed(),
        );
    }
    let buttons = Row::new()
        .spacing(GAP)
        .push(dialog_button(
            t!(i18n, ids::NOTEBOOK_STALE_REMEASURE),
            DialogButtonKind::Primary,
            Some(actions.remeasure),
            tokens,
        ))
        .push(dialog_button(
            t!(i18n, ids::NOTEBOOK_STALE_MARK_VALID),
            DialogButtonKind::Secondary,
            Some(actions.mark_valid),
            tokens,
        ))
        .push(dialog_button(
            t!(i18n, ids::NOTEBOOK_STALE_SHOW_CHANGE),
            DialogButtonKind::Secondary,
            Some(actions.show_change),
            tokens,
        ));
    card = card.push(buttons.boxed()).push(
        text(t!(i18n, ids::NOTEBOOK_STALE_NOTE))
            .size(TS.caption)
            .color(r.ink3)
            .boxed(),
    );
    let fill = r.warn_soft;
    Some(
        container(card)
            .padding(PAD)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    color: warn,
                    width: 1.0,
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}
