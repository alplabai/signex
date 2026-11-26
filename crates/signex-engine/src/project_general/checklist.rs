//! The Design Checklist tab's edits (#1291, mockup row 206): start from a
//! template, add an item, set a field, set the state, remove. Each is one
//! step on the project's General history; the checks that refuse one live
//! in `signex-types`.

use signex_types::project::{
    ChecklistField, ChecklistProblem, ItemState, NewChecklistItem, ProjectData,
};

/// One edit of the design checklist. An item is named by its id. `today` is
/// the day the edit is stamped with (`YYYY-MM-DD`), given by the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectChecklistCommand {
    /// Start the checklist from `template` with `items`, worded already.
    Start {
        template: String,
        items: Vec<NewChecklistItem>,
    },
    /// Add an open item after the others.
    AddItem {
        stage: String,
        title: String,
        today: Option<String>,
    },
    /// One field as typed; empty clears the owner, the link and the notes.
    SetField {
        item: String,
        field: ChecklistField,
        text: String,
        today: Option<String>,
    },
    /// Open, Done, or Waived with a reason.
    SetState {
        item: String,
        state: ItemState,
        reason: Option<String>,
        today: Option<String>,
    },
    RemoveItem {
        item: String,
    },
}

impl ProjectChecklistCommand {
    /// Apply to `data`. A refused edit changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), ChecklistProblem> {
        match self {
            Self::Start { template, items } => data.start_design_checklist(template, items.clone()),
            Self::AddItem {
                stage,
                title,
                today,
            } => data
                .add_checklist_item(stage, title, today.as_deref())
                .map(|_| ()),
            Self::SetField {
                item,
                field,
                text,
                today,
            } => data.set_checklist_field(item, *field, text, today.as_deref()),
            Self::SetState {
                item,
                state,
                reason,
                today,
            } => data.set_checklist_state(item, *state, reason.as_deref(), today.as_deref()),
            Self::RemoveItem { item } => data.remove_checklist_item(item),
        }
    }
}
