//! The Health Check tab (#2353, #2523, mockup row 93): one card per check, each with a
//! status icon. Design rules: the board's finding count as a link to the
//! results, Run DRC (the panel's only Run DRC) and Show results. Electrical
//! rules: the project's schematic check. The app does not record when a
//! check last ran, so the cards say nothing about it.

use iced::widget::text::Wrapping;
use iced::widget::{Column, Space, row, svg};
use iced::{Alignment, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_widgets::text;

use super::chrome::Form;
use super::{PanelMsg, PcbBoardMsg, SP, info};
use crate::styles::{TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

const GLYPH_OK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M3.5 8.4l3 3L12.5 5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const GLYPH_WARN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M8 2.2L14.6 13.6H1.4Z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M8 6.4v3.4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="11.7" r="0.9" fill="currentColor"/></svg>"#;
const GLYPH_IDLE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="5" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>"#;

/// How a check stands, for its status icon.
#[derive(Clone, Copy)]
enum Status {
    Clean,
    Findings,
    NotRun,
}

pub(super) fn view<'a>(form: &Form<'a>) -> Element<'a, PanelMsg> {
    Column::new()
        .width(Length::Fill)
        .spacing(SP.xs)
        .push(design_rules(form))
        .push(electrical_rules(form))
        .boxed()
}

fn findings(form: &Form<'_>, count: usize) -> Localized {
    t!(form.i18n, ids::PCB_BOARD_HEALTH_FINDINGS, "count" => count)
}

/// Design rules: status, the count (a link to the results above zero) or
/// Not run, then Run DRC and Show results.
fn design_rules<'a>(form: &Form<'a>) -> Element<'a, PanelMsg> {
    let open = PanelMsg::PcbBoard(PcbBoardMsg::DrcResultsOpened);
    let (status, value) = match info::drc_violations(form) {
        Some(0) => (Status::Clean, caption(form, findings(form, 0))),
        Some(n) => (
            Status::Findings,
            form.text_link(
                findings(form, n),
                open.clone(),
                form.t(ids::PCB_BOARD_INFO_DRC_TIP),
            ),
        ),
        None => (
            Status::NotRun,
            caption(form, form.t(ids::PCB_BOARD_HEALTH_NOT_RUN)),
        ),
    };
    let actions = row![
        form.primary(form.t(ids::PCB_BOARD_HEALTH_RUN_DRC), PanelMsg::RunDrc),
        form.action(form.t(ids::PCB_BOARD_HEALTH_SHOW_RESULTS), Some(open), None),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let body = Column::new()
        .spacing(SP.s)
        .width(Length::Fill)
        .push(title_line(
            form,
            status,
            form.t(ids::PCB_BOARD_HEALTH_DRC),
            value,
        ))
        .push(actions.boxed());
    form.card(body.boxed())
}

/// Electrical rules: the project's ERC findings, or Not run before one.
fn electrical_rules<'a>(form: &Form<'a>) -> Element<'a, PanelMsg> {
    let ctx = form.ctx;
    let count = ctx.erc_diagnostics.len();
    let (status, value) = if count > 0 {
        (Status::Findings, caption(form, findings(form, count)))
    } else if ctx.erc_ran {
        (Status::Clean, caption(form, findings(form, 0)))
    } else {
        (
            Status::NotRun,
            caption(form, form.t(ids::PCB_BOARD_HEALTH_NOT_RUN)),
        )
    };
    let source = text(form.t(ids::PCB_BOARD_HEALTH_ERC_SOURCE))
        .size(TS.caption)
        .color(form.faint())
        .wrapping(Wrapping::None);
    let body = Column::new()
        .spacing(SP.s)
        .width(Length::Fill)
        .push(title_line(
            form,
            status,
            form.t(ids::PCB_BOARD_HEALTH_ERC),
            value,
        ))
        .push(source.boxed());
    form.card(body.boxed())
}

/// Status icon, the check's name in bold, and its value at the right.
fn title_line<'a>(
    form: &Form<'a>,
    status: Status,
    name: Localized,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let tokens = form.tokens();
    let (glyph, ink): (&'static [u8], Color) = match status {
        Status::Clean => (GLYPH_OK, ti(tokens.success)),
        Status::Findings => (GLYPH_WARN, ti(tokens.error)),
        Status::NotRun => (GLYPH_IDLE, form.faint()),
    };
    let icon = svg(svg::Handle::from_memory(glyph))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    row![
        icon,
        text(name)
            .size(TS.body)
            .font(semibold)
            .color(form.ink())
            .wrapping(Wrapping::None),
        Space::new().width(Length::Fill),
        value,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .boxed()
}

fn caption<'a>(form: &Form<'a>, line: Localized) -> Element<'a, PanelMsg> {
    text(line)
        .size(TS.caption)
        .font(super::chrome::mono())
        .color(form.muted())
        .wrapping(Wrapping::None)
        .boxed()
}
