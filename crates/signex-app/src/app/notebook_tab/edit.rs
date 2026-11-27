//! Edits in the notebook editor: typing (with `@` opening the reference
//! picker in place of typing an `@`), the insert toolbar, the picker's
//! choice, and moving the cursor to a line or the next problem. Every edit
//! copies the body into the notebook document, reads the outline and
//! colours again, marks the file unsaved while the body differs from it,
//! and asks for a compile.

use std::sync::Arc;

use iced::Task;
use iced::advanced::text::Position;
use iced::widget::text_editor::{Action, Cursor, Edit};
use signex_i18n::{Translatable, ids};
use signex_notebook::{Inserted, Insertion, NotebookDocument, Reference, insertion_text};

use super::open::MAIN_FILE;
use super::picker::{PICKER_INPUT_ID, Picker};
use super::{Load, NotebookTab, NotebookText};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// What an editor action does on the tab.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Acted {
    /// Nothing changed in the text.
    Moved,
    /// The text changed.
    Edited,
    /// `@` was typed: the picker opens, nothing is typed.
    OpensPicker,
}

/// Perform `action` on `tab`'s text. A read-only file takes no edit.
pub(super) fn act(doc: &NotebookDocument, tab: &mut NotebookTab, action: Action) -> Acted {
    let read_only = doc.is_read_only();
    let Some(text) = &mut tab.text else {
        return Acted::Moved;
    };
    if matches!(action, Action::Edit(Edit::Insert('@'))) && !read_only {
        return Acted::OpensPicker;
    }
    if action.is_edit() && read_only {
        return Acted::Moved;
    }
    let edits = action.is_edit();
    text.content.perform(action);
    if edits { Acted::Edited } else { Acted::Moved }
}

/// Copy the editor's body into `doc` and read the outline and colours
/// again: the one step after every edit.
pub(super) fn sync_body(doc: &mut NotebookDocument, tab: &mut NotebookTab) {
    if let Some(text) = &mut tab.text {
        doc.body = Some(text.body());
        text.refresh(doc);
    }
}

/// Insert `inserted` at the cursor of `text`, on a line of its own for a
/// block or a figure.
pub(super) fn paste(text: &mut NotebookText, inserted: &Inserted, own_line: bool) {
    let (_, column) = text.cursor();
    let lead = if own_line && column > 0 { "\n" } else { "" };
    let pasted = format!("{lead}{}", inserted.text);
    text.content
        .perform(Action::Edit(Edit::Paste(Arc::new(pasted))));
}

/// Move the cursor of `text` to the start of 0-based body line `line`.
pub(super) fn move_to_line(text: &mut NotebookText, line: usize) {
    let last = text.content.line_count().saturating_sub(1);
    text.content.move_to(Cursor {
        position: Position {
            line: line.min(last),
            index: 0,
        },
        selection: None,
    });
}

/// Make the editor's text `body` as one edit of the editor (the changed
/// span selected and pasted over), so Undo takes it back and what was typed
/// before stays in the history (#3019). Falls back to setting the text when
/// the editor does not end up with `body`.
pub(super) fn replace_in_editor(text: &mut NotebookText, body: &str) {
    let old = text.body();
    if old == body {
        return;
    }
    let mut prefix = old
        .bytes()
        .zip(body.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(prefix) || !body.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = old[prefix..]
        .bytes()
        .rev()
        .zip(body[prefix..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - suffix) || !body.is_char_boundary(body.len() - suffix) {
        suffix -= 1;
    }
    let (start, old_end) = (prefix, old.len() - suffix);
    let new_mid = &body[prefix..body.len() - suffix];
    let at = |offset: usize| {
        let before = &old[..offset];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        Position {
            line: before.matches('\n').count(),
            index: offset - line_start,
        }
    };
    text.content.move_to(Cursor {
        position: at(old_end),
        selection: (start != old_end).then(|| at(start)),
    });
    if new_mid.is_empty() {
        text.content.perform(Action::Edit(Edit::Delete));
    } else {
        text.content
            .perform(Action::Edit(Edit::Paste(Arc::new(new_mid.to_string()))));
    }
    if text.body() != body {
        text.content = iced::widget::text_editor::Content::with_text(body);
    }
}

impl Signex {
    /// The notebook file text of `id` as typed, header and body; `None`
    /// while it has no body.
    pub(crate) fn notebook_tab_file_text(&self, id: ProjectId) -> Option<String> {
        let doc = self.document_state.notebooks.get(&id)?;
        super::save::written_text(doc)?.ok()
    }

    /// Put the file text `text` in `id`'s notebook, as a reference fix does
    /// (mockup row 196): its header becomes the document's, its body goes
    /// in the editor, and the tab compiles. Written on Save, like any edit.
    pub(crate) fn replace_notebook_tab_file_text(
        &mut self,
        id: ProjectId,
        text: &str,
    ) -> Result<Task<Message>, Translatable> {
        let Some((doc, tab)) = self.notebook_parts_mut(id) else {
            return Err(Translatable::new(ids::NOTEBOOK_TAB_CMD_NOT_OPEN));
        };
        if doc.is_read_only() {
            return Err(Translatable::new(ids::NOTEBOOK_TAB_CMD_READ_ONLY));
        }
        if doc.body.is_none() {
            return Err(Translatable::new(ids::NOTEBOOK_TAB_CMD_NOT_OPEN));
        }
        doc.replace_file_text(text)
            .map_err(|why| crate::app::notebook_project::because(why.to_string()))?;
        if let (Some(editor), Some(body)) = (&mut tab.text, &doc.body) {
            replace_in_editor(editor, body);
        }
        Ok(self.notebook_edited(id))
    }

    /// An editor action landed.
    pub(super) fn notebook_text_acted(&mut self, id: ProjectId, action: Action) -> Task<Message> {
        let Some((doc, tab)) = self.notebook_parts_mut(id) else {
            return Task::none();
        };
        match act(doc, tab, action) {
            Acted::Moved => Task::none(),
            Acted::Edited => self.notebook_edited(id),
            Acted::OpensPicker => self.open_notebook_picker(id),
        }
    }

    /// The text changed: copy it into the document, read it again, mark it
    /// unsaved (or saved again when it is back to the file), and compile.
    pub(super) fn notebook_edited(&mut self, id: ProjectId) -> Task<Message> {
        let Some((doc, tab)) = self.notebook_parts_mut(id) else {
            return Task::none();
        };
        sync_body(doc, tab);
        let path = doc.path.clone();
        if doc.is_unsaved() {
            self.document_state.mark_document_dirty(&path);
        } else {
            self.document_state.clear_document_dirty(&path);
        }
        self.request_notebook_compile(id)
    }

    /// A toolbar button: insert at the cursor.
    pub(super) fn notebook_insert(
        &mut self,
        id: ProjectId,
        insertion: &Insertion,
    ) -> Task<Message> {
        self.notebook_insert_at_cursor(id, insertion).1
    }

    /// Insert at the cursor and say what was written; `None` when the tab
    /// holds no text or its file is read-only.
    pub(super) fn notebook_insert_at_cursor(
        &mut self,
        id: ProjectId,
        insertion: &Insertion,
    ) -> (Option<Inserted>, Task<Message>) {
        let Some((doc, tab)) = self.notebook_parts_mut(id) else {
            return (None, Task::none());
        };
        tab.block_menu = false;
        if doc.is_read_only() {
            return (None, Task::none());
        }
        let Some(inserted) = insert_at_cursor(doc, tab, insertion) else {
            return (None, Task::none());
        };
        (Some(inserted), self.notebook_edited(id))
    }

    /// Open the picker over the editor and focus its query. It lists the
    /// parts, pins and nets of the design the project's last notebook read
    /// built (`signex_notebook::picker_entries`, mockup row 196).
    pub(super) fn open_notebook_picker(&mut self, id: ProjectId) -> Task<Message> {
        let design = self.notebook_design(id);
        let Some((doc, tab)) = self.notebook_parts_mut(id) else {
            return Task::none();
        };
        if tab.text.is_none() || doc.is_read_only() {
            return Task::none();
        }
        tab.block_menu = false;
        tab.picker = Some(Picker::new(design));
        iced::widget::operation::focus(iced::widget::Id::new(PICKER_INPUT_ID))
    }

    /// Enter in the picker, or a row pressed: insert the call, close it.
    pub(super) fn notebook_picker_chosen(
        &mut self,
        id: ProjectId,
        row: Option<usize>,
    ) -> Task<Message> {
        let chosen = self
            .ui_state
            .notebook_tabs
            .get(&id)
            .and_then(|tab| tab.picker.as_ref())
            .and_then(|picker| picker.chosen(row));
        self.notebook_reference_picked(id, chosen)
    }

    /// A picker entry was picked: insert its call, close the picker.
    pub(super) fn notebook_reference_picked(
        &mut self,
        id: ProjectId,
        reference: Option<Reference>,
    ) -> Task<Message> {
        if let Some(tab) = self.ui_state.notebook_tabs.get_mut(&id) {
            tab.picker = None;
        }
        let Some(reference) = reference else {
            return Task::none();
        };
        self.notebook_insert(id, &Insertion::Reference(reference))
    }

    /// Move the cursor to 1-based line `line` of the notebook file.
    pub(super) fn notebook_line_picked(&mut self, id: ProjectId, line: u32) {
        let Some((doc, tab)) = self.notebook_parts_mut(id) else {
            return;
        };
        let Some(body_line) = doc.body_line(line) else {
            return;
        };
        if tab.view == super::ViewMode::Preview {
            tab.view = super::ViewMode::Split;
        }
        if let Some(text) = &mut tab.text {
            move_to_line(text, body_line);
        }
    }

    /// F8 or Go to error: the cursor moves to the problem after it in the
    /// notebook file, wrapping to the first.
    pub(super) fn notebook_next_problem(&mut self, id: ProjectId) -> Option<(u32, u32)> {
        let (doc, tab) = self.notebook_parts(id)?;
        let text = tab.text.as_ref()?;
        let (line, _) = text.cursor();
        let here = doc.file_line(line);
        let outcome = tab.compile.last.clone()?;
        let problem = outcome.status.next_problem(MAIN_FILE, here)?;
        let at = (problem.line, problem.column);
        self.notebook_line_picked(id, at.0);
        Some(at)
    }
}

/// Write `insertion` at `tab`'s cursor; `None` when the tab holds no text.
pub(super) fn insert_at_cursor(
    doc: &mut NotebookDocument,
    tab: &mut NotebookTab,
    insertion: &Insertion,
) -> Option<Inserted> {
    let line = tab.text.as_ref().map(|text| text.cursor().0)?;
    let file_line = doc.file_line(line);
    let text = tab.text.as_mut()?;
    let inserted = insertion_text(insertion, &text.outline, MAIN_FILE, file_line);
    let own_line = !matches!(insertion, Insertion::Reference(_));
    paste(text, &inserted, own_line);
    if doc.load == Load::Missing {
        doc.load = Load::Open;
    }
    Some(inserted)
}

/// The text editor action that types `c`.
#[cfg(test)]
pub(super) fn typed(c: char) -> Action {
    Action::Edit(Edit::Insert(c))
}
