//! The Alternate part picker card (mockup row 208): the Base part on top,
//! the search over the project libraries, the candidates (library
//! alternates first), the compare strip of the picked one, the scope on a
//! channel tab, and Cancel / Apply to <variant>. A pure view; `overlay`
//! lays it, or the Edit value popover, over the editor.

use iced::widget::{Column, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::project::VariantSettings;
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, radio, text};

use super::VariantPartEditMsg;
use super::candidates::{CandidateSource, same_footprint};
use super::picker_parts::{
    SP, TS, base_card, candidate_row, compare_strip, differs, fill, group_head, head_row,
    list_note, stacked,
};
use super::state::{AlternatePicker, CandidateRead, EditScope, EditedPart, VariantPartEdit};
use crate::app::{Message, Signex};
use crate::styles::tokens::resolve;
use crate::styles::{DialogButtonKind, chrome_px, dialog_button, dialog_search_field, ti};

/// Card width (mockup: 760 px).
const CARD_W: f32 = chrome_px(760.0 * 11.0 / 12.0);
/// Height of the candidate list before it scrolls, in px.
const LIST_H: f32 = chrome_px(240.0);

/// The open edit's card, as the overlay lays it.
pub(crate) enum EditCard<'a> {
    /// The picker, centred over a click-outside layer.
    Centred(Element<'a, Message>),
    /// The popover, at the point the menu opened (window coordinates).
    Anchored(Element<'a, Message>, (f32, f32)),
}

impl Signex {
    /// The open picker or popover, when it opened in `window_id`.
    pub(crate) fn variant_part_edit_card(
        &self,
        window_id: iced::window::Id,
    ) -> Option<EditCard<'_>> {
        let edit = self.ui_state.variant_part_edit.as_deref()?;
        if edit.part().window != self.overlay_owner(window_id) {
            return None;
        }
        let empty = VariantSettings::default();
        let settings = self
            .document_state
            .project_by_id(edit.part().project)
            .map_or(&empty, |project| &project.data.variant_settings);
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        Some(match edit {
            VariantPartEdit::Alternate(picker) => {
                EditCard::Centred(picker_card(picker, settings, i18n, tokens))
            }
            VariantPartEdit::Value(popover) => EditCard::Anchored(
                super::popover::popover_card(popover, settings, i18n, tokens),
                popover.part.anchor,
            ),
        })
    }
}

/// The picker card.
pub(super) fn picker_card<'a>(
    picker: &'a AlternatePicker,
    settings: &VariantSettings,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let part = &picker.part;
    let r = resolve(tokens);
    let where_ = match &part.channel {
        Some(channel) => t!(
            i18n,
            ids::VARIANT_EDIT_WHERE_CHANNEL,
            "part" => part.this_part.designator.as_str(),
            "variant" => part.variant.as_str(),
            "channel" => channel.as_str(),
            "sheet" => part.sheet.as_str()
        ),
        None => t!(
            i18n,
            ids::VARIANT_EDIT_WHERE,
            "part" => part.this_part.designator.as_str(),
            "variant" => part.variant.as_str(),
            "sheet" => part.sheet.as_str()
        ),
    };
    let head = row![
        text(t!(i18n, ids::VARIANT_EDIT_ALTERNATE_TITLE))
            .size(TS.title)
            .font(crate::styles::medium_font())
            .color(ti(tokens.text)),
        text(where_).size(TS.caption).color(r.ink3),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let search = row![
        dialog_search_field(
            t!(i18n, ids::VARIANT_EDIT_SEARCH_PLACEHOLDER),
            &picker.query,
            |query| Message::from(VariantPartEditMsg::QueryEdited(query)),
            None,
            tokens,
        ),
        checkbox(picker.same_footprint_only)
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.title)
            .labelled(t!(i18n, ids::VARIANT_EDIT_SAME_FOOTPRINT))
            .text_size(TS.label)
            .on_toggle(|on| Message::from(VariantPartEditMsg::SameFootprintToggled(on))),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let mut body = vec![
        head.boxed(),
        base_card(part, part.is_fitted(settings), i18n, tokens),
        search.boxed(),
        candidate_list(picker, settings, i18n, tokens),
    ];
    if let Some(picked) = picker.picked.as_ref() {
        let wired = picker.wired.as_deref();
        body.push(compare_strip(part, picked, wired, i18n, tokens));
    }
    if let Some(scope) = scope_choice(part, picker.scope, i18n) {
        body.push(scope);
    }
    if let Some(problem) = &picker.problem {
        body.push(crate::styles::dialog_inline_error(
            text(UserText::new(problem.clone())).size(TS.body).boxed(),
            tokens,
        ));
    }
    body.push(footer(picker, i18n, tokens));
    container(stacked(body).width(Length::Fixed(CARD_W)))
        .padding(SP.l)
        .style(crate::styles::modal_card(tokens))
        .boxed()
}

/// The candidate list: the library alternates, then the search results,
/// each under its heading; a footprint that differs from Base is amber.
fn candidate_list<'a>(
    picker: &'a AlternatePicker,
    settings: &VariantSettings,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let part = &picker.part;
    let designator = part.this_part.designator.as_str();
    let mut rows: Column<Element<'a, Message>> = column![head_row(i18n, tokens)];
    let candidates = match &picker.read {
        CandidateRead::Reading => {
            rows = rows.push(list_note(t!(i18n, ids::VARIANT_EDIT_READING), tokens));
            return framed(rows, tokens);
        }
        CandidateRead::Failed(why) => {
            let note = t!(i18n, ids::VARIANT_EDIT_READ_FAILED, "why" => why.as_str());
            rows = rows.push(list_note(note, tokens));
            return framed(rows, tokens);
        }
        CandidateRead::Read(candidates) => candidates,
    };
    let alternates_head = match &candidates.library {
        Some(library) => t!(
            i18n,
            ids::VARIANT_EDIT_GROUP_ALTERNATES,
            "part" => designator,
            "library" => library.as_str(),
            "count" => candidates.alternates.len()
        ),
        None => t!(i18n, ids::VARIANT_EDIT_GROUP_NO_ROW, "part" => designator),
    };
    rows = rows.push(group_head(alternates_head, tokens));
    let found_head = || {
        if picker.query.trim().is_empty() {
            t!(i18n, ids::VARIANT_EDIT_GROUP_FOUND_EMPTY)
        } else {
            t!(
                i18n,
                ids::VARIANT_EDIT_GROUP_FOUND,
                "query" => picker.query.trim(),
                "shown" => candidates.found.len(),
                "total" => candidates.found_total
            )
        }
    };
    let mut found_headed = false;
    for (at, candidate) in candidates.all().enumerate() {
        if candidate.source == CandidateSource::Search && !found_headed {
            rows = rows.push(group_head(found_head(), tokens));
            found_headed = true;
        }
        let hidden = picker.same_footprint_only
            && !same_footprint(&candidate.footprint, &part.base.footprint);
        if hidden {
            continue;
        }
        let used = used_in(settings, part, &candidate.mpn);
        rows = rows.push(candidate_row(
            candidate,
            at,
            picker.picked.as_ref() == Some(candidate),
            &part.base.footprint,
            used.as_deref(),
            designator,
            i18n,
            tokens,
        ));
    }
    if !found_headed {
        rows = rows.push(group_head(found_head(), tokens));
    }
    if candidates.unread > 0 {
        let note = t!(i18n, ids::VARIANT_EDIT_UNREAD, "count" => candidates.unread);
        rows = rows.push(list_note(note, tokens));
    }
    framed(rows, tokens)
}

/// The variant other than the edited one that already uses `mpn` for the
/// part, if any.
fn used_in(settings: &VariantSettings, part: &EditedPart, mpn: &str) -> Option<String> {
    settings
        .parts
        .iter()
        .filter(|entry| !entry.variant.eq_ignore_ascii_case(&part.variant))
        .filter(|entry| entry.is_for(part.this_part.placed()))
        .find(|entry| {
            entry
                .parameters
                .get(signex_types::project::MPN_PARAMETER)
                .is_some_and(|used| used.trim().eq_ignore_ascii_case(mpn.trim()))
        })
        .map(|entry| entry.variant.clone())
}

/// `rows` in a bordered list that scrolls past [`LIST_H`].
fn framed<'a>(rows: Column<Element<'a, Message>>, tokens: &ThemeTokens) -> Element<'a, Message> {
    let edge = ti(tokens.border);
    container(
        scrollable(rows)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .height(Length::Fixed(LIST_H)),
    )
    .width(Length::Fill)
    .style(move |_: &iced::Theme| container::Style {
        border: iced::Border {
            width: 1.0,
            color: edge,
            radius: crate::styles::Radii::GRAPHITE.button.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The scope on a channel tab (mockup rows 208, 209): this channel only or
/// all channels; `None` elsewhere, where the edit applies to the part shown
/// or, on Editor, to every channel.
pub(super) fn scope_choice<'a>(
    part: &EditedPart,
    scope: EditScope,
    i18n: &I18n,
) -> Option<Element<'a, Message>> {
    if !part.has_scope() {
        return None;
    }
    let channel = part.channel.as_deref().unwrap_or_default();
    let (first, last) = part.channels.clone().unwrap_or_default();
    let option = |label, value: EditScope| -> Element<'a, Message> {
        radio(label, value, Some(scope), |picked| {
            Message::from(VariantPartEditMsg::ScopePicked(picked))
        })
        .size(TS.heading)
        .spacing(SP.s)
        .text_size(TS.body)
        .boxed()
    };
    Some(
        row![
            text(t!(i18n, ids::VARIANT_EDIT_SCOPE)).size(TS.caption),
            option(
                t!(i18n, ids::VARIANT_EDIT_SCOPE_THIS, "channel" => channel),
                EditScope::ThisChannel
            ),
            option(
                t!(i18n, ids::VARIANT_EDIT_SCOPE_ALL, "first" => first, "last" => last),
                EditScope::AllChannels
            ),
        ]
        .spacing(SP.l)
        .align_y(Alignment::Center)
        .boxed(),
    )
}

/// The caption (one undo step, or the pins and footprint warning), Cancel
/// and Apply to <variant>, live once a part is picked.
fn footer<'a>(picker: &AlternatePicker, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let warns = picker
        .picked
        .as_ref()
        .is_some_and(|picked| differs(&picker.part, picked));
    let caption = if warns {
        text(t!(i18n, ids::VARIANT_EDIT_FOOT_DIFFERS))
            .size(TS.caption)
            .color(ti(tokens.warning))
    } else {
        text(t!(i18n, ids::VARIANT_EDIT_FOOT_ONE_STEP))
            .size(TS.caption)
            .color(resolve(tokens).ink4)
    };
    let apply = picker
        .picked
        .is_some()
        .then(|| Message::from(VariantPartEditMsg::ApplyPressed));
    row![
        caption,
        fill(),
        dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(Message::from(VariantPartEditMsg::CancelPressed)),
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::VARIANT_EDIT_APPLY, "variant" => picker.part.variant.as_str()),
            DialogButtonKind::Primary,
            apply,
            tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}
