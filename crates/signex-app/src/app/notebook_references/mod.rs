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

/// A reference command's own read of the project it named, and the command
/// that waits for it: it travels with the command, not in the Annotations
/// panel's state, so the panel reading the project in view meanwhile
/// cannot take its place (#3022).
#[derive(Debug, Clone)]
pub struct ReferenceCommandRead {
    pub(crate) rerun: NotebookRerun,
    pub(crate) project: crate::app::ProjectId,
    pub(crate) result: std::sync::Arc<Result<NotebookScan, String>>,
}

impl PartialEq for ReferenceCommandRead {
    fn eq(&self, other: &Self) -> bool {
        self.rerun == other.rerun
            && self.project == other.project
            && std::sync::Arc::ptr_eq(&self.result, &other.result)
    }
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
    /// A reference command's read landed: the command runs again on it.
    CommandRead(Box<ReferenceCommandRead>),
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
            } => match self.fix_reference_in_buffer(&file, &reference, &fix) {
                Ok((_changed, task)) => task,
                Err(why) => {
                    crate::diagnostics::log_info(why);
                    Task::none()
                }
            },
            NotebookReferenceMsg::BlockOpened { file, line } => self.open_notebook_at(&file, line),
            NotebookReferenceMsg::BadgesToggled => {
                let state = &mut self.ui_state.annotations;
                state.badges_hidden = !state.badges_hidden;
                state.hovered_badge = None;
                self.repaint_notebook_badges();
                Task::none()
            }
            NotebookReferenceMsg::CommandRead(read) => self.rerun_on_command_read(*read),
        }
    }

    /// A card action: cross-probe and Open sheet go to the part (a net is
    /// highlighted), Copy reference writes the call to the clipboard,
    /// Timeline opens the notebook tab of the project read with the
    /// reference's timeline in its margin lane (mockup row 199). The source
    /// and Update all belong to the notebook tab.
    fn run_card_action(&mut self, reference: &Reference, action: CardAction) -> Task<Message> {
        if action == CardAction::Timeline
            && let Some((project, _)) = self.ui_state.annotations.target.clone()
        {
            let open = self.open_notebook_tab(project);
            let timeline = crate::app::notebook_tab::NotebookMsg::TimelineOpened {
                project_id: project,
                reference: reference.clone(),
            };
            let shown = self.dispatch_notebook_message(timeline);
            return Task::batch([open, shown]);
        }
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

    /// A reference command's read landed: run the command again, as the
    /// caller that sent it, answering from that read. The read is held
    /// only for the length of the run.
    pub(crate) fn rerun_on_command_read(&mut self, read: ReferenceCommandRead) -> Task<Message> {
        let ReferenceCommandRead {
            rerun,
            project,
            result,
        } = read;
        let scan = match &*result {
            Ok(scan) => Some(std::sync::Arc::new(scan.clone())),
            Err(_) => None,
        };
        self.agent.notebook_read = Some((project, scan));
        let task = self.rerun_after_read(
            rerun.ticket,
            &rerun.command,
            &rerun.args,
            &rerun.project,
            rerun.origin,
            rerun.serving,
        );
        self.agent.notebook_read = None;
        task
    }
}
