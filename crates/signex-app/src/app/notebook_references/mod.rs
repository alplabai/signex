//! References from the Design Notebook to the design (mockup rows 196 and
//! 197): each notebook file's references resolved against the project on
//! the notebook read's worker, the stale ones and their fixes, the hover
//! cards, the `@` picker, cross-probing both ways, the schematic's
//! Notebook badges and Properties ▸ Design notes. The rules live in
//! `signex-notebook`; the catalog commands are in [`commands`] and the
//! views in [`views`]. See `docs/NOTEBOOK_ANNOTATIONS.md`, "References".

mod badge_hover;
mod buffer;
pub(crate) mod commands;
mod read;
pub mod views;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use iced::Task;
use signex_i18n::{ids, t};
use signex_notebook::{Badge, CardAction, Fix, Reference};

pub use read::FileReferences;
pub(crate) use read::{file_references, of_text, read_design};

use crate::app::agent::Origin;
use crate::app::command::CommandArgs;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::annotations::NotebookScan;

/// The Notebook badges of a read: the blocks per part and net.
pub(crate) fn notebook_badges_of(scan: &NotebookScan) -> Vec<Badge> {
    crate::panels::annotations::notebook_badges(scan)
}

/// A notebook command that waits for the notebook read it started: run
/// again, as the caller that sent it, when the read lands.
#[derive(Debug, Clone, PartialEq)]
pub struct NotebookRerun {
    pub(crate) ticket: u64,
    pub(crate) command: AppCommandId,
    pub(crate) args: CommandArgs,
    /// The project file the command named.
    pub(crate) project: PathBuf,
    pub(crate) origin: Origin,
    pub(crate) serving: bool,
}

/// What happened on a reference card, a badge card or Properties ▸ Design
/// notes.
#[derive(Debug, Clone, PartialEq)]
pub enum NotebookReferenceMsg {
    /// A card's button, or the reference's right-click row, was pressed.
    CardActionPicked {
        reference: Reference,
        action: CardAction,
    },
    /// A fix of a stale reference was picked in `file` (inside `notebook/`).
    FixPicked {
        file: String,
        reference: Reference,
        fix: Fix,
    },
    /// A block row, or Open in notebook, was pressed: the notebook opens at
    /// the block in `file` (inside `notebook/`) starting on `line`.
    BlockOpened { file: String, line: u32 },
    /// View ▸ Notebook Badges (B) was pressed.
    BadgesToggled,
}

impl Signex {
    /// A reference card, badge card or Design notes event.
    pub(crate) fn handle_notebook_reference_message(
        &mut self,
        msg: NotebookReferenceMsg,
    ) -> Task<Message> {
        match msg {
            NotebookReferenceMsg::CardActionPicked { reference, action } => {
                self.run_card_action(&reference, action)
            }
            NotebookReferenceMsg::FixPicked {
                file,
                reference,
                fix,
            } => {
                if let Err(why) = self.fix_reference_in_buffer(&file, &reference, &fix) {
                    crate::diagnostics::log_info(why);
                }
                Task::none()
            }
            NotebookReferenceMsg::BlockOpened { file, line } => {
                self.open_notebook_at(&file, line);
                Task::none()
            }
            NotebookReferenceMsg::BadgesToggled => {
                let state = &mut self.ui_state.annotations;
                state.badges_hidden = !state.badges_hidden;
                state.hovered_badge = None;
                self.repaint_notebook_badges();
                Task::none()
            }
        }
    }

    /// A card action: cross-probe and Open sheet go to the part (a net is
    /// highlighted), Copy reference writes the call to the clipboard. The
    /// timeline (mockup row 199) and the source belong to the notebook tab.
    fn run_card_action(&mut self, reference: &Reference, action: CardAction) -> Task<Message> {
        match action {
            CardAction::CrossProbe | CardAction::OpenSheet | CardAction::HighlightNet => {
                self.jump_to_reference(reference)
            }
            CardAction::CopyReference => {
                let line = t!(
                    self.ui_state.i18n, ids::NOTEBOOK_REF_COPIED,
                    "reference" => reference.name()
                );
                crate::diagnostics::log_info(line.into_string());
                iced::clipboard::write(reference.to_markup()).discard()
            }
            CardAction::Timeline | CardAction::ShowInSource | CardAction::UpdateAllStale => {
                let line = t!(self.ui_state.i18n, ids::NOTEBOOK_REF_NEEDS_TAB);
                crate::diagnostics::log_info(line.into_string());
                Task::none()
            }
        }
    }

    /// Run every notebook command that waited for the read that just
    /// landed, each answering from it.
    pub(crate) fn rerun_waiting_notebook_commands(&mut self) -> Task<Message> {
        let waiting = std::mem::take(&mut self.ui_state.annotations.reruns);
        let mut tasks = Vec::with_capacity(waiting.len());
        for rerun in waiting {
            self.ui_state.annotations.answering = true;
            tasks.push(self.rerun_after_read(
                rerun.ticket,
                &rerun.command,
                &rerun.args,
                &rerun.project,
                rerun.origin,
                rerun.serving,
            ));
            self.ui_state.annotations.answering = false;
        }
        Task::batch(tasks)
    }
}
