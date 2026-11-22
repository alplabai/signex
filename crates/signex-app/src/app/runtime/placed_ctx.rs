//! The Properties state of the single selected placed object (#2418): its
//! editable geometry and, when a lock holds it, the reason its rows are
//! disabled. A draft survives a refresh only while its object stays selected.
//! A text note's Text box is seeded from the note; a typed box whose note is
//! left is kept as owed to its sheet, and the update loop applies it
//! (`Signex::settle_owed_note_drafts`). Nothing here writes a document.

use super::super::*;

impl Signex {
    pub(super) fn sync_placed_panel(&mut self) {
        let selected = &self.interaction_state.active_canvas().selected;
        let selected_one = match selected.as_slice() {
            [one] => Some(*one),
            _ => None,
        };
        let (item, geometry, locked, end, note) =
            match (selected.as_slice(), self.document_state.active_engine()) {
                ([one], Some(engine)) => {
                    let sheet = engine.document();
                    let item = sheet.selection_unit(*one);
                    let geometry = signex_engine::placed_geometry(sheet, &item);
                    // The lock's own words, as a move would be refused with.
                    let locked = geometry
                        .and_then(|g| {
                            signex_engine::move_to_command(sheet, &item, g.position, false)
                        })
                        .and_then(|command| engine.lock_refusal(&command));
                    let end = Self::run_end(sheet, &item);
                    let is_note = item.kind == signex_types::schematic::SelectedKind::TextNote;
                    let note = (sheet.text_notes.iter())
                        .find(|n| is_note && n.uuid == item.uuid)
                        .map(|n| (item, n.text.clone(), n.font_size));
                    (geometry.map(|_| item), geometry, locked, end, note)
                }
                _ => (None, None, None, None, None),
            };
        let sheet_path = self.document_state.active_path.clone();
        let run_net = match (item, sheet_path.as_deref()) {
            (Some(run), Some(sheet))
                if matches!(
                    run.kind,
                    signex_types::schematic::SelectedKind::Wire
                        | signex_types::schematic::SelectedKind::Bus
                ) =>
            {
                self.run_net_of(sheet, run.uuid)
            }
            _ => None,
        };
        let placed = &mut self.document_state.panel_ctx.placed;
        placed.run_net = run_net;
        if placed.draft.as_ref().is_some_and(|d| Some(d.item) != item) {
            placed.draft = None;
        }
        placed.item = item;
        placed.geometry = geometry;
        placed.end = end;
        placed.locked = locked;
        placed.note_size_mm = note.as_ref().map(|(_, _, size)| *size);
        let selected_note = match (&note, sheet_path.as_deref()) {
            (Some((item, text, _)), Some(sheet)) => Some((sheet, *item, text.as_str())),
            _ => None,
        };
        placed.sync_note(selected_note);
        placed.sync_port_name(sheet_path.as_deref(), selected_one);
    }
}
