//! `query_block_diagram document=`: every block, port, connection and group
//! of an open diagram as JSON, with whether it is saved, dirty or
//! read-only. Reads the diagram only; changes nothing.

use serde_json::{Value, json};

use super::refusal::Refusal;
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};

impl Signex {
    /// The answer of `query_block_diagram`.
    pub(crate) fn query_block_diagram(&self, args: &CommandArgs) -> Result<Value, Refusal> {
        let path = self.named_block_diagram(args)?;
        let editor = self
            .document_state
            .block_diagrams
            .editors
            .get(&path)
            .ok_or_else(|| Refusal::NotOpen {
                file: super::file_name(&path),
            })?;
        let diagram = serde_json::to_value(&editor.diagram).unwrap_or(Value::Null);
        Ok(json!({
            "id": editor.diagram.id.to_string(),
            "path": path.display().to_string(),
            "dirty": editor.is_dirty(),
            "saved": !editor.is_new(),
            "read_only": editor.is_read_only(),
            "diagram": diagram,
        }))
    }

    /// `query_block_diagram` as a command outcome.
    pub(crate) fn query_block_diagram_outcome(&self, args: &CommandArgs) -> CommandOutcome {
        match self.query_block_diagram(args) {
            Ok(value) => CommandOutcome::Answered(value),
            Err(refusal) => CommandOutcome::Disabled {
                reason: refusal.translatable(),
            },
        }
    }
}
