//! The Position, Rotation, Size and Parameters rows of a placed object in
//! Properties (#2418). A box's draft commits on Enter through the same
//! method as the `set_object_*` commands: one edit, one undo step. A refused
//! edit keeps the draft and says why in Messages. A text note's multi-line
//! Text box keeps its own draft and applies it as one edit (`NoteSubmitted`,
//! or when another object is selected).

use super::super::super::*;
use crate::app::command::PlacedRefusal;
use crate::panels::{NoteDraft, PlacedDraft, PlacedEditMsg, PlacedField};
use signex_types::schematic::{Point, SelectedItem};

impl Signex {
    pub(super) fn handle_dock_placed_edit_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        let crate::panels::PanelMsg::PlacedEdit(msg) = panel_msg else {
            return false;
        };
        match msg {
            PlacedEditMsg::Typed { item, field, text } => {
                self.document_state.panel_ctx.placed.draft = Some(PlacedDraft {
                    item: *item,
                    field: field.clone(),
                    text: text.clone(),
                });
            }
            PlacedEditMsg::Submitted { item, field } => {
                let draft = (self.document_state.panel_ctx.placed.draft.clone())
                    .filter(|d| d.item == *item && d.field == *field);
                let Some(draft) = draft else {
                    return true;
                };
                match self.commit_placed_field(*item, field, &draft.text) {
                    Ok(()) => self.document_state.panel_ctx.placed.draft = None,
                    // The draft stays, so the user can fix the text.
                    Err(refusal) => say_refusal(&refusal),
                }
            }
            PlacedEditMsg::RotationPicked { item, degrees } => {
                if let Err(refusal) = self.set_placed_rotation(*item, *degrees) {
                    say_refusal(&refusal);
                }
            }
            PlacedEditMsg::NoteActed { item, action } => {
                let note = self.document_state.panel_ctx.placed.note.as_mut();
                if let Some(draft) = note.filter(|d| d.item == *item) {
                    draft.edited |= action.is_edit();
                    draft.content.perform(action.clone());
                }
            }
            PlacedEditMsg::NoteSubmitted { item } => {
                let note = self.document_state.panel_ctx.placed.note.as_mut();
                if let Some(draft) = note.filter(|d| d.item == *item && d.edited) {
                    // Marked applied first: the selection refresh the edit
                    // runs must keep this box, not hand it back again.
                    draft.edited = false;
                    let text = draft.content.text();
                    self.apply_note_text(*item, text);
                }
            }
            PlacedEditMsg::NoteLayoutPicked { item, layout } => {
                let layout = *layout;
                if let Err(refusal) = self.edit_text_note(*item, |note| layout.apply(note)) {
                    say_refusal(&refusal);
                }
            }
        }
        true
    }

    /// Apply a draft typed into a text note that is no longer selected, so
    /// leaving the box never drops what was typed.
    pub(crate) fn apply_note_handoff(&mut self, handoff: NoteDraft) {
        let on_sheet = self.document_state.active_engine().is_some_and(|engine| {
            (engine.document().text_notes.iter()).any(|n| n.uuid == handoff.item.uuid)
        });
        if on_sheet {
            self.apply_note_text(handoff.item, handoff.content.text());
        } else {
            crate::diagnostics::log_warning(
                "Properties: the text typed into a note was not applied: its sheet is no longer \
                 the active one",
            );
        }
    }

    /// Set the text of the note `item` to `text`: one edit, one undo step.
    fn apply_note_text(&mut self, item: SelectedItem, text: String) {
        self.apply_engine_command(
            signex_engine::Command::UpdateText {
                target: signex_engine::TextTarget::TextNote(item.uuid),
                value: text,
            },
            true,
            true,
        );
    }

    /// The edit the text typed into `field` of `item` asks for. A length is
    /// read in the panel's unit, and Y from the sheet's page origin.
    fn commit_placed_field(
        &mut self,
        item: SelectedItem,
        field: &PlacedField,
        text: &str,
    ) -> Result<(), PlacedRefusal> {
        if let PlacedField::Parameter(name) = field {
            return self.set_symbol_parameter(item, name, text).map(|_| ());
        }
        let unit = self.ui_state.unit;
        let typed = signex_types::measure::parse_length_field_mm(text, unit)
            .ok_or_else(|| PlacedRefusal::new(format!("`{}` is not a number", text.trim())))?;
        let engine = self
            .document_state
            .edit_engine()
            .ok_or_else(|| PlacedRefusal::new("no schematic is open"))?;
        let sheet = engine.document();
        let geometry = signex_engine::placed_geometry(sheet, &item)
            .ok_or_else(|| PlacedRefusal::new("the object is no longer on this sheet"))?;
        let at = geometry.position;
        let end = Self::run_end(sheet, &item);
        let size = geometry.size.unwrap_or_default();
        // The page origin's mapping is its own inverse.
        let stored_y = sheet.page_y(typed);
        let placed = match field {
            PlacedField::X => self.set_placed_position(item, Point::new(typed, at.y)),
            PlacedField::Y => self.set_placed_position(item, Point::new(at.x, stored_y)),
            PlacedField::EndX | PlacedField::EndY => {
                let end = end
                    .ok_or_else(|| PlacedRefusal::new("only a wire or a bus has an end to set"))?;
                let to = if *field == PlacedField::EndX {
                    Point::new(typed, end.y)
                } else {
                    Point::new(end.x, stored_y)
                };
                self.set_run_end(item, to)
            }
            PlacedField::TextSize if typed > 0.0 => {
                self.edit_text_note(item, |note| note.font_size = typed)
            }
            PlacedField::TextSize => {
                return Err(PlacedRefusal::new("a text size must be above zero"));
            }
            PlacedField::Width => self.set_sheet_symbol_size(item, (typed, size.1)),
            PlacedField::Height => self.set_sheet_symbol_size(item, (size.0, typed)),
            PlacedField::Parameter(_) => return Ok(()),
        };
        placed.map(|_| ())
    }
}

/// Put a refusal in Messages unless the gateway already did.
fn say_refusal(refusal: &PlacedRefusal) {
    if !refusal.logged {
        crate::diagnostics::log_warning(format!("Properties: {}", refusal.reason));
    }
}
