//! The Health Check tab (#2353): the board's design rule findings, with
//! Run DRC and a link to the results, and the project's electrical rule
//! findings.

use iced::widget::{Column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};

use super::chrome::Form;
use super::{PanelMsg, PcbBoardMsg, SP, info};

pub(super) fn view<'a>(form: &Form<'a>) -> Element<'a, PanelMsg> {
    let findings = |count: usize| t!(form.i18n, ids::PCB_BOARD_HEALTH_FINDINGS, "count" => count);
    let drc_label = form.t(ids::PCB_BOARD_HEALTH_DRC);
    let drc_value = match info::drc_violations(form) {
        Some(n) => form.link(
            findings(n),
            PanelMsg::PcbBoard(PcbBoardMsg::DrcResultsOpened),
            form.t(ids::PCB_BOARD_INFO_DRC_TIP),
        ),
        None => form.value(form.t(ids::PCB_BOARD_HEALTH_NOT_RUN)),
    };
    let erc_label = form.t(ids::PCB_BOARD_HEALTH_ERC);
    let erc_value = form.value(findings(form.ctx.erc_diagnostics.len()));
    let run = form.action(
        form.t(ids::PCB_BOARD_HEALTH_RUN_DRC),
        Some(PanelMsg::RunDrc),
        None,
    );
    Column::new()
        .width(Length::Fill)
        .spacing(SP.xs)
        .push(form.field(&drc_label, drc_value))
        .push(form.field(&erc_label, erc_value))
        .push(
            container(row![run])
                .padding([SP.s, SP.l])
                .width(Length::Fill)
                .boxed(),
        )
        .boxed()
}
