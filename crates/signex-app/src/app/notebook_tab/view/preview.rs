//! The preview (mockup row 195): the compiled page in the theme's page
//! colours (decided as drawn: under Graphite Dark the page is dark, the PDF
//! prints white), the page navigator, and on an error the last good compile
//! under a red banner with Go to error instead of a blank page.

use iced::widget::{Column, Space, column, container, image, row, scrollable};
use iced::{Alignment, Background, Border, ContentFit, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_notebook::DiagnosticSeverity;
use signex_widgets::{button, text};

use super::editor::main_problems;
use super::{Look, SP, TS};
use crate::app::notebook_tab::{NotebookMsg, NotebookTab};
use crate::styles::tokens::resolve;
use crate::styles::{Radii, ti};

pub(super) fn view<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    let mut pane: Column<'a, NotebookMsg> = column![].spacing(SP.s).padding(SP.s);
    if let Some(banner) = failed_banner(look, tab) {
        pane = pane.push(banner);
    }
    pane = pane.push(navigator(look, tab));
    let page: Element<'a, NotebookMsg> = match &tab.compile.shown {
        Some(shown) => scrollable(
            container(
                image(shown.handle.clone())
                    .content_fit(ContentFit::Contain)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding(SP.s),
        )
        .direction(crate::styles::thin_vertical())
        .style(crate::styles::scrollbar_style(look.tokens))
        .height(Length::Fill)
        .boxed(),
        None => container(
            text(look.t(waiting_label(tab)))
                .size(TS.body)
                .color(ti(look.tokens.text_secondary)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed(),
    };
    pane.push(page).height(Length::Fill).boxed()
}

/// What the preview says before it has a page.
fn waiting_label(tab: &NotebookTab) -> signex_i18n::MessageId {
    if tab.compile.running.is_some() && tab.compile.last.is_none() {
        ids::NOTEBOOK_TAB_COMPILING
    } else {
        ids::NOTEBOOK_TAB_PREVIEW_NONE
    }
}

/// "page 3 of 12" with the previous and next page.
fn navigator<'a>(look: Look<'a>, tab: &NotebookTab) -> Element<'a, NotebookMsg> {
    let r = resolve(look.tokens);
    let count = tab
        .compile
        .last_good
        .as_ref()
        .map_or(0, |good| good.status.pages.len());
    let id = look.id;
    let step = |label, forward: bool, enabled: bool| {
        button(text(look.t(label)).size(TS.caption))
            .padding([0.0, SP.s])
            .style(crate::styles::ghost_button(look.tokens))
            .on_press_maybe(enabled.then_some(NotebookMsg::PageStepped {
                project_id: id,
                forward,
            }))
            .boxed()
    };
    let shown = if count == 0 { 0 } else { tab.page + 1 };
    let at = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_PREVIEW_PAGE,
        "page" => shown,
        "count" => count
    );
    row![
        text(look.t(ids::NOTEBOOK_TAB_VIEW_PREVIEW))
            .size(TS.section)
            .color(r.ink3)
            .boxed(),
        Space::new().width(Length::Fill).boxed(),
        step(ids::NOTEBOOK_TAB_PREVIOUS_PAGE, false, tab.page > 0),
        text(at).size(TS.caption).color(r.ink3).boxed(),
        step(ids::NOTEBOOK_TAB_NEXT_PAGE, true, tab.page + 1 < count),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// On a failed compile: where it failed, from when the preview is, and Go
/// to error.
fn failed_banner<'a>(look: Look<'a>, tab: &NotebookTab) -> Option<Element<'a, NotebookMsg>> {
    if !tab.compile.failed() {
        return None;
    }
    let line = main_problems(tab)
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| d.line);
    let time = tab
        .compile
        .last_good
        .as_ref()
        .map(|good| good.finished_at.clone())
        .unwrap_or_default();
    let message = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_FAILED_BANNER,
        "line" => line.unwrap_or(0),
        "time" => time.as_str()
    );
    let mut content = row![text(message).size(TS.caption).width(Length::Fill).boxed()]
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if let Some(line) = line {
        content = content.push(
            button(text(look.t(ids::NOTEBOOK_TAB_GO_TO_ERROR)).size(TS.caption))
                .padding([SP.xs, SP.s])
                .style(crate::styles::outline_button(look.tokens))
                .on_press(NotebookMsg::LinePicked {
                    project_id: look.id,
                    line,
                })
                .boxed(),
        );
    }
    let (fill, ink) = crate::styles::pill_colors(look.tokens, crate::styles::PillTone::Err);
    Some(
        container(content)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                text_color: Some(ink),
                border: Border {
                    radius: Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}
