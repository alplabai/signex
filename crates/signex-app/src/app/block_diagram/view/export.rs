//! The export popover (mockup row 171, "Export PDF…"): format, colours,
//! and for a PDF its pages, paper and title block, then the file, with
//! Cancel and Export. It
//! exports the saved file, so it says so while the diagram holds unsaved
//! edits. A pure view; its events are the tab's (`TabEvent::Export`).

use iced::widget::{Column, Space, column, container, pick_list, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, checkbox, segmented, text, text_input};

use super::{Look, SP, TS};
use crate::app::block_diagram::export::KINDS;
use crate::app::block_diagram::state::{ExportDraft, ExportKind, ExportPage, ExportPaper};
use crate::app::block_diagram::{ExportEvent, OpenDiagram, TabEvent};
use crate::choice::{Choice, chosen};
use crate::styles::tokens::resolve;
use crate::styles::{DialogButtonKind, dialog_button, ti};

/// The card's width, in body-text ems (mockup: 300 px).
const CARD_EMS: f32 = 24.0;
/// The label column's width, in body-text ems (mockup: 76 px).
const LABEL_EMS: f32 = 6.0;

fn kind_label(look: Look<'_>, kind: ExportKind) -> signex_widgets::Label<'static> {
    match kind {
        ExportKind::Svg => look.t(ids::BLOCK_DIAGRAM_EXPORT_SVG).into(),
        ExportKind::Pdf => look.t(ids::BLOCK_DIAGRAM_EXPORT_PDF_FORMAT).into(),
    }
}

/// A name on the left, its control on the right.
fn line<'a>(
    look: Look<'a>,
    name: MessageId,
    value: Element<'a, TabEvent>,
) -> Element<'a, TabEvent> {
    row![
        text(look.t(name))
            .size(TS.dense)
            .color(resolve(look.tokens).ink3)
            .width(Length::Fixed(TS.body * LABEL_EMS))
            .boxed(),
        container(value).width(Length::Fill).boxed(),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

fn sent(event: ExportEvent) -> TabEvent {
    TabEvent::Export(event)
}

fn page_label(page: ExportPage) -> MessageId {
    match page {
        ExportPage::Blocks => ids::BLOCK_DIAGRAM_EXPORT_PAGE_BLOCKS,
        ExportPage::PowerTree => ids::BLOCK_DIAGRAM_EXPORT_PAGE_POWER,
        ExportPage::Budget => ids::BLOCK_DIAGRAM_EXPORT_PAGE_BUDGET,
    }
}

fn paper_label(paper: ExportPaper) -> MessageId {
    match paper {
        ExportPaper::A4 => ids::BLOCK_DIAGRAM_EXPORT_PAPER_A4,
        ExportPaper::A3 => ids::BLOCK_DIAGRAM_EXPORT_PAPER_A3,
        ExportPaper::A2 => ids::BLOCK_DIAGRAM_EXPORT_PAPER_A2,
        ExportPaper::Letter => ids::BLOCK_DIAGRAM_EXPORT_PAPER_LETTER,
        ExportPaper::Tabloid => ids::BLOCK_DIAGRAM_EXPORT_PAPER_TABLOID,
    }
}

/// Pages: a box per page, in print order (mockup row 171).
fn pages<'a>(look: Look<'a>, draft: &ExportDraft) -> Element<'a, TabEvent> {
    let mut list: Column<Element<'a, TabEvent>> = Column::new().spacing(SP.xs);
    for page in ExportPage::ALL {
        list = list.push(
            checkbox(draft.pages.contains(&page))
                .style(crate::panels::form_look::checkbox_style)
                .size(TS.body)
                .labelled(look.t(page_label(page)))
                .text_size(TS.dense)
                .on_toggle(move |on| sent(ExportEvent::PageToggled(page, on)))
                .boxed(),
        );
    }
    list.boxed()
}

/// Paper: the size, then landscape or portrait.
fn paper_field<'a>(look: Look<'a>, draft: &ExportDraft) -> Element<'a, TabEvent> {
    let choices = Choice::named(&ExportPaper::ALL, paper_label, look.i18n);
    let selected = chosen(&choices, &draft.paper);
    let size = pick_list(selected, choices, |c: &Choice<ExportPaper>| c.label.clone())
        .graphite()
        .on_select(|c: Choice<ExportPaper>| sent(ExportEvent::PaperPicked(c.value)))
        .text_size(TS.dense)
        .boxed();
    let turn = segmented(
        vec![
            (false, look.t(ids::BLOCK_DIAGRAM_EXPORT_LANDSCAPE).into()),
            (true, look.t(ids::BLOCK_DIAGRAM_EXPORT_PORTRAIT).into()),
        ],
        draft.portrait,
        resolve(look.tokens).segmented(TS.dense),
        |on| sent(ExportEvent::PortraitPicked(on)),
    );
    row![size, turn]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

/// The popover card for `draft`.
pub(super) fn view<'a>(
    look: Look<'a>,
    editor: OpenDiagram<'a>,
    draft: &'a ExportDraft,
) -> Element<'a, TabEvent> {
    let r = resolve(look.tokens);
    let title = text(look.t(ids::BLOCK_DIAGRAM_EXPORT_HEADING))
        .size(TS.label)
        .color(ti(look.tokens.text))
        .boxed();
    let kinds: Vec<(ExportKind, signex_widgets::Label<'a>)> =
        KINDS.iter().map(|k| (*k, kind_label(look, *k))).collect();
    let format = segmented(kinds, draft.kind, r.segmented(TS.dense), |kind| {
        sent(ExportEvent::KindPicked(kind))
    });
    let colours = segmented(
        vec![
            (true, look.t(ids::BLOCK_DIAGRAM_EXPORT_COLOURS_PRINT).into()),
            (
                false,
                look.t(ids::BLOCK_DIAGRAM_EXPORT_COLOURS_SHOWN).into(),
            ),
        ],
        draft.monochrome,
        r.segmented(TS.dense),
        |on| sent(ExportEvent::MonochromePicked(on)),
    );
    let file = text_input(UserText::new(String::new()), &draft.file)
        .style(crate::panels::form_look::text_input_style)
        .on_input(|typed| sent(ExportEvent::FileEdited(typed)))
        .on_submit(sent(ExportEvent::ExportPressed))
        .size(TS.dense)
        .padding(SP.xs)
        .boxed();
    let mut body: Column<Element<'a, TabEvent>> = column![
        title,
        line(look, ids::BLOCK_DIAGRAM_EXPORT_FORMAT, format),
        line(look, ids::BLOCK_DIAGRAM_EXPORT_COLOURS, colours),
    ]
    .spacing(SP.s)
    .width(Length::Fixed(TS.body * CARD_EMS));
    if draft.kind == ExportKind::Pdf {
        body = body.push(line(
            look,
            ids::BLOCK_DIAGRAM_EXPORT_PAGES,
            pages(look, draft),
        ));
        let paper = paper_field(look, draft);
        let title_block = checkbox(draft.title_block)
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.body)
            .labelled(look.t(ids::BLOCK_DIAGRAM_EXPORT_TITLE_BLOCK))
            .text_size(TS.dense)
            .on_toggle(|on| sent(ExportEvent::TitleBlockToggled(on)))
            .boxed();
        body = body
            .push(line(look, ids::BLOCK_DIAGRAM_EXPORT_PAPER, paper))
            .push(line(look, ids::BLOCK_DIAGRAM_EXPORT_PAGE, title_block));
    }
    body = body.push(line(look, ids::BLOCK_DIAGRAM_EXPORT_FILE, file));
    if editor.doc.is_dirty() {
        body = body.push(
            text(look.t(ids::BLOCK_DIAGRAM_EXPORT_SAVED_ONLY))
                .size(TS.dense)
                .color(ti(look.tokens.warning))
                .boxed(),
        );
    }
    let ready =
        !draft.file.trim().is_empty() && (draft.kind == ExportKind::Svg || !draft.pages.is_empty());
    let buttons = row![
        Space::new().width(Length::Fill),
        dialog_button(
            look.t(ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(sent(ExportEvent::Cancelled)),
            look.tokens,
        ),
        dialog_button(
            look.t(ids::BLOCK_DIAGRAM_EXPORT_RUN),
            DialogButtonKind::Primary,
            ready.then(|| sent(ExportEvent::ExportPressed)),
            look.tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed();
    container(body.push(buttons))
        .padding(SP.l)
        .style(crate::styles::context_menu(look.tokens))
        .boxed()
}
