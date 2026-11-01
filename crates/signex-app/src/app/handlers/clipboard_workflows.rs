use iced::Task;

use super::super::*;

/// Fixed offset of a Duplicate, and the fallback of Smart Paste.
pub(crate) const DEFAULT_PASTE_OFFSET_MM: f64 = 5.08;

fn smart_paste_offset(app: &Signex) -> (f64, f64) {
    let default_offset = DEFAULT_PASTE_OFFSET_MM;
    let Some(snapshot) = app.active_render_snapshot() else {
        return (default_offset, default_offset);
    };
    let Some(copied) = app.interaction_state.clipboard.as_ref() else {
        return (default_offset, default_offset);
    };
    let Some(clipboard_min_x) = copied.bounds.map(|b| b.min_x) else {
        return (default_offset, default_offset);
    };
    let Some(content_bounds) = snapshot.content_bounds() else {
        return (default_offset, default_offset);
    };

    let margin = crate::canvas::grid::pitch_mm_exact(app.ui_state.grid_size_mm.max(2.54));
    let offset_x = (content_bounds.max_x - clipboard_min_x) + margin;
    (offset_x, margin)
}

/// What a Copy did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CopyOutcome {
    /// The in-app copy now holds the selection.
    Carried,
    /// Nothing in the selection can be carried; the in-app copy was cleared.
    NothingToCarry,
    /// No copy was made (nothing selected, no sheet, or an error that was
    /// logged); the in-app copy is as it was.
    Failed,
}

impl Signex {
    /// Cut is copy + delete, but Copy (`collect_selection_clipboard`)
    /// silently drops kinds it can't carry — `ChildSheet`/`SheetPin`
    /// today. Deleting those anyway would destroy them with nothing in
    /// the clipboard to restore on paste, so only the clipboard-carriable
    /// subset of the selection is cut; anything else stays selected and
    /// untouched (#341 — sheet clipboard support itself stays out of
    /// scope; Cut just can't silently eat what Copy drops).
    pub(crate) fn handle_selection_cut_requested(&mut self) -> Task<Message> {
        let selected = self.interaction_state.active_canvas().selected.clone();
        let (carriable, mut kept) = signex_engine::partition_cuttable(&selected);
        // Delete skips locked symbols; Cut must too, or Copy would overwrite
        // the clipboard with a symbol that then stays on the sheet (#1250).
        let (cuttable, skipped) = match self.document_state.active_engine() {
            Some(engine) => {
                let (command, skipped) =
                    engine.without_locked(signex_engine::Command::DeleteSelection {
                        items: carriable.clone(),
                    });
                match command {
                    Some(signex_engine::Command::DeleteSelection { items }) => (items, skipped),
                    _ => (Vec::new(), skipped),
                }
            }
            None => (carriable.clone(), Vec::new()),
        };
        Self::report_locked_skips(&skipped);
        kept.extend(carriable.into_iter().filter(|i| !cuttable.contains(i)));
        if cuttable.is_empty() {
            return Task::none();
        }

        self.interaction_state.active_canvas_mut().selected = cuttable;
        let previous_copy = self.interaction_state.clipboard.clone();
        let previous_text = self.interaction_state.clipboard_text.clone();
        // Cut deletes only what the copy now holds: if the copy failed (the
        // reason is in the Messages panel), nothing is deleted.
        if self.handle_selection_copy_requested() != CopyOutcome::Carried {
            crate::diagnostics::log_warning(
                "Cut cancelled: the selection could not be copied, so nothing was deleted.",
            );
            self.interaction_state.clipboard = previous_copy;
            self.interaction_state.clipboard_text = previous_text;
            self.interaction_state.active_canvas_mut().selected = selected;
            return Task::none();
        }
        // The system clipboard is written only once the delete went through;
        // a failed delete leaves the earlier copy in charge.
        if !self.handle_selection_delete_requested() {
            crate::diagnostics::log_warning(
                "Cut cancelled: the selection could not be deleted, so nothing was copied.",
            );
            self.interaction_state.clipboard = previous_copy;
            self.interaction_state.clipboard_text = previous_text;
            self.interaction_state.active_canvas_mut().selected = selected;
            return Task::none();
        }
        self.interaction_state
            .active_canvas_mut()
            .selected
            .extend(kept);
        self.clipboard_write_task()
    }

    /// Duplicate copies the selection (the copy is the clipboard, as for any
    /// Copy) and places it at once, shifted by a fixed offset.
    pub(crate) fn handle_selection_duplicate_requested(&mut self) -> Task<Message> {
        if self.interaction_state.active_canvas().selected.is_empty()
            || !self.has_active_schematic()
        {
            return self.finish_update();
        }

        // A failed copy must not paste whatever was on the clipboard before.
        let outcome = self.handle_selection_copy_requested();
        let write = self.mirror_copy_on_clipboard_task(outcome);
        if outcome != CopyOutcome::Carried {
            return Task::batch([write, self.finish_update()]);
        }
        // The offset paste itself happens once Next free numbering is ready.
        Task::batch([write, self.begin_paste(crate::app::PasteIntent::Duplicate)])
    }

    pub(crate) fn handle_clipboard_smart_paste_requested(&mut self) {
        let (offset_x, offset_y) = smart_paste_offset(self);
        self.paste_with_offset(offset_x, offset_y);
    }

    /// Copy the selection to the in-app clipboard; see [`CopyOutcome`].
    pub(crate) fn handle_selection_copy_requested(&mut self) -> CopyOutcome {
        let Some(engine) = self.document_state.active_engine() else {
            return CopyOutcome::Failed;
        };
        let selected = &self.interaction_state.active_canvas().selected;
        if selected.is_empty() {
            return CopyOutcome::Failed;
        }
        let grid = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
        match engine.copy_selection(selected, grid) {
            Ok(Some(copied)) => {
                self.interaction_state.clipboard = Some(copied);
                CopyOutcome::Carried
            }
            Ok(None) => {
                crate::diagnostics::log_info(
                    "Copy: nothing in the selection can be copied (sheet symbols and sheet pins are not carried); the clipboard was emptied.",
                );
                self.interaction_state.clipboard = None;
                self.interaction_state.clipboard_text = None;
                CopyOutcome::NothingToCarry
            }
            Err(error) => {
                crate::diagnostics::log_warning(format!("Copy failed: {error}."));
                CopyOutcome::Failed
            }
        }
    }

    /// Paste: the copied content attaches to the cursor, Space / X / Y turn
    /// it, and one click places it as one undo step. A paste is single-shot.
    pub(crate) fn handle_clipboard_paste_requested(&mut self) {
        self.arm_paste_session(crate::app::state::PlacementKind::Paste);
    }

    /// Attach the copy to the cursor as a `kind` session (a paste or a
    /// stamp). Designators are given at each click, not here.
    pub(crate) fn arm_paste_session(&mut self, kind: crate::app::state::PlacementKind) {
        let Some((payload, target, _origin)) = self.prepared_paste() else {
            return;
        };
        // A library read still in flight must not replace the paste on the
        // cursor when it finishes: a paste is the newest gesture.
        self.interaction_state.placement_epoch += 1;
        self.arm_placement_session(payload, &target, kind);
        self.refresh_paste_ghost();
    }

    /// Smart Paste and Duplicate: place the copy now, shifted by a fixed
    /// offset from where it was copied, as one undo step.
    pub(crate) fn paste_with_offset(&mut self, offset_x: f64, offset_y: f64) {
        let Some((payload, target, origin)) = self.prepared_paste() else {
            return;
        };
        let payload = self.paste_numbering(&target).apply(&payload);
        let at = signex_types::schematic::Point::new(origin.x + offset_x, origin.y + offset_y);
        match payload.place_commands(at) {
            Ok(commands) if !commands.is_empty() => {
                self.apply_engine_commands(commands, true, true);
            }
            Ok(_) => {}
            Err(error) => crate::diagnostics::log_warning(format!(
                "Paste failed: the copy could not be placed ({error})."
            )),
        }
    }

    /// The clipboard as it will land on the active sheet: the payload with
    /// the sheet's own copy of any body it already holds (so a ghost shows
    /// what lands), the sheet, and where the copy came from. `None` when
    /// there is nothing to paste or no sheet is active (silent: there is
    /// nothing to say), or when the sheet is read-only (reported in the
    /// Messages panel and as a notice).
    pub(crate) fn prepared_paste(
        &mut self,
    ) -> Option<(
        signex_engine::PlacementPayload,
        std::path::PathBuf,
        signex_types::schematic::Point,
    )> {
        let copied = self.interaction_state.clipboard.clone()?;
        let target = self.document_state.active_path.clone()?;
        let sheet = self.document_state.engines.get(&target)?.document();
        if sheet.unknown.newer_minor_source.is_some() {
            let detail = format!(
                "The sheet {} was written by a newer Signex and is read-only in this build; nothing was pasted.",
                target.display()
            );
            crate::diagnostics::log_warning(format!("Paste failed: {detail}"));
            self.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::new("Paste Failed", detail));
            return None;
        }
        let (payload, differing) = copied.payload.adopt_sheet_bodies(sheet);
        for id in differing {
            crate::diagnostics::log_warning(format!(
                "Library symbol {id} in the clipboard differs from the copy this sheet already holds; the sheet keeps its existing copy, so the pasted parts use it. Select a pasted part and use \"Update from library\" in Properties to adopt a newer one."
            ));
        }
        Some((payload, target, copied.origin))
    }
}
