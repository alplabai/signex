//! The panel header (#2353): the board's name, the search field, the
//! Selection Filter menu and the General / Parameters / Health Check tabs.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Row, container, row};
use iced::{Alignment, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, UserText, ids, t};
use signex_widgets::{button, text, text_input};

use super::super::form_look;
use super::chrome::Form;
use super::{BoardTab, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, filter, info};
use crate::styles::{PillTone, TypeScale, pill};

const TS: TypeScale = TypeScale::DEFAULT;

fn msg(m: PcbBoardMsg) -> PanelMsg {
    PanelMsg::PcbBoard(m)
}

pub(super) fn view<'a>(form: &Form<'a>, board: &'a PcbBoardSummary) -> Element<'a, PanelMsg> {
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let title = row![
        text(form.t(ids::PCB_PROPERTIES_TITLE))
            .size(TS.body)
            .font(semibold)
            .color(form.ink()),
        text(UserText::new(board.name.clone()))
            .size(TS.dense)
            .color(form.muted())
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);

    let search = text_input(form.t(ids::PCB_BOARD_SEARCH), board.view.search.as_str())
        .on_input(|s| msg(PcbBoardMsg::SearchChanged(s)))
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .style(form_look::text_input_style)
        .width(Length::FillPortion(3));
    let summary = filter::summary(form.i18n, &board.filter);
    let menu = button(
        row![
            text(summary)
                .size(TS.body)
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .width(Length::Fill),
            signex_widgets::chevron::chevron(board.view.filter_menu_open, TS.section, form.muted()),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.s])
    .width(Length::FillPortion(2))
    .on_press(msg(PcbBoardMsg::FilterMenuToggled))
    .style(form_look::value_field_button);
    let tools = row![search, menu].spacing(SP.s).align_y(Alignment::Center);

    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .width(Length::Fill)
        .push(title.boxed())
        .push(tools.boxed());
    if board.view.filter_menu_open {
        col = col.push(filter::chips(form, &board.filter));
    }
    col = col.push(tabs(form, board.view.tab));
    container(col)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

fn tabs<'a>(form: &Form<'a>, active: BoardTab) -> Element<'a, PanelMsg> {
    let findings = info::drc_violations(form).unwrap_or(0) + form.ctx.erc_diagnostics.len();
    let general = tab(
        form,
        form.t(ids::PCB_BOARD_TAB_GENERAL),
        None,
        active == BoardTab::General,
        Some(msg(PcbBoardMsg::TabPicked(BoardTab::General))),
    );
    let parameters = form.tip(
        tab(
            form,
            form.t(ids::PCB_BOARD_TAB_PARAMETERS),
            None,
            false,
            None,
        ),
        form.t(ids::PCB_BOARD_PARAMETERS_UNAVAILABLE),
    );
    let health = tab(
        form,
        form.t(ids::PCB_BOARD_TAB_HEALTH),
        (findings > 0).then(|| t!(form.i18n, ids::PCB_BOARD_HEALTH_BADGE, "count" => findings)),
        active == BoardTab::HealthCheck,
        Some(msg(PcbBoardMsg::TabPicked(BoardTab::HealthCheck))),
    );
    row![general, parameters, health]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
}

/// One tab chip; `msg: None` draws it disabled. `badge` is a warning count.
fn tab<'a>(
    form: &Form<'a>,
    label: Localized,
    badge: Option<Localized>,
    active: bool,
    msg: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let mut content: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(text(label).size(TS.body).boxed());
    if let Some(badge) = badge {
        content = content.push(
            container(text(badge).size(TS.caption))
                .padding([0.0, SP.s])
                .style(pill(form.tokens(), PillTone::Warn))
                .boxed(),
        );
    }
    let enabled = msg.is_some();
    button(content)
        .padding([SP.s, SP.l])
        .on_press_maybe(msg)
        .style(move |theme: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered);
            let mut style = form_look::tab_chip(theme, active, hovered);
            if !enabled {
                style.text_color = form_look::FormLook::of(theme).ink4;
            }
            style
        })
        .boxed()
}
