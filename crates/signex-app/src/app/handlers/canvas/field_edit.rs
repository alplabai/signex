//! In-place editing of an object's text (#1255, #1670): double-click and the
//! `edit_*` commands open the one inline editor on a label, a text note or a
//! part's designator, value or footprint, and Enter commits it through the
//! same engine edit Properties uses.

use iced::Task;
use iced::widget::operation;
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::super::super::*;

/// The editor's placeholder, which names the field being edited.
pub(crate) fn editor_placeholder(kind: SelectedKind) -> &'static str {
    match kind {
        SelectedKind::SymbolRefField => "Designator",
        SelectedKind::SymbolValField => "Value",
        SelectedKind::Symbol => "Footprint",
        _ => "",
    }
}

/// The item a `target` command edits when pointed at `item`, if it can.
fn item_for(target: TextEditTarget, item: SelectedItem) -> Option<SelectedItem> {
    use SelectedKind::*;
    let part = matches!(item.kind, Symbol | SymbolRefField | SymbolValField);
    let kind = match target {
        TextEditTarget::Value if part => SymbolValField,
        TextEditTarget::Designator if part => SymbolRefField,
        TextEditTarget::FootprintField if part => Symbol,
        TextEditTarget::Text => match item.kind {
            Label | TextNote | SymbolRefField | SymbolValField => item.kind,
            Symbol => SymbolRefField,
            _ => return None,
        },
        _ => return None,
    };
    Some(SelectedItem::new(item.uuid, kind))
}

/// What the user is told when `target` finds nothing to edit.
fn nothing_to_edit(target: TextEditTarget) -> &'static str {
    match target {
        TextEditTarget::Value => "Edit value: point at a part, or select one part, first",
        TextEditTarget::Designator => {
            "Edit reference designator: point at a part, or select one part, first"
        }
        TextEditTarget::FootprintField => {
            "Edit footprint field: point at a part, or select one part, first"
        }
        TextEditTarget::Text => {
            "Edit text in place: point at a label, a text note or a part, or select one, first"
        }
    }
}

impl Signex {
    /// Focus the editor that just opened and select its text.
    fn focus_text_edit() -> Task<Message> {
        Task::batch([
            operation::focus(crate::app::contracts::TEXT_EDIT_INPUT_ID.clone()),
            operation::select_all(crate::app::contracts::TEXT_EDIT_INPUT_ID.clone()),
        ])
    }

    /// Open the editor on `item`. `false` when the item has no text to edit
    /// on the active sheet.
    pub(crate) fn begin_text_edit(&mut self, item: SelectedItem) -> bool {
        let Some(engine) = self.document_state.active_engine() else {
            return false;
        };
        let sheet = engine.document();
        // A power port's designator is never drawn; its text is the net name,
        // so a designator edit becomes the Value edit Properties uses (#1799).
        let item = match item.kind {
            SelectedKind::SymbolRefField
                if sheet
                    .symbols
                    .iter()
                    .any(|s| s.uuid == item.uuid && s.is_power) =>
            {
                SelectedItem::new(item.uuid, SelectedKind::SymbolValField)
            }
            _ => item,
        };
        let sheet_path = self.document_state.active_path.clone();
        let state = |text: String, at: signex_types::schematic::Point| TextEditState {
            uuid: item.uuid,
            kind: item.kind,
            original_text: text.clone(),
            text,
            world_x: at.x,
            world_y: at.y,
            sheet: sheet_path.clone(),
            placement: sheet_path
                .as_deref()
                .filter(|_| item.kind == SelectedKind::SymbolRefField)
                .and_then(|p| self.shown_placement(p)),
        };
        let opened = match item.kind {
            SelectedKind::Label => sheet
                .labels
                .iter()
                .find(|l| l.uuid == item.uuid)
                .map(|l| state(l.text.clone(), l.position)),
            SelectedKind::TextNote => sheet
                .text_notes
                .iter()
                .find(|t| t.uuid == item.uuid)
                .map(|t| state(t.text.clone(), t.position)),
            SelectedKind::SymbolRefField | SelectedKind::SymbolValField | SelectedKind::Symbol => {
                sheet.symbols.iter().find(|s| s.uuid == item.uuid).map(|s| {
                    let (text, field) = match item.kind {
                        SelectedKind::SymbolRefField => {
                            (self.shown_designator(s).to_owned(), s.ref_text.as_ref())
                        }
                        SelectedKind::SymbolValField => (s.value.clone(), s.val_text.as_ref()),
                        _ => (s.footprint.clone(), None),
                    };
                    let at = field
                        .filter(|f| !f.hidden)
                        .map_or(s.position, |f| f.position);
                    state(text, at)
                })
            }
            _ => None,
        };
        let Some(opened) = opened else {
            return false;
        };
        self.interaction_state.editing_text = Some(opened);
        true
    }

    /// The part the selection is, when it is one part (its body, its
    /// designator or its value, alone or together, as a box select takes
    /// them), else the one selected object.
    fn single_selected_item(&self) -> Option<SelectedItem> {
        let selected = &self.interaction_state.active_canvas().selected;
        let first = selected.first()?;
        let is_part = |i: &SelectedItem| {
            matches!(
                i.kind,
                SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField
            )
        };
        if selected.iter().all(|i| is_part(i) && i.uuid == first.uuid) {
            return Some(SelectedItem::new(first.uuid, SelectedKind::Symbol));
        }
        (selected.len() == 1).then_some(*first)
    }

    /// The item `target` edits: the object under the cursor, else the single
    /// selection.
    fn text_edit_item(&self, target: TextEditTarget) -> Option<SelectedItem> {
        let under_cursor = self
            .cursor_world_now()
            .and_then(|(x, y)| self.item_hit_at(x, y));
        under_cursor
            .and_then(|item| item_for(target, item))
            .or_else(|| {
                self.single_selected_item()
                    .and_then(|i| item_for(target, i))
            })
    }

    /// A field-edit command (F2, V, U, F): open the editor, or say in the
    /// Messages panel why not.
    pub(crate) fn request_text_edit(&mut self, target: TextEditTarget) -> Task<Message> {
        if !self.has_active_schematic() {
            crate::diagnostics::log_warning("Edit in place works on a schematic sheet");
            return Task::none();
        }
        if let Some(reason) = self.refuse_if_active_read_only() {
            crate::diagnostics::log_warning(reason);
            return Task::none();
        }
        // Text already typed in an open editor is kept, as a canvas click does.
        if let Some(open) = self.interaction_state.editing_text.take() {
            self.commit_text_edit(open);
        }
        let opened = self
            .text_edit_item(target)
            .is_some_and(|item| self.begin_text_edit(item));
        if !opened {
            crate::diagnostics::log_warning(nothing_to_edit(target));
            return Task::none();
        }
        Self::focus_text_edit()
    }

    /// Double-click on a field: open its editor. `None` when `item` has no
    /// text to edit.
    pub(super) fn open_text_edit_on_double_click(
        &mut self,
        item: SelectedItem,
    ) -> Option<Task<Message>> {
        let editable = matches!(
            item.kind,
            SelectedKind::Label
                | SelectedKind::TextNote
                | SelectedKind::SymbolRefField
                | SelectedKind::SymbolValField
        );
        (editable && self.begin_text_edit(item)).then(Self::focus_text_edit)
    }

    /// Commit the editor's text through the engine edit Properties uses; one
    /// undo step. An unchanged text records nothing.
    pub(crate) fn commit_text_edit(&mut self, state: TextEditState) {
        if state.text == state.original_text {
            return;
        }
        if let Some(why) = self.stale_text_edit(&state) {
            crate::diagnostics::log_warning(format!("In-place edit discarded: {why}"));
            return;
        }
        let uuid = state.uuid;
        let text_command = |target| signex_engine::Command::UpdateText {
            target,
            value: state.text.clone(),
        };
        let commands = match state.kind {
            SelectedKind::Label => vec![text_command(signex_engine::TextTarget::Label(uuid))],
            SelectedKind::TextNote => vec![text_command(signex_engine::TextTarget::TextNote(uuid))],
            SelectedKind::SymbolRefField => self.designator_edit_commands(uuid, state.text.clone()),
            SelectedKind::SymbolValField => {
                vec![text_command(signex_engine::TextTarget::SymbolValue(uuid))]
            }
            SelectedKind::Symbol => vec![signex_engine::Command::UpdateSymbolFootprint {
                symbol_id: uuid,
                footprint: state.text.clone(),
            }],
            _ => return,
        };
        self.apply_engine_commands(commands, true, true);
    }

    /// Why `state` can no longer be committed, if it cannot: its sheet or
    /// placement is not the one in view, or its object is gone.
    fn stale_text_edit(&self, state: &TextEditState) -> Option<&'static str> {
        let active = self.document_state.active_path.as_deref();
        if state.sheet.as_deref() != active {
            return Some("another sheet is in view now");
        }
        if state.kind == SelectedKind::SymbolRefField
            && active.and_then(|p| self.shown_placement(p)) != state.placement
        {
            return Some("the sheet is shown through another placement now");
        }
        let sheet = self.document_state.active_engine()?.document();
        let exists = match state.kind {
            SelectedKind::Label => sheet.labels.iter().any(|l| l.uuid == state.uuid),
            SelectedKind::TextNote => sheet.text_notes.iter().any(|t| t.uuid == state.uuid),
            _ => sheet.symbols.iter().any(|s| s.uuid == state.uuid),
        };
        (!exists).then_some("the object no longer exists")
    }

    /// Close the editor when the document under it changed (undo, redo, a
    /// placement switch), saying so when typed text is dropped.
    pub(crate) fn close_text_edit(&mut self, why: &str) {
        if let Some(state) = self.interaction_state.editing_text.take()
            && state.text != state.original_text
        {
            crate::diagnostics::log_warning(format!("In-place edit discarded: {why}"));
        }
    }
}
