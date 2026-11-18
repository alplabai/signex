//! The Project tab's Requirements edits (#2339): add, set a field, turn on
//! or off, remove. Each is one step on the project's General history; the
//! checks that refuse one live in `signex-types`.

use signex_types::project::{ProjectData, RequirementField, RequirementProblem};

/// One edit of the requirements list. An entry is named by its place in the
/// list, which no other writer changes between two steps.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectRequirementCommand {
    /// Add a requirement of `kind` after the others; `id` `None` takes the
    /// kind's name, made unique.
    Add {
        kind: String,
        id: Option<String>,
    },
    /// One field as typed; empty clears an optional field.
    SetField {
        index: usize,
        field: RequirementField,
        text: String,
    },
    SetEnabled {
        index: usize,
        on: bool,
    },
    Remove {
        index: usize,
    },
}

impl ProjectRequirementCommand {
    /// Apply to `data`. A refused edit changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), RequirementProblem> {
        match self {
            Self::Add { kind, id } => data.add_requirement(kind, id.as_deref()).map(|_| ()),
            Self::SetField { index, field, text } => {
                data.set_requirement_field(*index, *field, text)
            }
            Self::SetEnabled { index, on } => data.set_requirement_enabled(*index, *on),
            Self::Remove { index } => data.remove_requirement(*index),
        }
    }
}
