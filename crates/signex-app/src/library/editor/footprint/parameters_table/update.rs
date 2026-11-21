//! The table's own state transitions: opening and closing it, its row
//! menu, the rename draft, the delete prompt and Esc. Nothing here touches
//! the sketch; the edits are `updates::sketch::parameters`.

use super::state::{ParametersTableState, RenameDraft};

/// The table slot on the editor state.
pub type TableSlot = Option<Box<ParametersTableState>>;

/// Parameters was pressed: open the table, or close it when open.
pub fn toggle(slot: &mut TableSlot) {
    *slot = match slot.take() {
        Some(_) => None,
        None => Some(Box::default()),
    };
}

/// Open the table, if closed, with `name` focused and nothing else open.
pub fn open_focused(slot: &mut TableSlot, name: &str) {
    let table = slot.get_or_insert_with(Box::default);
    table.menu = None;
    table.rename = None;
    table.delete_prompt = None;
    table.focus = Some(name.to_string());
}

/// Toggle the ⋯ menu of `name`; any rename or prompt closes.
pub fn toggle_menu(slot: &mut TableSlot, name: String) {
    if let Some(table) = slot.as_deref_mut() {
        table.rename = None;
        table.delete_prompt = None;
        table.menu = if table.menu.as_deref() == Some(name.as_str()) {
            None
        } else {
            Some(name)
        };
    }
}

/// Rename… was picked: the row's name becomes a field holding the name.
pub fn start_rename(slot: &mut TableSlot, name: String) {
    if let Some(table) = slot.as_deref_mut() {
        table.menu = None;
        table.delete_prompt = None;
        table.rename = Some(RenameDraft {
            from: name.clone(),
            draft: name,
            error: None,
        });
    }
}

/// The new name was typed: keep it, forget the last refusal.
pub fn edit_rename(slot: &mut TableSlot, draft: String) {
    if let Some(rename) = slot.as_deref_mut().and_then(|t| t.rename.as_mut()) {
        rename.draft = draft;
        rename.error = None;
    }
}

/// Esc: close the innermost thing in the table, else the table. False when
/// the table was closed already, so Esc goes on down the editor's ladder.
pub fn escape(slot: &mut TableSlot) -> bool {
    match slot.as_deref_mut() {
        None => false,
        Some(table) => {
            if !table.close_innermost() {
                *slot = None;
            }
            true
        }
    }
}
