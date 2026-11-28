//! The Item pane's link (mockup row 206, "Linked to"): a card with what the
//! item links to and what it says now, Open (Open in Requirements for a
//! requirement, the results for ERC and DRC, Parameters for the title block,
//! Show for a design object, which cross-probes) and Run check; then "Link a
//! design object or check…", which opens the link picker: a filter box over
//! the checks, the project's requirements and the parts, nets, sheets and
//! board of its open documents. Typed text that matches none links to it as
//! a design object.

use iced::widget::{Column, Row, column, container};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_types::project::{ChecklistItem, ChecklistLink};
use signex_widgets::{text, text_input};

use super::super::look::{ButtonKind, Glyph, field_label, pill, text_button, ui_font};
use super::super::table::{SP, TS, note};
use super::links::{ChoiceGroup, LinkChoice};
use super::view::{Ctx, result_tone};
use super::words::{link_words, result_words};
use super::{ChecklistMsg, message};
use crate::app::Message;
use crate::styles::{HAIRLINE, Radii, ti};

/// The most picker rows shown at once; the filter box narrows the rest.
const PICKER_ROWS: usize = 40;

/// The words of a picker group's heading.
fn group_words(group: ChoiceGroup) -> MessageId {
    match group {
        ChoiceGroup::Check => ids::CHECKLIST_GROUP_CHECKS,
        ChoiceGroup::Requirement => ids::CHECKLIST_GROUP_REQUIREMENTS,
        ChoiceGroup::Part => ids::CHECKLIST_GROUP_PARTS,
        ChoiceGroup::Net => ids::CHECKLIST_GROUP_NETS,
        ChoiceGroup::Sheet => ids::CHECKLIST_GROUP_SHEETS,
        ChoiceGroup::Board => ids::CHECKLIST_GROUP_BOARD,
    }
}

/// The words of the card's Open, by what the link is to.
fn open_words(link: &ChecklistLink) -> MessageId {
    match link {
        ChecklistLink::Requirement { .. } => ids::CHECKLIST_OPEN_IN_REQUIREMENTS,
        ChecklistLink::Erc | ChecklistLink::Drc => ids::CHECKLIST_OPEN_RESULTS,
        ChecklistLink::TitleBlock => ids::CHECKLIST_OPEN_PARAMETERS,
        ChecklistLink::Object { .. } => ids::CHECKLIST_SHOW_OBJECT,
    }
}

/// The card of `link`.
fn card<'a>(ctx: &Ctx, item: &ChecklistItem, link: &ChecklistLink) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let mut head: Row<Element<'a, Message>> =
        Row::new().spacing(SP.s).align_y(Alignment::Center).push(
            text(link_words(Some(link), i18n))
                .size(TS.body)
                .font(ui_font(iced::font::Weight::Semibold))
                .color(ti(tokens.text))
                .boxed(),
        );
    if let Some(result) = ctx.result_of(item)
        && let Some(words) = result_words(result, i18n)
    {
        head = head.push(pill(words, result_tone(result), None, tokens));
    }
    let open = ctx.send(ChecklistMsg::LinkOpenPressed {
        item: item.id.clone(),
    });
    let mut actions: Row<Element<'a, Message>> = Row::new().spacing(SP.s).push(text_button(
        None,
        t!(i18n, open_words(link)),
        Some(open),
        ButtonKind::Small,
        tokens,
    ));
    if link.run_id().is_some() {
        let running = ctx.state.is_some_and(|state| state.running);
        let run = (!running).then(|| {
            ctx.send(ChecklistMsg::ItemRunPressed {
                item: item.id.clone(),
            })
        });
        actions = actions.push(text_button(
            Some(Glyph::Play),
            t!(i18n, ids::CHECKLIST_RUN_CHECK),
            run,
            ButtonKind::Small,
            tokens,
        ));
    }
    let clear = ctx.send(ChecklistMsg::LinkPicked {
        item: item.id.clone(),
        link: String::new(),
    });
    actions = actions.push(text_button(
        None,
        t!(i18n, ids::CHECKLIST_LINK_CLEAR),
        Some(clear),
        ButtonKind::Ghost,
        tokens,
    ));
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(column![head, actions].spacing(SP.s))
        .width(Length::Fill)
        .padding(SP.m)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Does `choice` match the picker's filter text `needle` (lower case)?
fn is_match(ctx: &Ctx, choice: &LinkChoice, needle: &str) -> bool {
    needle.is_empty()
        || choice.link.to_text().to_lowercase().contains(needle)
        || link_words(Some(&choice.link), ctx.i18n)
            .to_string()
            .to_lowercase()
            .contains(needle)
}

/// The link typed text names: a link's own text form, else a design object.
fn typed_link(query: &str) -> Option<ChecklistLink> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    match ChecklistLink::parse(query) {
        Ok(Some(link)) => Some(link),
        _ => Some(ChecklistLink::Object {
            target: query.to_string(),
        }),
    }
}

/// The picker: its filter box and the rows that match, under their group's
/// heading.
fn picker<'a>(ctx: &Ctx, item: &ChecklistItem) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let query = ctx
        .state
        .map(|state| state.link_query.clone())
        .unwrap_or_default();
    let project_id = ctx.project_id;
    let input = text_input(t!(i18n, ids::CHECKLIST_LINK_SEARCH), query.clone())
        .on_input(move |text| message(project_id, ChecklistMsg::LinkQueryEdited(text)))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fill);
    let mut rows: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.xs)
        .push(input.boxed())
        .push(note(t!(i18n, ids::CHECKLIST_FIELD_LINK_HINT), tokens));
    let pick = |link: &ChecklistLink| {
        ctx.send(ChecklistMsg::LinkPicked {
            item: item.id.clone(),
            link: link.to_text(),
        })
    };
    let needle = query.trim().to_lowercase();
    let mut group = None;
    let shown = ctx
        .choices
        .iter()
        .filter(|choice| is_match(ctx, choice, &needle))
        .take(PICKER_ROWS);
    for choice in shown {
        if group != Some(choice.group) {
            group = Some(choice.group);
            rows = rows.push(field_label(t!(i18n, group_words(choice.group)), tokens));
        }
        rows = rows.push(text_button(
            None,
            link_words(Some(&choice.link), i18n),
            Some(pick(&choice.link)),
            ButtonKind::Ghost,
            tokens,
        ));
    }
    if let Some(link) = typed_link(&query)
        && !ctx.choices.iter().any(|choice| choice.link == link)
    {
        let what = link.to_text();
        let words = t!(i18n, ids::CHECKLIST_LINK_TYPED, "what" => what.as_str());
        rows = rows.push(text_button(
            Some(Glyph::Plus),
            words,
            Some(pick(&link)),
            ButtonKind::Ghost,
            tokens,
        ));
    }
    rows.boxed()
}

/// The pane's "Linked to" block.
pub(super) fn link_block<'a>(ctx: &Ctx, item: &ChecklistItem) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let mut block: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.s)
        .push(field_label(t!(i18n, ids::CHECKLIST_FIELD_LINK), tokens));
    match &item.link {
        Some(link) => block = block.push(card(ctx, item, link)),
        None => block = block.push(note(t!(i18n, ids::CHECKLIST_LINK_NONE), tokens)),
    }
    let toggle = ctx.send(ChecklistMsg::LinkPickerToggled {
        item: item.id.clone(),
    });
    block = block.push(text_button(
        Some(Glyph::Plus),
        t!(i18n, ids::CHECKLIST_LINK_PICK),
        Some(toggle),
        ButtonKind::Ghost,
        tokens,
    ));
    let open = ctx
        .state
        .and_then(|state| state.link_picker.as_deref())
        .is_some_and(|id| id == item.id);
    if open {
        block = block.push(picker(ctx, item));
    }
    block.boxed()
}

/// Is the link picker open in `ctx`'s tab?
pub(super) fn picker_open(state: Option<&super::ChecklistTabState>) -> bool {
    state.is_some_and(|state| state.link_picker.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_text_links_as_itself_or_as_a_design_object() {
        assert_eq!(typed_link("  "), None);
        assert_eq!(typed_link("erc"), Some(ChecklistLink::Erc));
        assert_eq!(
            typed_link("U7 pin 3"),
            Some(ChecklistLink::Object {
                target: "U7 pin 3".into()
            })
        );
    }
}
