//! A command that names a diagram not open yet opens it on demand: the
//! file is read off the update thread, the diagram is held (no tab, so the
//! person's view does not move; File ▸ Open shows it with the edits) and the
//! command runs again when the read lands, as the caller that sent it
//! (`Deferred`). A read that fails answers the command with why. A person's
//! own surfaces never come here: they act on a diagram in view.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};

use super::io::{DiagramFile, read_diagram_file};
use super::{BlockDiagramMsg, FileFailure, file_name};
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// The command that waits for the read, and how it was sent.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRerun {
    pub(crate) waiting: WaitingCommand,
    /// Whether it came over the agent socket.
    pub(crate) serving: bool,
}

type Ran = (CommandOutcome, Task<Message>);

impl Signex {
    /// The diagram `args` names when it is in a project's folder but not
    /// open, for a caller other than the person's own surfaces.
    fn diagram_to_open(&self, args: &CommandArgs) -> Option<PathBuf> {
        if self.agent.from_view {
            return None;
        }
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return None;
        };
        let entry = self.resolve_document_target(target).ok()?;
        let path = entry
            .path
            .filter(|_| entry.kind == DocumentKind::BlockDiagram)?;
        let diagrams = &self.document_state.block_diagrams;
        (!diagrams.editors.contains_key(&path)).then_some(path)
    }

    /// Read the diagram `args` names first when it is not open: the
    /// command `id` runs when the read lands. `None` when it is open (or
    /// names none), so the command runs now.
    pub(crate) fn open_diagram_first(&mut self, id: &str, args: &CommandArgs) -> Option<Ran> {
        let path = self.diagram_to_open(args)?;
        let command = AppCommandId::new(id).ok()?;
        let waiting = self.wait_for_command(&command, args);
        let ticket = waiting.ticket;
        let rerun = CommandRerun {
            waiting,
            serving: self.agent.serving,
        };
        let read_path = path.clone();
        let task = Task::perform(
            on_blocking_pool(move || Ok(read_diagram_file(&read_path))),
            move |result| {
                Message::BlockDiagram(BlockDiagramMsg::ReadForCommand {
                    path: path.clone(),
                    result: result.unwrap_or_else(|died| Err(FileFailure::Failed(died))),
                    rerun: Box::new(rerun.clone()),
                })
            },
        );
        Some((CommandOutcome::Deferred { ticket }, task))
    }

    /// The read a command waited for landed: hold the diagram and run the
    /// command, or answer it with why the file did not read. A diagram
    /// opened meanwhile (a tab, an earlier command) is kept as it is.
    pub(super) fn block_diagram_read_for_command(
        &mut self,
        path: PathBuf,
        result: Result<Box<DiagramFile>, FileFailure>,
        rerun: CommandRerun,
    ) -> Task<Message> {
        let CommandRerun { waiting, serving } = rerun;
        let file = match result {
            Ok(file) => file,
            Err(failure) => {
                let reason = unread(&path, &failure);
                return self.finish_waiting_command(waiting, &path, move |_| {
                    CommandOutcome::Disabled { reason }
                });
            }
        };
        self.hold_read_diagram(&path, *file);
        self.refresh_panel_ctx();
        if waiting.command.as_str() == "query_block_diagram" {
            let args = waiting.args.clone();
            return self.finish_waiting_command(waiting, &path, move |app| {
                app.query_block_diagram_outcome(&args)
            });
        }
        let WaitingCommand {
            ticket,
            command,
            args,
            origin,
        } = waiting;
        self.rerun_after_read(ticket, &command, &args, &path, origin, serving)
    }
}

/// Why the file named did not open, for the command's answer.
fn unread(path: &Path, failure: &FileFailure) -> Translatable {
    Translatable::new(ids::BLOCK_DIAGRAM_READ_FOR_COMMAND_FAILED)
        .arg("file", file_name(path))
        .arg("reason", super::io::failure_text(failure))
}
