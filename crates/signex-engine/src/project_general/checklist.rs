//! The Design Checklist tab's edits (#1291, mockup row 206): start from a
//! template or from another project's items, reset from the template, add
//! an item, set a field, set the state, remove. Each is one step on the
//! project's General history; the checks that refuse one live in
//! `signex-types`.

use signex_types::project::{
    ChecklistField, ChecklistProblem, EditStamp, ItemState, NewChecklistItem, ProjectData,
};

/// One edit of the design checklist. An item is named by its id. `stamp` is
/// the day and the editor an edit is stamped with, given by the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectChecklistCommand {
    /// Start the checklist from `template` (its key, `None` for a list of
    /// one's own) with `items`, worded already.
    Start {
        template: Option<String>,
        items: Vec<NewChecklistItem>,
        stamp: EditStamp,
    },
    /// Start the checklist from another project's items; `from` names it.
    Import {
        from: String,
        template: Option<String>,
        items: Vec<NewChecklistItem>,
        stamp: EditStamp,
    },
    /// Put the template's items back in place of every item.
    Reset {
        template: String,
        items: Vec<NewChecklistItem>,
        stamp: EditStamp,
    },
    /// Add an open item after the others.
    AddItem {
        stage: String,
        title: String,
        stamp: EditStamp,
    },
    /// One field as typed; empty clears the owner, the link and the notes.
    SetField {
        item: String,
        field: ChecklistField,
        text: String,
        stamp: EditStamp,
    },
    /// Open, Done, or Waived with a reason.
    SetState {
        item: String,
        state: ItemState,
        reason: Option<String>,
        stamp: EditStamp,
    },
    RemoveItem {
        item: String,
    },
}

impl ProjectChecklistCommand {
    /// Apply to `data`. A refused edit changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), ChecklistProblem> {
        match self {
            Self::Start {
                template,
                items,
                stamp,
            } => data.start_design_checklist(template.as_deref(), items.clone(), stamp),
            Self::Import {
                from,
                template,
                items,
                stamp,
            } => data.import_design_checklist(from, template.as_deref(), items.clone(), stamp),
            Self::Reset {
                template,
                items,
                stamp,
            } => data.reset_design_checklist(template, items.clone(), stamp),
            Self::AddItem {
                stage,
                title,
                stamp,
            } => data.add_checklist_item(stage, title, stamp).map(|_| ()),
            Self::SetField {
                item,
                field,
                text,
                stamp,
            } => data.set_checklist_field(item, *field, text, stamp),
            Self::SetState {
                item,
                state,
                reason,
                stamp,
            } => data.set_checklist_state(item, *state, reason.as_deref(), stamp),
            Self::RemoveItem { item } => data.remove_checklist_item(item),
        }
    }
}
