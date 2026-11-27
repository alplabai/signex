//! The Position, Rotation, Size and Parameters rows of a placed object in
//! Properties (#2418). A box's draft commits on Enter through the same
//! method as the `set_object_*` commands: one edit, one undo step. A refused
//! edit keeps the draft and says why in Messages. A wire's or bus's Start
//! and End each stretch their own end only. A text note's multi-line Text
//! box keeps its own draft, tied to its sheet and note, and applies it as
//! one edit to that sheet: on `NoteSubmitted`, when the note is left (the
//! update loop settles it), and before a save, a close or a quit.

use std::path::Path;

use super::super::super::*;
use crate::app::command::PlacedRefusal;
use crate::panels::{NoteDraft, PlacedDraft, PlacedEditMsg, PlacedField, PortNameDraft};
use signex_i18n::{ids, t};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

/// How much of a note's text a "not applied" message quotes.
const QUOTED_NOTE_CHARS: usize = 200;

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
                let keeps = self.ui_state.move_keeps_wires;
                if let Err(refusal) = self.set_placed_rotation(*item, *degrees, keeps) {
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
                    // runs must keep this box, not owe it again.
                    draft.edited = false;
                    let draft = draft.clone();
                    self.apply_note_draft(draft);
                }
            }
            PlacedEditMsg::NoteLayoutPicked { item, layout } => {
                if let Err(refusal) = self.set_text_note_props(*item, layout.props()) {
                    say_refusal(&refusal);
                }
            }
            PlacedEditMsg::PortNameTyped { item, text } => {
                if let Some(sheet) = self.document_state.active_path.clone() {
                    self.document_state.panel_ctx.placed.port_name = Some(PortNameDraft {
                        sheet,
                        item: *item,
                        text: text.clone(),
                    });
                }
            }
            PlacedEditMsg::PortNameSubmitted { item } => {
                let placed = &mut self.document_state.panel_ctx.placed;
                if placed.port_name.as_ref().is_some_and(|d| d.item == *item)
                    && let Some(draft) = placed.port_name.take()
                {
                    self.apply_port_name_draft(draft);
                }
            }
            // Opened with its focus task by the dock router.
            PlacedEditMsg::RichNoteEditRequested { .. } => {}
        }
        true
    }

    /// Apply the typed note and power port boxes whose object is no longer
    /// selected, each to its own sheet. Run by the update loop after every
    /// message, never by a refresh, so a refresh writes no document.
    pub(crate) fn settle_owed_note_drafts(&mut self) {
        let owed = self.take_note_drafts(&|_| true, false);
        for draft in owed {
            self.apply_note_draft(draft);
        }
        for draft in self.take_port_name_drafts(&|_| true, false) {
            self.apply_port_name_draft(draft);
        }
    }

    /// Apply every typed note and power port box of the sheets `matches`
    /// picks, the selected object's included: before a save, a close, an
    /// export or a quit, so what the box shows is what is written.
    pub(crate) fn commit_note_drafts_where(&mut self, matches: &dyn Fn(&Path) -> bool) {
        for draft in self.take_note_drafts(matches, true) {
            self.apply_note_draft(draft);
        }
        for draft in self.take_port_name_drafts(matches, true) {
            let name = draft.text.clone();
            let waiting = self.document_state.rail_rename.queue.len();
            self.apply_port_name_draft(draft);
            // A project's rail needs its closed sheets read first, which a
            // save cannot wait for here.
            if self.document_state.rail_rename.queue.len() > waiting {
                let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
                crate::diagnostics::log_warning(
                    t!(i18n, ids::PROPERTIES_RAIL_RENAME_WAITS, "name" => name.as_str())
                        .into_string(),
                );
            }
        }
    }

    /// Drop the typed note and power port boxes of the sheets `matches`
    /// picks, unapplied: a discard throws them away with the sheet's other
    /// edits, along with a rail rename still waiting on its reads.
    pub(crate) fn drop_note_drafts_where(&mut self, matches: &dyn Fn(&Path) -> bool) {
        drop(self.take_note_drafts(matches, true));
        drop(self.take_port_name_drafts(matches, true));
        self.document_state.rail_rename.drop_where(matches);
    }

    /// The sheets with a typed note or power port box not applied yet, the
    /// owed and the selected ones, in the main window's Properties and every
    /// undocked window's: a scope (a project, an export) adds them to the
    /// sheets it commits.
    pub(crate) fn typed_note_sheets(&self) -> Vec<std::path::PathBuf> {
        let docs = &self.document_state;
        std::iter::once(&docs.panel_ctx.placed)
            .chain(docs.window_panel_ctx.values().map(|ctx| &ctx.placed))
            .flat_map(|placed| {
                let selected = placed.note.iter().filter(|d| d.edited);
                let notes = placed.notes_owed.iter().chain(selected);
                let names = placed.port_names_owed.iter().chain(placed.port_name.iter());
                notes
                    .map(|draft| draft.sheet.clone())
                    .chain(names.map(|draft| draft.sheet.clone()))
            })
            .collect()
    }

    /// The typed power port names of the sheets `matches` picks, from the
    /// main window's Properties and every undocked window's.
    fn take_port_name_drafts(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        selected: bool,
    ) -> Vec<PortNameDraft> {
        let docs = &mut self.document_state;
        std::iter::once(&mut docs.panel_ctx.placed)
            .chain(docs.window_panel_ctx.values_mut().map(|c| &mut c.placed))
            .flat_map(|placed| placed.take_typed_port_names(matches, selected))
            .collect()
    }

    /// Rename a power port through the engine. A named port takes its whole
    /// rail as one undo step (`app/rail_rename`); a power flag or a port with
    /// no name yet is on no rail and changes alone. A port that is gone keeps
    /// nothing, so the typed name is quoted in Messages instead of being
    /// dropped silently.
    fn apply_port_name_draft(&mut self, draft: PortNameDraft) {
        let PortNameDraft { sheet, item, text } = draft;
        if let Some(rename) = self.rail_rename_on(&sheet, item.uuid, &text) {
            self.rename_rail(rename);
            return;
        }
        let on_sheet = self
            .document_state
            .engines
            .get(&sheet)
            .is_some_and(|e| e.document().symbols.iter().any(|s| s.uuid == item.uuid));
        if !on_sheet {
            let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
            let shown = sheet.display().to_string();
            crate::diagnostics::log_warning(
                t!(i18n, ids::PROPERTIES_PORT_NAME_NOT_APPLIED,
                    "sheet" => shown.as_str(),
                    "name" => text.as_str())
                .into_string(),
            );
            return;
        }
        let command = signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::SymbolValue(item.uuid),
            value: text,
        };
        if self.document_state.active_path.as_deref() == Some(sheet.as_path()) {
            self.apply_engine_command(command, true, true);
        } else {
            self.apply_command_to_sheet(&sheet, command);
        }
    }

    /// The owed note boxes (and with `selected` the typed selected boxes) of
    /// the sheets `matches` picks, from the main window's Properties and
    /// every undocked window's.
    fn take_note_drafts(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        selected: bool,
    ) -> Vec<NoteDraft> {
        let docs = &mut self.document_state;
        let panels = std::iter::once(&mut docs.panel_ctx.placed).chain(
            docs.window_panel_ctx
                .values_mut()
                .map(|ctx| &mut ctx.placed),
        );
        let mut taken = Vec::new();
        for placed in panels {
            if selected {
                taken.extend(placed.take_typed_notes(matches));
            } else {
                let owed = std::mem::take(&mut placed.notes_owed);
                let (mine, kept): (Vec<_>, Vec<_>) =
                    owed.into_iter().partition(|d| matches(&d.sheet));
                placed.notes_owed = kept;
                taken.extend(mine);
            }
        }
        taken
    }

    /// Set the text of the note a draft belongs to, on its own sheet: one
    /// edit, one undo step there. A note that is gone keeps nothing, so the
    /// typed text is quoted in Messages instead of being dropped silently.
    fn apply_note_draft(&mut self, draft: NoteDraft) {
        let text = draft.content.text();
        let sheet = draft.sheet;
        let on_sheet = self
            .document_state
            .engines
            .get(&sheet)
            .is_some_and(|engine| {
                (engine.document().text_notes.iter()).any(|n| n.uuid == draft.item.uuid)
            });
        if !on_sheet {
            let quoted: String = text.chars().take(QUOTED_NOTE_CHARS).collect();
            crate::diagnostics::log_warning(format!(
                "Properties: the text typed into a note of {} was not applied, the note is no \
                 longer there. The text was: {quoted}",
                sheet.display()
            ));
            return;
        }
        let command = signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::TextNote(draft.item.uuid),
            value: text,
        };
        if self.document_state.active_path.as_deref() == Some(sheet.as_path()) {
            self.apply_engine_command(command, true, true);
        } else {
            self.apply_command_to_sheet(&sheet, command);
        }
    }

    /// The edit the text typed into `field` of `item` asks for. A length is
    /// read in the panel's unit, and Y from the sheet's page origin. A
    /// wire's or bus's X / Y is its start, stretched as End stretches the
    /// end.
    fn commit_placed_field(
        &mut self,
        item: SelectedItem,
        field: &PlacedField,
        text: &str,
    ) -> Result<(), PlacedRefusal> {
        if let PlacedField::Parameter(name) = field {
            return self.set_symbol_parameter(item, name, text).map(|_| ());
        }
        if let PlacedField::NoteLength(length) = field {
            return self.commit_note_length(item, *length, text);
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
        let ends = signex_engine::run_ends(sheet, &item);
        let size = geometry.size.unwrap_or_default();
        // The page origin's mapping is its own inverse.
        let stored_y = sheet.page_y(typed);
        let is_run = matches!(item.kind, SelectedKind::Wire | SelectedKind::Bus);
        let keeps = self.ui_state.move_keeps_wires;
        let placed = match field {
            PlacedField::X if is_run => self.set_run_end(item, true, Point::new(typed, at.y)),
            PlacedField::Y if is_run => self.set_run_end(item, true, Point::new(at.x, stored_y)),
            PlacedField::X => self.set_placed_position(item, Point::new(typed, at.y), keeps),
            PlacedField::Y => self.set_placed_position(item, Point::new(at.x, stored_y), keeps),
            PlacedField::EndX | PlacedField::EndY => {
                let (_, end) = ends
                    .ok_or_else(|| PlacedRefusal::new("only a wire or a bus has an end to set"))?;
                let to = if *field == PlacedField::EndX {
                    Point::new(typed, end.y)
                } else {
                    Point::new(end.x, stored_y)
                };
                self.set_run_end(item, false, to)
            }
            PlacedField::TextSize if typed > 0.0 => self.set_text_note_props(
                item,
                signex_engine::TextNoteProps {
                    font_size_mm: Some(typed),
                    ..Default::default()
                },
            ),
            PlacedField::TextSize => {
                return Err(PlacedRefusal::new("a text size must be above zero"));
            }
            PlacedField::Width => self.set_sheet_symbol_size(item, (typed, size.1)),
            PlacedField::Height => self.set_sheet_symbol_size(item, (size.0, typed)),
            PlacedField::Parameter(_) | PlacedField::NoteLength(_) => return Ok(()),
        };
        placed.map(|_| ())
    }

    /// A rich note's Border, Padding, Width or Height box, read in the
    /// panel's unit; an empty or `auto` Border is Auto. One undo step.
    fn commit_note_length(
        &mut self,
        item: SelectedItem,
        length: crate::panels::NoteLength,
        text: &str,
    ) -> Result<(), PlacedRefusal> {
        use crate::panels::NoteLength;
        let unit = self.ui_state.unit;
        let auto = text.trim().is_empty() || text.trim().eq_ignore_ascii_case("auto");
        let typed = || {
            signex_types::measure::parse_length_field_mm(text, unit)
                .filter(|mm| mm.is_finite() && *mm >= 0.0)
                .ok_or_else(|| PlacedRefusal::new(format!("`{}` is not a length", text.trim())))
        };
        let mut layout = self.text_note_layout(item).map_err(PlacedRefusal::new)?;
        match length {
            NoteLength::Border if auto => layout.border_mm = None,
            NoteLength::Border => layout.border_mm = Some(typed()?),
            NoteLength::Padding => layout.padding_mm = typed()?,
            NoteLength::Width => layout.width_mm = typed()?,
            NoteLength::Height => layout.height_mm = typed()?,
        }
        self.set_text_note_layout(item, layout).map(|_| ())
    }
}

/// Put a refusal in Messages unless the gateway already did.
fn say_refusal(refusal: &PlacedRefusal) {
    if !refusal.logged {
        crate::diagnostics::log_warning(format!("Properties: {}", refusal.reason));
    }
}
