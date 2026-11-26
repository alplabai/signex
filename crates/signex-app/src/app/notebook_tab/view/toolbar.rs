//! The toolbar (mockup row 195): Source | Split | Preview, then Insert
//! reference, Calculation, Measurement, Figure and Block (its menu: Note,
//! Issue, Decision), and at the end Export PDF… and Save (Ctrl+S). With no
//! notebook the view switch and Export PDF are greyed.

use iced::widget::{Column, column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_notebook::{AnnotationKind, Insertion};
use signex_widgets::doc_tab_shell::Toolbar;
use signex_widgets::segmented::segmented_disabled;
use signex_widgets::{button, segmented, text};

use super::{Look, SP, TS};
use crate::app::Message;
use crate::app::notebook_tab::{Load, NotebookMsg, NotebookTab, ViewMode};
use crate::panels::graphite::hint_tooltip;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Save's shortcut as the keymap spells it; shown as this OS writes it.
const SAVE_SHORTCUT: &str = "Mod+S";

/// The Block menu's rows, in order.
pub(super) const BLOCK_ROWS: [(AnnotationKind, MessageId); 3] = [
    (AnnotationKind::Note, ids::NOTEBOOK_TAB_BLOCK_NOTE),
    (AnnotationKind::Issue, ids::NOTEBOOK_TAB_BLOCK_ISSUE),
    (AnnotationKind::Decision, ids::NOTEBOOK_TAB_BLOCK_DECISION),
];

pub(super) fn view<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Toolbar<'a, Message> {
    let open = look.doc.load == Load::Open;
    let editable = open && !look.doc.is_read_only();
    let id = look.id;
    let lift = |item: Element<'a, NotebookMsg>| item.map(Message::from).boxed();
    let mut bar = Toolbar::new().push(lift(views(look, tab, open)));
    if editable {
        bar = bar
            .separator()
            .push(lift(tool(
                look,
                ids::NOTEBOOK_TAB_INSERT_REFERENCE,
                Some(NotebookMsg::PickerOpened { project_id: id }),
            )))
            .push(lift(insert(
                look,
                ids::NOTEBOOK_TAB_INSERT_CALCULATION,
                Insertion::Block(AnnotationKind::Calculation),
            )))
            .push(lift(insert(
                look,
                ids::NOTEBOOK_TAB_INSERT_MEASUREMENT,
                Insertion::Block(AnnotationKind::Measurement),
            )))
            .push(lift(insert(
                look,
                ids::NOTEBOOK_TAB_INSERT_FIGURE,
                Insertion::Figure,
            )))
            .push(lift(block_button(look)));
    }
    // The export reads the notebook as saved, so it waits for a file.
    let saved = look
        .doc
        .file
        .is_some()
        .then_some(Message::from(NotebookMsg::ExportPdfRequested {
            project: Some(id),
        }));
    let export = hint_tooltip(
        button(text(look.t(ids::NOTEBOOK_TAB_EXPORT_PDF)).size(TS.dense))
            .padding([SP.xs, SP.s])
            .style(crate::styles::ghost_button(look.tokens))
            .on_press_maybe(saved)
            .boxed(),
        look.t(ids::NOTEBOOK_TAB_EXPORT_PDF_TIP),
        look.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    bar = bar.push_end(export);
    if open {
        bar = bar.push_end(lift(save_button(look, tab)));
    }
    bar
}

/// Source | Split | Preview; greyed with no notebook.
fn views<'a>(look: Look<'a>, tab: &NotebookTab, open: bool) -> Element<'a, NotebookMsg> {
    let id = look.id;
    let cells: Vec<(ViewMode, signex_widgets::Label<'a>)> = ViewMode::ALL
        .into_iter()
        .map(|view| (view, look.t(view_label(view)).into()))
        .collect();
    let style = resolve(look.tokens).segmented(TS.dense);
    if open {
        segmented(cells, tab.view, style, move |view| {
            NotebookMsg::ViewPicked {
                project_id: id,
                view,
            }
        })
    } else {
        segmented_disabled(cells, tab.view, style)
    }
}

pub(super) fn view_label(view: ViewMode) -> MessageId {
    match view {
        ViewMode::Source => ids::NOTEBOOK_TAB_VIEW_SOURCE,
        ViewMode::Split => ids::NOTEBOOK_TAB_VIEW_SPLIT,
        ViewMode::Preview => ids::NOTEBOOK_TAB_VIEW_PREVIEW,
    }
}

/// An insert button.
fn insert<'a>(look: Look<'a>, label: MessageId, insertion: Insertion) -> Element<'a, NotebookMsg> {
    let message = NotebookMsg::InsertPressed {
        project_id: look.id,
        insertion,
    };
    tool(look, label, Some(message))
}

/// A quiet toolbar button.
fn tool<'a>(
    look: Look<'a>,
    label: MessageId,
    on_press: Option<NotebookMsg>,
) -> Element<'a, NotebookMsg> {
    let face = text(look.t(label))
        .size(TS.dense)
        .color(ti(look.tokens.text));
    button(face)
        .padding([SP.xs, SP.s])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press_maybe(on_press)
        .boxed()
}

/// Block: its menu opens over the body (`block_menu`).
fn block_button<'a>(look: Look<'a>) -> Element<'a, NotebookMsg> {
    tool(
        look,
        ids::NOTEBOOK_TAB_INSERT_BLOCK,
        Some(NotebookMsg::BlockMenuToggled {
            project_id: look.id,
        }),
    )
}

/// The Block menu: Note, Issue, Decision.
pub(super) fn block_menu<'a>(look: Look<'a>) -> Element<'a, NotebookMsg> {
    let id = look.id;
    let mut rows: Column<Element<'a, NotebookMsg>> = column![].spacing(SP.xs);
    for (kind, label) in BLOCK_ROWS {
        rows = rows.push(
            button(text(look.t(label)).size(TS.dense))
                .padding([SP.xs, SP.s])
                .width(Length::Fill)
                .style(crate::styles::ghost_button(look.tokens))
                .on_press(NotebookMsg::InsertPressed {
                    project_id: id,
                    insertion: Insertion::Block(kind),
                })
                .boxed(),
        );
    }
    container(rows)
        .padding(SP.xs)
        .style(crate::styles::card(look.tokens))
        .boxed()
}

/// Save, the primary action, with its shortcut; greyed while the text is
/// saved or the file is read-only.
fn save_button<'a>(look: Look<'a>, _tab: &NotebookTab) -> Element<'a, NotebookMsg> {
    let doc = look.doc;
    let can_save = doc.is_unsaved() && !doc.saving && !doc.is_read_only();
    let face = button(text(look.t(ids::NOTEBOOK_TAB_SAVE)).size(TS.dense))
        .padding([SP.xs, SP.m])
        .style(crate::styles::primary_button(look.tokens))
        .on_press_maybe(can_save.then_some(NotebookMsg::SavePressed {
            project_id: look.id,
        }));
    hint_tooltip(
        face.boxed(),
        t!(
            look.i18n,
            ids::NOTEBOOK_TAB_SAVE_TIP,
            "key" => crate::keymap::shown_in(SAVE_SHORTCUT, look.i18n)
        ),
        look.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}
