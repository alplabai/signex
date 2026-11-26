//! Rich text notes on the schematic (#1114, mockup row 203): their Typst
//! compiles in tasks with a hash-keyed cache, the drawings the canvas shows,
//! and the in-place editor over the sheet (source with line numbers on the
//! left, the note as it will draw on the right, recompiled as you type; a
//! toolbar that wraps the selection; Ctrl+Enter applies as one undo step,
//! Esc drops the edit; links open on Ctrl+click). The compile and the vector
//! lowering are `signex_notebook::note`'s. Behaviour:
//! `docs/architecture/snxsch-note-format.md`.

mod commands;
mod compile;
mod editor;
mod preview;
mod state;
mod view;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_commands;
#[cfg(test)]
mod tests_view;

use std::sync::Arc;

use iced::Task;
use iced::widget::text_editor;
use signex_notebook::note::NoteKey;

pub(crate) use commands::{
    cancel_rich_note_edit, compile_rich_note, edit_rich_note, open_rich_note_link,
    wrap_rich_note_selection,
};
pub use compile::{NoteJob, run_jobs};
pub use editor::{LinkTarget, resolve_link};
pub use state::{NoteOutcome, RichNoteEditor, RichNotes, Wrap, line_and_index};

use crate::app::{Message, Signex, TextEditMsg};

/// What happened to rich notes and their editor. Namespaced under
/// `Message::TextEdit`: the editor is the in-place editor of a rich note.
#[derive(Debug, Clone)]
pub enum RichNoteMsg {
    /// Compiles of the open sheets' notes landed, by key.
    Compiled {
        outcomes: Arc<Vec<(NoteKey, Arc<NoteOutcome>)>>,
    },
    /// `compile_rich_note`'s compile landed: answer its caller.
    CommandCompiled {
        waiting: Box<crate::app::agent::WaitingCommand>,
        sheet: std::path::PathBuf,
        note: uuid::Uuid,
        key: NoteKey,
        outcome: Arc<NoteOutcome>,
    },
    /// A double-click, Edit… in Properties or `edit_rich_note` asked for the
    /// editor on a note of the active sheet, the cursor at byte `at`.
    EditorRequested { note: uuid::Uuid, at: usize },
    /// A keystroke, a cursor move or a selection in the source.
    EditorActed(text_editor::Action),
    /// Typing paused: compile the text of `generation` if it is still the
    /// newest.
    EditorPaused { generation: u64 },
    /// The compile of the typed text of `generation` landed.
    EditorCompiled {
        generation: u64,
        key: NoteKey,
        outcome: Arc<NoteOutcome>,
    },
    /// A toolbar button was pressed.
    WrapPressed(Wrap),
    /// Apply, or Ctrl+Enter.
    ApplyPressed,
    /// Cancel, or Esc.
    CancelPressed,
    /// Ctrl+click on a link of the preview.
    LinkClicked { target: String },
    /// A link's launch finished; `failure` says why it did not open.
    LinkOpened {
        target: String,
        failure: Option<String>,
    },
}

impl From<RichNoteMsg> for Message {
    fn from(msg: RichNoteMsg) -> Self {
        Message::TextEdit(TextEditMsg::RichNote(msg))
    }
}

impl Signex {
    /// The router: every message to its handler.
    pub(crate) fn dispatch_rich_note_message(&mut self, msg: RichNoteMsg) -> Task<Message> {
        match msg {
            RichNoteMsg::Compiled { outcomes } => self.rich_notes_compiled(&outcomes),
            RichNoteMsg::CommandCompiled {
                waiting,
                sheet,
                note,
                key,
                outcome,
            } => self.rich_note_command_compiled(*waiting, &sheet, note, key, outcome),
            RichNoteMsg::EditorRequested { note, at } => self
                .request_rich_note_editor(note, at)
                .unwrap_or_else(|why| {
                    crate::diagnostics::log_warning(why);
                    Task::none()
                }),
            RichNoteMsg::EditorActed(action) => self.rich_note_editor_acted(action),
            RichNoteMsg::EditorPaused { generation } => self.rich_note_editor_paused(generation),
            RichNoteMsg::EditorCompiled {
                generation,
                key,
                outcome,
            } => {
                self.rich_note_editor_compiled(generation, key, outcome);
                Task::none()
            }
            RichNoteMsg::WrapPressed(wrap) => {
                self.wrap_rich_note_editor(wrap).unwrap_or_else(|why| {
                    crate::diagnostics::log_warning(why);
                    Task::none()
                })
            }
            RichNoteMsg::ApplyPressed => {
                if let Err(refusal) = self.apply_rich_note_editor() {
                    crate::diagnostics::log_warning(refusal.reason);
                }
                self.finish_update()
            }
            RichNoteMsg::CancelPressed => {
                self.ui_state.rich_notes.editor = None;
                Task::none()
            }
            RichNoteMsg::LinkClicked { target } => self.open_note_link(&target),
            RichNoteMsg::LinkOpened { target, failure } => {
                if let Some(why) = failure {
                    let line = self.ui_state.i18n.message_plain(
                        signex_i18n::ids::OVERLAY_RICH_NOTE_LINK_FAILED,
                        &[
                            ("target", signex_i18n::Arg::from(target)),
                            ("why", signex_i18n::Arg::from(why)),
                        ],
                    );
                    crate::diagnostics::log_warning(line);
                }
                Task::none()
            }
        }
    }
}
