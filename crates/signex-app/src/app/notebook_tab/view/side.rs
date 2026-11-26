//! The side panel (mockup row 195): Outline (numbered sections, figures and
//! tables, annotation blocks), Pages (a thumbnail per page of the last good
//! compile) or References (each part, pin and net the notebook names). A
//! row of the notebook file moves the cursor to it; a page shows it in the
//! preview. Also the reference picker's card, drawn over the editor.

use iced::widget::{Column, column, container, image, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_notebook::{FigureKind, Reference, ReferenceReport, SourceAt};
use signex_widgets::{button, segmented, text, text_input};

use super::editor::mono;
use super::{Look, SP, TS, user};
use crate::app::notebook_references::views::{picker_list, summary_line};
use crate::app::notebook_tab::open::MAIN_FILE;
use crate::app::notebook_tab::picker::{PICKER_INPUT_ID, Picker};
use crate::app::notebook_tab::{NotebookMsg, NotebookTab, SidePanel};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// A thumbnail's width, in body-text ems.
const THUMB_EMS: f32 = 7.0;

pub(super) fn view<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    let id = look.id;
    let cells: Vec<(SidePanel, signex_widgets::Label<'a>)> = SidePanel::ALL
        .into_iter()
        .map(|panel| (panel, look.t(panel_label(panel)).into()))
        .collect();
    let switch = segmented(
        cells,
        tab.panel,
        resolve(look.tokens).segmented(TS.dense),
        move |panel| NotebookMsg::PanelPicked {
            project_id: id,
            panel,
        },
    );
    let list = match tab.panel {
        SidePanel::Outline => outline(look, tab),
        SidePanel::Pages => pages(look, tab),
        SidePanel::References => references(look, tab),
    };
    column![
        switch,
        scrollable(list)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(look.tokens))
            .height(Length::Fill)
            .boxed(),
    ]
    .spacing(SP.s)
    .padding(SP.s)
    .height(Length::Fill)
    .boxed()
}

pub(super) fn panel_label(panel: SidePanel) -> MessageId {
    match panel {
        SidePanel::Outline => ids::NOTEBOOK_TAB_OUTLINE,
        SidePanel::Pages => ids::NOTEBOOK_TAB_PAGES,
        SidePanel::References => ids::NOTEBOOK_TAB_REFERENCES,
    }
}

fn outline<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    let Some(typed) = &tab.text else {
        return Column::<Element<'a, NotebookMsg>>::new().boxed();
    };
    let outline = &typed.outline;
    if outline.sections.is_empty() && outline.figures.is_empty() && outline.blocks.is_empty() {
        return quiet(look, ids::NOTEBOOK_TAB_OUTLINE_EMPTY);
    }
    let mut list: Column<Element<'a, NotebookMsg>> = column![].spacing(SP.xs);
    if !outline.sections.is_empty() {
        list = list.push(group(look, ids::NOTEBOOK_TAB_OUTLINE_SECTIONS));
        for section in &outline.sections {
            let indent = SP.m * f32::from(section.depth.saturating_sub(1));
            let face = row![
                text(user(section.number.clone()))
                    .size(TS.dense)
                    .font(mono())
                    .color(resolve(look.tokens).ink3)
                    .boxed(),
                text(user(section.title.clone())).size(TS.dense).boxed(),
            ]
            .spacing(SP.s)
            .padding(iced::Padding::ZERO.left(indent));
            list = list.push(at_row(look, face.boxed(), &section.at));
        }
    }
    if !outline.figures.is_empty() {
        list = list.push(group(look, ids::NOTEBOOK_TAB_OUTLINE_FIGURES));
        for figure in &outline.figures {
            let kind = match figure.kind {
                FigureKind::Figure => "figure",
                FigureKind::Table => "table",
            };
            let label = t!(
                look.i18n,
                ids::NOTEBOOK_TAB_FIGURE_ROW,
                "kind" => kind,
                "number" => figure.number,
                "caption" => figure.caption.as_str()
            );
            list = list.push(at_row(look, text(label).size(TS.dense).boxed(), &figure.at));
        }
    }
    if !outline.blocks.is_empty() {
        list = list.push(group(look, ids::NOTEBOOK_TAB_OUTLINE_BLOCKS));
        for block in &outline.blocks {
            let id = block
                .id
                .as_ref()
                .map_or_else(|| block.kind.to_string(), ToString::to_string);
            let face = row![
                text(user(id))
                    .size(TS.dense)
                    .font(mono())
                    .color(resolve(look.tokens).accent_text)
                    .boxed(),
                text(user(block.title.clone())).size(TS.dense).boxed(),
            ]
            .spacing(SP.s);
            list = list.push(at_row(look, face.boxed(), &block.at));
        }
    }
    list.boxed()
}

/// An outline row: pressable when it sits in the notebook file.
fn at_row<'a>(
    look: Look<'a>,
    face: Element<'a, NotebookMsg>,
    at: &SourceAt,
) -> Element<'a, NotebookMsg> {
    let press = (at.file == MAIN_FILE).then_some(NotebookMsg::LinePicked {
        project_id: look.id,
        line: at.line,
    });
    button(face)
        .padding([0.0, SP.xs])
        .width(Length::Fill)
        .style(crate::styles::ghost_button(look.tokens))
        .on_press_maybe(press)
        .boxed()
}

fn pages<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    if tab.compile.thumbnails.is_empty() {
        return quiet(look, ids::NOTEBOOK_TAB_PAGES_EMPTY);
    }
    let accent = ti(look.tokens.accent);
    let mut list: Column<Element<'a, NotebookMsg>> =
        column![].spacing(SP.s).align_x(Alignment::Center);
    for (index, thumb) in tab.compile.thumbnails.iter().enumerate() {
        let number = t!(look.i18n, ids::NOTEBOOK_TAB_PAGE_ROW, "number" => index + 1);
        let ink = if index == tab.page {
            accent
        } else {
            ti(look.tokens.text_secondary)
        };
        let face = column![
            image(thumb.clone())
                .width(Length::Fixed(TS.body * THUMB_EMS))
                .boxed(),
            text(number).size(TS.caption).color(ink).boxed(),
        ]
        .spacing(SP.xs)
        .align_x(Alignment::Center);
        list = list.push(
            button(face)
                .padding(SP.xs)
                .style(crate::styles::ghost_button(look.tokens))
                .on_press(NotebookMsg::PagePicked {
                    project_id: look.id,
                    page: index,
                })
                .boxed(),
        );
    }
    list.boxed()
}

/// Each reference the notebook file makes. Once the project's notebook
/// read has resolved them against the design (mockup row 196): the summary
/// line, then every reference with its uses, a stale one in the warning
/// colour; a row shows its card in the margin lane.
fn references<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    if let Some(report) = look.references {
        return resolved_references(look, report);
    }
    let Some(outcome) = &tab.compile.last else {
        return quiet(look, ids::NOTEBOOK_TAB_REFERENCES_EMPTY);
    };
    if outcome.status.references.is_empty() {
        return quiet(look, ids::NOTEBOOK_TAB_REFERENCES_EMPTY);
    }
    let mut list: Column<Element<'a, NotebookMsg>> = column![].spacing(SP.xs);
    for reference in &outcome.status.references {
        list = list.push(
            row![
                text(user(reference.name()))
                    .size(TS.dense)
                    .font(mono())
                    .boxed(),
                text(look.t(kind_label(reference)))
                    .size(TS.caption)
                    .color(resolve(look.tokens).ink3)
                    .boxed(),
            ]
            .spacing(SP.s)
            .padding([0.0, SP.xs])
            .boxed(),
        );
    }
    list.boxed()
}

fn resolved_references<'a>(
    look: Look<'a>,
    report: &'a ReferenceReport,
) -> Element<'a, NotebookMsg> {
    if report.references.is_empty() {
        return quiet(look, ids::NOTEBOOK_TAB_REFERENCES_EMPTY);
    }
    let r = resolve(look.tokens);
    let summary = summary_line(look.i18n, report);
    let mut list: Column<Element<'a, NotebookMsg>> =
        column![text(summary).size(TS.caption).color(r.ink3).boxed()].spacing(SP.xs);
    for entry in &report.references {
        let ink = if entry.is_stale() {
            ti(look.tokens.warning)
        } else {
            ti(look.tokens.text)
        };
        let face = row![
            text(user(entry.reference.name()))
                .size(TS.dense)
                .font(mono())
                .color(ink)
                .boxed(),
            text(look.t(kind_label(&entry.reference)))
                .size(TS.caption)
                .color(r.ink3)
                .width(Length::Fill)
                .boxed(),
            text(user(entry.uses().to_string()))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        ]
        .spacing(SP.s);
        list = list.push(
            button(face)
                .padding([0.0, SP.xs])
                .width(Length::Fill)
                .style(crate::styles::ghost_button(look.tokens))
                .on_press(NotebookMsg::ReferenceShown {
                    project_id: look.id,
                    reference: Some(entry.reference.clone()),
                })
                .boxed(),
        );
    }
    list.boxed()
}

pub(super) fn kind_label(reference: &Reference) -> MessageId {
    match reference {
        Reference::Component(_) => ids::NOTEBOOK_TAB_KIND_COMPONENT,
        Reference::Pin { .. } => ids::NOTEBOOK_TAB_KIND_PIN,
        Reference::Net(_) => ids::NOTEBOOK_TAB_KIND_NET,
    }
}

fn group<'a>(look: Look<'a>, label: MessageId) -> Element<'a, NotebookMsg> {
    text(look.t(label))
        .size(TS.section)
        .color(resolve(look.tokens).ink3)
        .boxed()
}

fn quiet<'a>(look: Look<'a>, label: MessageId) -> Element<'a, NotebookMsg> {
    text(look.t(label))
        .size(TS.dense)
        .color(resolve(look.tokens).ink3)
        .boxed()
}

/// The reference picker: the query, the count, the matching parts, pins and
/// nets of the design as the `@` picker lists them (mockup row 196), and
/// the keys.
pub(super) fn picker_card<'a>(look: Look<'a>, picker: &'a Picker) -> Element<'a, NotebookMsg> {
    let id = look.id;
    let r = resolve(look.tokens);
    let placeholder = look.t(ids::NOTEBOOK_TAB_PICKER_PLACEHOLDER);
    let input = text_input(placeholder, picker.query.as_str())
        .id(iced::widget::Id::new(PICKER_INPUT_ID))
        .on_input(move |query| NotebookMsg::PickerQueryChanged {
            project_id: id,
            query,
        })
        .on_submit(NotebookMsg::PickerChosen {
            project_id: id,
            row: None,
        })
        .size(TS.body)
        .padding([SP.xs, SP.s]);
    let count = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_PICKER_COUNT,
        "shown" => picker.entries.len(),
        "count" => picker.matches
    );
    let entries =
        picker_list(&picker.entries, picker.row, look.i18n, look.tokens).map(move |entry| {
            NotebookMsg::PickerEntryPicked {
                project_id: id,
                reference: entry.reference,
            }
        });
    let mut list: Column<Element<'a, NotebookMsg>> = column![
        input.boxed(),
        text(count).size(TS.caption).color(r.ink3).boxed(),
        entries.boxed(),
    ]
    .spacing(SP.xs);
    let call = picker
        .chosen(None)
        .map(|reference| reference.to_markup())
        .unwrap_or_default();
    list = list.push(
        text(t!(look.i18n, ids::NOTEBOOK_TAB_PICKER_HINT, "call" => call.as_str()))
            .size(TS.caption)
            .color(r.ink3)
            .boxed(),
    );
    let card = container(list)
        .padding(SP.s)
        .style(crate::styles::card(look.tokens));
    container(card).padding(SP.m).width(Length::Fill).boxed()
}
