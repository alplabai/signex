//! The group bar over the table: one chip per group level, outermost
//! first, joined by `›`. A chip's arrow flips its direction and its ✕
//! removes it; "+ Group" adds an innermost level; Expand all and Collapse
//! all open or close every group.

use iced::widget::{Row, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::super::{HINT_TEXT_SIZE, SP};
use super::columns::caption;
use super::state::ComponentsTable;
use super::view::user_text;
use crate::library::messages::{ComponentsTableMsg, LibraryMessage};
use crate::styles::ti;
use crate::styles::tokens::resolve;

fn msg(m: ComponentsTableMsg) -> LibraryMessage {
    LibraryMessage::ComponentsTable(m)
}

pub(super) fn view<'a>(
    table: &'a ComponentsTable,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (fill, ink, muted) = (r.accent_soft, r.accent_soft_ink, r.ink3);
    let group = &table.cache.setup.group;
    let mut chips: Row<Element<'a, LibraryMessage>> =
        row![].spacing(SP.xs).align_y(Alignment::Center);
    if group.is_empty() {
        chips = chips.push(
            text(t!(i18n, ids::PANELS_COMPONENTS_GROUP_HINT))
                .size(HINT_TEXT_SIZE)
                .color(muted)
                .boxed(),
        );
    }
    for (level, key) in group.iter().enumerate() {
        if level > 0 {
            chips = chips.push(
                text(user_text("›"))
                    .size(HINT_TEXT_SIZE)
                    .color(muted)
                    .boxed(),
            );
        }
        let arrow = if key.descending { "↓" } else { "↑" };
        let flat = move |_: &Theme, _: button::Status| button::Style {
            background: None,
            text_color: ink,
            ..button::Style::default()
        };
        let chip = row![
            text(user_text(caption(&key.column, i18n)))
                .size(HINT_TEXT_SIZE)
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None),
            button(text(user_text(arrow)).size(HINT_TEXT_SIZE).color(ink))
                .padding(0)
                .style(flat)
                .on_press(msg(ComponentsTableMsg::GroupFlipped(level))),
            button(
                text(t!(i18n, ids::LIBRARY_RANGE_FILTER_REMOVE))
                    .size(HINT_TEXT_SIZE)
                    .color(ink)
            )
            .padding(0)
            .style(flat)
            .on_press(msg(ComponentsTableMsg::GroupRemoved(level))),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        chips = chips.push(
            container(chip)
                .padding([1.0, 6.0])
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(fill)),
                    border: Border {
                        radius: 9.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed(),
        );
    }
    chips = chips.push(bar_button(
        t!(i18n, ids::PANELS_COMPONENTS_GROUP_ADD),
        ComponentsTableMsg::GroupMenuToggled,
        tokens,
    ));
    let mut line = row![chips.width(Length::Fill)]
        .spacing(SP.xs)
        .align_y(Alignment::Center);
    if !group.is_empty() {
        line = line
            .push(bar_button(
                t!(i18n, ids::PANELS_COMPONENTS_EXPAND_ALL),
                ComponentsTableMsg::ExpandAllPressed,
                tokens,
            ))
            .push(bar_button(
                t!(i18n, ids::PANELS_COMPONENTS_COLLAPSE_ALL),
                ComponentsTableMsg::CollapseAllPressed,
                tokens,
            ));
    }
    container(line)
        .padding([SP.xs, 10.0])
        .width(Length::Fill)
        .boxed()
}

fn bar_button<'a>(
    label: signex_i18n::Localized,
    pressed: ComponentsTableMsg,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    button(text(label).size(HINT_TEXT_SIZE).color(ink))
        .padding([1.0, 6.0])
        .on_press(msg(pressed))
        .style(crate::panels::form_look::ghost_button)
        .boxed()
}
