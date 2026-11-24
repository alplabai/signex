//! The panel header (#2353, #2523): 'Board' and the file name with the
//! Selection Filter button on one row, the General / Parameters / Health
//! Check tabs under it, and the search field the General tab shows next.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Row, container, row, svg};
use iced::{Alignment, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, UserText, ids, t};
use signex_widgets::{button, text, text_input};

use super::super::form_look;
use super::chrome::{Form, mono, segment_cell};
use super::{BoardTab, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, filter, info};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;

fn msg(m: PcbBoardMsg) -> PanelMsg {
    PanelMsg::PcbBoard(m)
}

/// The header row and the tabs.
pub(super) fn view<'a>(form: &Form<'a>, board: &'a PcbBoardSummary) -> Element<'a, PanelMsg> {
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let title = row![
        text(form.t(ids::PCB_PROPERTIES_TITLE))
            .size(TS.label)
            .font(semibold)
            .color(form.ink())
            .wrapping(Wrapping::None),
        text(UserText::new(board.name.clone()))
            .size(TS.caption)
            .font(mono())
            .color(form.faint())
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill),
        filter_button(form, board),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    iced::widget::column![title, tabs(form, board.view.tab)]
        .spacing(SP.s)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// The search field that filters the General tab's rows.
pub(super) fn search<'a>(form: &Form<'a>, board: &'a PcbBoardSummary) -> Element<'a, PanelMsg> {
    let field = text_input(form.t(ids::PCB_BOARD_SEARCH), board.view.search.as_str())
        .on_input(|s| msg(PcbBoardMsg::SearchChanged(s)))
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .style(form_look::text_input_style)
        .width(Length::Fill);
    container(field)
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// The three tabs as one segmented strip. The Health Check tab carries the
/// count of design and electrical rule findings as an error badge.
fn tabs<'a>(form: &Form<'a>, active: BoardTab) -> Element<'a, PanelMsg> {
    let findings = info::drc_violations(form).unwrap_or(0) + form.ctx.erc_diagnostics.len();
    let label = |id| text(form.t(id)).size(TS.body).wrapping(Wrapping::None);
    let general = segment_cell(
        label(ids::PCB_BOARD_TAB_GENERAL).boxed(),
        active == BoardTab::General,
        Some(msg(PcbBoardMsg::TabPicked(BoardTab::General))),
    );
    let parameters = form.tip(
        segment_cell(label(ids::PCB_BOARD_TAB_PARAMETERS).boxed(), false, None),
        form.t(ids::PCB_BOARD_PARAMETERS_UNAVAILABLE),
    );
    let mut health: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(label(ids::PCB_BOARD_TAB_HEALTH).boxed());
    if findings > 0 {
        let badge = t!(form.i18n, ids::PCB_BOARD_HEALTH_BADGE, "count" => findings);
        health = health.push(form.pill(badge, PillTone::Err));
    }
    let health = segment_cell(
        health.boxed(),
        active == BoardTab::HealthCheck,
        Some(msg(PcbBoardMsg::TabPicked(BoardTab::HealthCheck))),
    );
    container(signex_widgets::FitRow::new(vec![general, parameters, health]).fill(true))
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed()
}

/// The Selection Filter button (#2383): the Active Bar's filter icon and how
/// many kinds are on, accent when any is off, the off kinds on hover. A click
/// opens the Active Bar's Selection Filter dropdown; the chips themselves
/// live in the Selection Filter section only.
fn filter_button<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let filter = &board.filter;
    let total = pcb_geom::PickKind::ALL.len();
    let on = filter.on_kinds().count();
    let accent = resolve(form.tokens()).accent_text;
    let ink = if on < total { accent } else { form.muted() };
    let glyph = svg(crate::icons::icon_filter(form.ctx.theme_id))
        .width(TS.body)
        .height(TS.body);
    let glyph = if on < total {
        glyph.style(move |_: &Theme, _| svg::Style {
            color: Some(accent),
            ..Default::default()
        })
    } else {
        glyph
    };
    let content = row![
        glyph,
        text(filter_count(form, board))
            .size(TS.body)
            .color(ink)
            .wrapping(Wrapping::None)
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let shown = button(content)
        .padding([SP.xs, SP.s])
        .on_press(msg(PcbBoardMsg::FilterMenuToggled))
        .style(form_look::value_field_button);
    let off: Vec<String> = pcb_geom::PickKind::ALL
        .into_iter()
        .filter(|kind| !filter.is_on(*kind))
        .map(|kind| t!(form.i18n, filter::kind_label(kind)).into_string())
        .collect();
    let tip = if off.is_empty() {
        form.t(ids::PCB_BOARD_FILTER_TIP_ALL)
    } else {
        t!(form.i18n, ids::PCB_BOARD_FILTER_TIP_OFF, "off" => off.join(", "))
    };
    form.tip(shown.boxed(), tip)
}

/// How many kinds the filter has on, of all: `11/13`.
pub(super) fn filter_count(form: &Form<'_>, board: &PcbBoardSummary) -> Localized {
    let total = pcb_geom::PickKind::ALL.len();
    let on = board.filter.on_kinds().count();
    t!(form.i18n, ids::PCB_BOARD_FILTER_COUNT, "on" => on, "total" => total)
}
