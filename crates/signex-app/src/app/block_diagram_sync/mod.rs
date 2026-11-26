//! Generate a block diagram from the sheet hierarchy and regenerate it
//! (#2968, mockup row 171): `generate_block_diagram` and
//! `regenerate_block_diagram`. Both read the project's hierarchy off the UI
//! thread (`job`) and write a `.snxblk`, never an open document. Run with
//! operands (an agent, the CLI) the command is answered with its JSON result
//! once the run lands; run by a person (the palette) it goes through the
//! same handler (`command::translator`) and Messages says what it did. A
//! regenerate a person runs lists its changes and applies nothing; the
//! block diagram tab's Re-sync applies them (`Requested { apply: true }`).
//! Rules: `docs/architecture/snxblk-format.md`, "Generate and regenerate".

mod job;
mod operands;
#[cfg(test)]
mod tests;

pub(crate) use operands::{generate_block_diagram, regenerate_block_diagram};

use std::path::PathBuf;

use iced::Task;
use serde_json::Value;
use signex_i18n::{ids, t};

use crate::app::agent::WaitingCommand;
use crate::app::command::translator;
use crate::app::command::{CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

/// Messages of the generate and regenerate commands.
#[derive(Debug, Clone)]
pub enum BlockDiagramSyncMsg {
    /// A person asked for a generate or a regenerate of the project in view.
    Requested { regenerate: bool, apply: bool },
    /// A run landed.
    Synced(Box<SyncLanded>),
}

/// A finished run and whom to answer.
#[derive(Debug, Clone)]
pub struct SyncLanded {
    /// The agent or headless caller waiting for the answer; `None` for a
    /// person's run, which Messages answers.
    pub(crate) waiting: Option<WaitingCommand>,
    /// The project file the run read.
    pub(crate) project: PathBuf,
    pub(crate) result: Result<Value, String>,
}

impl Signex {
    pub(crate) fn dispatch_block_diagram_sync_message(
        &mut self,
        msg: BlockDiagramSyncMsg,
    ) -> Task<Message> {
        match msg {
            BlockDiagramSyncMsg::Requested { regenerate, apply } => {
                self.request_block_diagram_sync(regenerate, apply)
            }
            BlockDiagramSyncMsg::Synced(landed) => self.land_block_diagram_sync(*landed),
        }
    }

    /// A person's run on the project in view, through the same handler an
    /// agent runs.
    fn request_block_diagram_sync(&mut self, regenerate: bool, apply: bool) -> Task<Message> {
        let project = self
            .compile_target()
            .and_then(|id| self.document_state.project_by_id(id))
            .map(|project| project.path.clone());
        let Some(project) = project else {
            self.say_block_diagram_refused("open a sheet of a project first");
            return Task::none();
        };
        let (id, operands) = if regenerate {
            let apply = vec![("apply", OperandValue::Bool(apply))];
            ("regenerate_block_diagram", apply)
        } else {
            ("generate_block_diagram", Vec::new())
        };
        let (outcome, task) = translator::run(self, id, &project, operands);
        if let CommandOutcome::Disabled { reason } = outcome {
            let why = reason.render(&self.ui_state.i18n);
            self.say_block_diagram_refused(&why);
        }
        task
    }

    fn land_block_diagram_sync(&mut self, landed: SyncLanded) -> Task<Message> {
        let SyncLanded {
            waiting,
            project,
            result,
        } = landed;
        match waiting {
            Some(waiting) => {
                self.finish_waiting_command(waiting, &project, move |_| match result {
                    Ok(answer) => CommandOutcome::Answered(answer),
                    Err(reason) => CommandOutcome::disabled(reason),
                })
            }
            None => {
                self.say_block_diagram_result(&result);
                Task::none()
            }
        }
    }

    /// What a person's run did, in Messages.
    fn say_block_diagram_result(&self, result: &Result<Value, String>) {
        let answer = match result {
            Ok(answer) => answer,
            Err(reason) => return self.say_block_diagram_refused(reason),
        };
        let i18n = &self.ui_state.i18n;
        let path = answer["path"].as_str().unwrap_or_default();
        let count = |v: &Value| v.as_u64().unwrap_or(0) as i64;
        let line = match answer["changes"].as_array() {
            None => t!(
                i18n,
                ids::PROJECT_BLOCK_DIAGRAM_GENERATED,
                "path" => path,
                "blocks" => count(&answer["summary"]["blocks"]),
                "connections" => count(&answer["summary"]["connections"])
            ),
            Some(changes) if changes.is_empty() => {
                t!(i18n, ids::PROJECT_BLOCK_DIAGRAM_IN_SYNC, "path" => path)
            }
            Some(changes) => {
                let id = if answer["written"].as_bool() == Some(true) {
                    ids::PROJECT_BLOCK_DIAGRAM_APPLIED
                } else {
                    ids::PROJECT_BLOCK_DIAGRAM_CHANGES
                };
                t!(i18n, id, "path" => path, "changes" => changes.len() as i64)
            }
        };
        crate::diagnostics::log_info(line.into_string());
    }

    fn say_block_diagram_refused(&self, reason: &str) {
        let line = t!(
            self.ui_state.i18n,
            ids::PROJECT_BLOCK_DIAGRAM_REFUSED,
            "reason" => reason
        );
        crate::diagnostics::log_warning(line.into_string());
    }
}
