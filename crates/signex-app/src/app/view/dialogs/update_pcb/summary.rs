//! The Update PCB summary bar (#2371): the Add, Change, Remove and Problems
//! chips (a click lists only that kind) and the Show problems only toggle.

use iced::widget::{Space, container, row, tooltip as tip};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_engine::pcb::eco::EcoChangeList;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, button, checkbox, text, tooltip};

use super::{SP, TS, up};
use crate::app::update_pcb::table::{Summary, TableState};
use crate::app::update_pcb::{RowAction, RowFilter};
use crate::app::{Message, UpdatePcbMsg};
use crate::styles::{PillTone, Radii, pill_colors};

pub(super) fn bar<'a>(
    list: &EcoChangeList,
    table: &TableState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let summary = Summary::of(list);
    let chips = [
        (
            RowFilter::Action(RowAction::Add),
            ids::UPDATE_PCB_CHIP_ADD,
            summary.add,
            PillTone::Neutral,
        ),
        (
            RowFilter::Action(RowAction::Change),
            ids::UPDATE_PCB_CHIP_CHANGE,
            summary.change,
            PillTone::Neutral,
        ),
        (
            RowFilter::Action(RowAction::Remove),
            ids::UPDATE_PCB_CHIP_REMOVE,
            summary.remove,
            PillTone::Neutral,
        ),
        (
            RowFilter::Problems,
            ids::UPDATE_PCB_CHIP_PROBLEMS,
            summary.problems,
            if summary.problems > 0 {
                PillTone::Warn
            } else {
                PillTone::Neutral
            },
        ),
    ];
    let mut bar = row![].spacing(SP.s).align_y(Alignment::Center);
    for (filter, id, count, tone) in chips {
        let active = table.filter == Some(filter);
        bar = bar.push(chip(i18n, id, count, tone, active, filter, tokens));
    }
    let problems_only = checkbox(table.problems_only)
        .size(TS.body)
        .labelled(t!(i18n, ids::UPDATE_PCB_PROBLEMS_ONLY))
        .text_size(TS.body)
        .style(crate::panels::form_look::checkbox_style)
        .on_toggle(|on| up(UpdatePcbMsg::ProblemsOnlyToggled(on)));
    bar.push(Space::new().width(Length::Fill))
        .push(problems_only)
        .boxed()
}

/// One chip: the pill look of its tone, outlined while it filters.
fn chip<'a>(
    i18n: &I18n,
    id: MessageId,
    count: usize,
    tone: PillTone,
    active: bool,
    filter: RowFilter,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (fill, ink) = pill_colors(tokens, tone);
    let outline = if active { ink } else { fill };
    let pressed = button(text(t!(i18n, id, "count" => count)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(up(UpdatePcbMsg::FilterChipClicked(filter)))
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(Background::Color(fill)),
            text_color: ink,
            border: Border {
                width: 1.0,
                color: outline,
                radius: Radii::GRAPHITE.pill.into(),
            },
            ..iced::widget::button::Style::default()
        });
    let r = crate::styles::tokens::resolve(tokens);
    let hint = container(text(t!(i18n, ids::UPDATE_PCB_CHIP_TOOLTIP)).size(TS.caption))
        .padding([SP.xs, SP.s])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            r.raised,
            r.line_strong,
        ));
    tooltip(pressed, hint, tip::Position::Bottom).into()
}
