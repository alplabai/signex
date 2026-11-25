//! The empty state (mockup row 195): what a notebook is for, the two
//! templates (Design report, Lab notebook), Create from template, Generate
//! from project… and Import .typ files…, and that nothing is written before
//! Save.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_notebook::Template;
use signex_widgets::{button, text};

use super::{Look, SP, TS};
use crate::app::notebook_tab::{NotebookMsg, NotebookTab};
use crate::panels::graphite::hint_tooltip;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// The width of the empty state's column, in body-text ems.
const COLUMN_EMS: f32 = 36.0;

/// The name and the line under it of each template card.
pub(super) fn template_labels(template: Template) -> (MessageId, MessageId) {
    match template {
        Template::DesignReport => (
            ids::NOTEBOOK_TAB_TEMPLATE_REPORT,
            ids::NOTEBOOK_TAB_TEMPLATE_REPORT_BODY,
        ),
        Template::LabNotebook => (
            ids::NOTEBOOK_TAB_TEMPLATE_LAB,
            ids::NOTEBOOK_TAB_TEMPLATE_LAB_BODY,
        ),
    }
}

pub(super) fn view<'a>(look: Look<'a>, tab: &NotebookTab) -> Element<'a, NotebookMsg> {
    let r = resolve(look.tokens);
    let mut cards = row![].spacing(SP.m);
    for template in Template::ALL {
        cards = cards.push(card(look, template, tab.template == template));
    }
    let lines: Column<'a, NotebookMsg> = column![
        text(look.t(ids::NOTEBOOK_TAB_EMPTY_TITLE))
            .size(TS.title)
            .color(ti(look.tokens.text))
            .boxed(),
        text(look.t(ids::NOTEBOOK_TAB_EMPTY_BODY))
            .size(TS.body)
            .color(ti(look.tokens.text_secondary))
            .boxed(),
        cards.boxed(),
        actions(look, tab.template),
        text(look.t(ids::NOTEBOOK_TAB_EMPTY_NOTE))
            .size(TS.caption)
            .color(r.ink3)
            .boxed(),
    ]
    .spacing(SP.m)
    .align_x(Alignment::Center);
    container(container(lines).width(Length::Fixed(TS.body * COLUMN_EMS)))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

/// A template card; the picked one in the accent.
fn card<'a>(look: Look<'a>, template: Template, picked: bool) -> Element<'a, NotebookMsg> {
    let (name, body) = template_labels(template);
    let name_ink = if picked {
        resolve(look.tokens).accent_text
    } else {
        ti(look.tokens.text)
    };
    let face = column![
        text(look.t(name)).size(TS.label).color(name_ink).boxed(),
        text(look.t(body))
            .size(TS.caption)
            .color(ti(look.tokens.text_secondary))
            .boxed(),
    ]
    .spacing(SP.xs);
    button(
        container(face)
            .padding(SP.s)
            .style(crate::styles::card(look.tokens)),
    )
    .width(Length::FillPortion(1))
    .padding(0)
    .style(crate::styles::ghost_button(look.tokens))
    .on_press(NotebookMsg::TemplateSelected {
        project_id: look.id,
        template,
    })
    .boxed()
}

/// Create from template, Generate from project… (greyed until it ships)
/// and Import .typ files….
fn actions<'a>(look: Look<'a>, template: Template) -> Element<'a, NotebookMsg> {
    let id = look.id;
    let create = button(text(look.t(ids::NOTEBOOK_TAB_CREATE)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::primary_button(look.tokens))
        .on_press(NotebookMsg::TemplatePicked {
            project_id: id,
            template,
        });
    let generate = hint_tooltip(
        button(text(look.t(ids::NOTEBOOK_TAB_GENERATE)).size(TS.label))
            .padding([SP.xs, SP.m])
            .style(crate::styles::outline_button(look.tokens))
            .boxed(),
        look.t(ids::NOTEBOOK_TAB_GENERATE_TIP),
        look.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    let import = button(text(look.t(ids::NOTEBOOK_TAB_IMPORT)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(look.tokens))
        .on_press(NotebookMsg::ImportPressed { project_id: id });
    row![create.boxed(), generate, import.boxed()]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}
