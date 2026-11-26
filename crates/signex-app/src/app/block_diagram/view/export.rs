//! The export popover (mockup row 171, "Export PDF…"): format, paper,
//! colours, the PDF title block and the file, with Cancel and Export. It
//! exports the saved file, so it says so while the diagram holds unsaved
//! edits. A pure view; its events are the tab's (`TabEvent::Export`).

use iced::widget::{Column, Space, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids};
use signex_widgets::{CheckboxLabel as _, checkbox, segmented, text, text_input};

use super::{Look, SP, TS};
use crate::app::block_diagram::export::KINDS;
use crate::app::block_diagram::state::{BlockDiagramEditor, ExportDraft, ExportKind};
use crate::app::block_diagram::{ExportEvent, TabEvent};
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

/// The popover card for `draft`.
pub(super) fn view<'a>(
    look: Look<'a>,
    editor: &'a BlockDiagramEditor,
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
        let paper = text(look.t(ids::BLOCK_DIAGRAM_EXPORT_PAPER_A4))
            .size(TS.dense)
            .color(ti(look.tokens.text))
            .boxed();
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
    if editor.is_dirty() {
        body = body.push(
            text(look.t(ids::BLOCK_DIAGRAM_EXPORT_SAVED_ONLY))
                .size(TS.dense)
                .color(ti(look.tokens.warning))
                .boxed(),
        );
    }
    let ready = !draft.file.trim().is_empty();
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
