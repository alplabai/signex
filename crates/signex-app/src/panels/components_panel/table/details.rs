//! The Details card's lists for the selected part: its parameters (the
//! pinned ones first, the rest under Show all; drawn by the details
//! parameter table with a pin star per row), its references and its part
//! choices. A value that is not set reads "Not set".

use iced::widget::{Column, column, row};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::super::{HINT_TEXT_SIZE, ROW_TEXT_SIZE, SP};
use super::cells;
use super::state::ComponentsTable;
use super::view::user_text;
use crate::library::messages::{ComponentsTableMsg, LibraryMessage};
use crate::styles::ti;
use crate::styles::tokens::resolve;

fn msg(m: ComponentsTableMsg) -> LibraryMessage {
    LibraryMessage::ComponentsTable(m)
}

/// One parameter line as Details shows it: name, value text, whether it
/// is set and whether it is pinned.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DetailLine {
    pub name: String,
    pub value: String,
    pub is_set: bool,
    pub pinned: bool,
}

/// The lines Details lists now: the pinned ones, then the rest when Show
/// all is on. The second value is how many parameters there are in all,
/// for the Show all button (`None` when nothing is left to show).
pub(crate) fn detail_lines(
    table: &ComponentsTable,
    i18n: &I18n,
) -> (Vec<DetailLine>, Option<usize>) {
    let details = &table.cache.details;
    let all = details.parameters.len();
    let line = |e: &signex_library::components_view::DetailEntry| {
        let shown = cells::shown(&e.value, i18n).or_else(|| details.half_set.get(&e.name).cloned());
        DetailLine {
            name: e.name.clone(),
            is_set: shown.is_some(),
            value: shown.unwrap_or_else(|| i18n.message_plain(ids::PANELS_COMPONENTS_NOT_SET, &[])),
            pinned: e.pinned,
        }
    };
    let pinned = details.parameters.iter().filter(|e| e.pinned).count();
    let lines = details
        .parameters
        .iter()
        .filter(|e| e.pinned || table.show_all_parameters)
        .map(line)
        .collect();
    let more = (pinned < all).then_some(all);
    (lines, more)
}

/// What follows the parameter table: Show all / Show less, then the
/// References and Part Choices sections.
pub(crate) fn details_extras<'a>(
    table: &'a ComponentsTable,
    more: Option<usize>,
    title: impl Fn(signex_i18n::MessageId) -> Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let muted = resolve(tokens).ink4;
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.xs).width(Length::Fill);
    if let Some(all) = more {
        let label = if table.show_all_parameters {
            t!(i18n, ids::PANELS_COMPONENTS_SHOW_LESS)
        } else {
            t!(i18n, ids::PANELS_COMPONENTS_SHOW_ALL, "count" => all)
        };
        col = col.push(
            button(text(label).size(HINT_TEXT_SIZE))
                .padding([1.0, 6.0])
                .on_press(msg(ComponentsTableMsg::ShowAllParametersToggled))
                .style(crate::panels::form_look::ghost_button)
                .boxed(),
        );
    }
    let details = &table.cache.details;
    if !details.references.is_empty() {
        col = col.push(title(ids::PANELS_COMPONENTS_DETAILS_REFERENCES));
        for reference in &details.references {
            col = col.push(
                text(user_text(reference.clone()))
                    .size(ROW_TEXT_SIZE)
                    .color(ink)
                    .boxed(),
            );
        }
    }
    if !details.choices.is_empty() {
        col = col.push(title(ids::PANELS_COMPONENTS_DETAILS_CHOICES));
        for choice in &details.choices {
            let stock = match choice.stock {
                Some(0) => i18n.message_plain(ids::PANELS_COMPONENTS_OUT_OF_STOCK, &[]),
                Some(n) => {
                    i18n.message_plain(ids::PANELS_COMPONENTS_IN_STOCK, &[("count", n.into())])
                }
                None => String::new(),
            };
            let price = choice
                .price
                .as_ref()
                .map(|(p, currency)| format!("{p} {currency}"))
                .unwrap_or_default();
            col = col.push(
                row![
                    text(user_text(choice.distributor.clone()))
                        .size(ROW_TEXT_SIZE)
                        .color(ink)
                        .width(Length::FillPortion(2)),
                    text(user_text(stock))
                        .size(HINT_TEXT_SIZE)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    text(user_text(price))
                        .size(HINT_TEXT_SIZE)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    text(user_text(choice.sku.clone()))
                        .size(HINT_TEXT_SIZE)
                        .color(muted)
                        .width(Length::FillPortion(3))
                        .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(SP.s)
                .align_y(Alignment::Center)
                .boxed(),
            );
        }
    }
    col.boxed()
}
