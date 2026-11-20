use iced::Task;
use signex_i18n::RenderTranslatable;

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
        crate::diagnostics::log_locked_skips(&skipped);
        kept.extend(carriable.into_iter().filter(|i| !cuttable.contains(i)));
        if cuttable.is_empty() {
            return Task::none();
        }

        self.interaction_state.active_canvas_mut().selected = cuttable;
        let previous_copy = self.interaction_state.clipboard.clone();
        let previous_text = self.interaction_state.clipboard_text.clone();
        let previous_board_copy = self.interaction_state.pcb_clipboard.clone();
        // Cut deletes only what the copy now holds: if the copy failed (the
        // reason is in the Messages panel), nothing is deleted.
        if self.handle_selection_copy_requested() != CopyOutcome::Carried {
            crate::diagnostics::log_warning(
                "Cut cancelled: the selection could not be copied, so nothing was deleted.",
            );
            self.interaction_state.clipboard = previous_copy;
            self.interaction_state.clipboard_text = previous_text;
            self.interaction_state.pcb_clipboard = previous_board_copy;
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
            self.interaction_state.pcb_clipboard = previous_board_copy;
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
                self.interaction_state.pcb_clipboard = None;
                CopyOutcome::Carried
            }
            Ok(None) => {
                crate::diagnostics::log_info(
                    "Copy: nothing in the selection can be copied (sheet symbols and sheet pins are not carried); the clipboard was emptied.",
                );
                self.interaction_state.clipboard = None;
                self.interaction_state.clipboard_text = None;
                self.interaction_state.pcb_clipboard = None;
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
        match payload.place_commands(at, &mut signex_types::object_id::UuidV7Ids) {
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
        if self.interaction_state.clipboard.is_none()
            && self.interaction_state.pcb_clipboard.is_some()
        {
            crate::diagnostics::log_warning(
                "Paste: the clipboard holds board objects, which cannot be pasted on a schematic; nothing was pasted.",
            );
            return None;
        }
        let copied = self.interaction_state.clipboard.clone()?;
        let target = self.document_state.active_path.clone()?;
        let sheet = self.document_state.engines.get(&target)?.document();
        if sheet.unknown.newer_minor_source.is_some() {
            let detail = signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_PASTE_READ_ONLY)
                .arg("path", target.display().to_string());
            crate::diagnostics::log_warning(format!("Paste failed: {}", detail.render_en_us()));
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::new(
                signex_i18n::ids::NOTICE_PASTE_FAILED,
                detail,
            ));
            return None;
        }
        let payload = copied.payload.clone();
        Some((payload, target, copied.origin))
    }
}

/// The paste-numbering rule of a Smart Paste choice.
fn designator_rule(
    choice: crate::smart_paste::DesignatorChoice,
) -> crate::paste_prefs::PasteDesignators {
    match choice {
        crate::smart_paste::DesignatorChoice::NextFree => {
            crate::paste_prefs::PasteDesignators::NextFree
        }
        crate::smart_paste::DesignatorChoice::KeepIfFree => {
            crate::paste_prefs::PasteDesignators::KeepIfFree
        }
    }
}

/// A drawing's box in mm: its minimum and maximum corners.
type PreviewBox = ((f64, f64), (f64, f64));

/// The drawables of a copied fragment for the Smart Paste drawing, and
/// their box.
fn preview_shapes(
    fragment: &signex_types::schematic::SchematicSheet,
) -> (Vec<crate::smart_paste::PreviewShape>, PreviewBox) {
    use crate::smart_paste::PreviewShape;
    use signex_types::schematic::{SelectedItem, SelectedKind};
    let mut shapes = Vec::new();
    for w in &fragment.wires {
        shapes.push(PreviewShape::Line {
            from: (w.start.x, w.start.y),
            to: (w.end.x, w.end.y),
        });
    }
    let parts: Vec<SelectedItem> = fragment
        .symbols
        .iter()
        .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
        .collect();
    let boxes = crate::schematic_runtime::selection::selection_boxes(
        fragment,
        &parts,
        &crate::schematic_runtime::Designators::new(),
        crate::schematic_runtime::WireWidths::default(),
    );
    for (symbol, b) in fragment.symbols.iter().zip(boxes) {
        let Some(b) = b else { continue };
        let reference = if symbol.is_power {
            symbol.value.clone()
        } else {
            symbol.reference.clone()
        };
        shapes.push(PreviewShape::Part {
            min: (b.min_x, b.min_y),
            max: (b.max_x, b.max_y),
            reference,
        });
    }
    for l in &fragment.labels {
        shapes.push(PreviewShape::Label {
            at: (l.position.x, l.position.y),
            net: l.text.clone(),
            power: crate::smart_paste::plan::is_power_label(l.label_type),
        });
    }
    let mut lo = (f64::MAX, f64::MAX);
    let mut hi = (f64::MIN, f64::MIN);
    let mut grow = |x: f64, y: f64| {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    };
    for shape in &shapes {
        match shape {
            PreviewShape::Line { from, to } => {
                grow(from.0, from.1);
                grow(to.0, to.1);
            }
            PreviewShape::Part { min, max, .. } => {
                grow(min.0, min.1);
                grow(max.0, max.1);
            }
            PreviewShape::Label { at, .. } => grow(at.0, at.1),
        }
    }
    if shapes.is_empty() {
        return (shapes, ((0.0, 0.0), (0.0, 0.0)));
    }
    (shapes, (lo, hi))
}

impl Signex {
    /// Whether Paste Special opens the Smart Paste dialog: for a person in a
    /// window yes; for an agent (even in a window) or a headless run no, it
    /// pastes at once as before #155, since nobody would answer the dialog.
    pub(crate) fn smart_paste_opens_dialog(&self, request: &crate::app::PasteRequest) -> bool {
        !request.from_agent && self.headless_answers().is_none()
    }

    /// Open the Smart Paste dialog for the copy (#155).
    pub(crate) fn open_smart_paste(&mut self) {
        if !self.has_active_schematic() {
            return;
        }
        let Some(copied) = self.interaction_state.clipboard.as_ref() else {
            crate::diagnostics::log_info(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::SMART_PASTE_NOTHING_COPIED
                )
                .into_string(),
            );
            return;
        };
        let Some(target) = self.document_state.active_path.clone() else {
            return;
        };
        let grid = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm.max(1.27));
        let (_, ((x0, _), (x1, _))) = preview_shapes(copied.payload.fragment());
        let step = ((x1 - x0) / grid).ceil() * grid + grid;
        let step = if step.is_finite() && step > 0.0 {
            step
        } else {
            DEFAULT_PASTE_OFFSET_MM
        };
        self.ui_state.smart_paste = Some(Box::new(crate::smart_paste::SmartPasteState::new(
            target, step,
        )));
        self.refresh_smart_paste();
    }

    /// The dialog's derived text and drawing: the clipboard summary, the
    /// copied nets, what Next free calls each copy, the shapes.
    fn refresh_smart_paste(&mut self) {
        let Some(state) = self.ui_state.smart_paste.as_deref() else {
            return;
        };
        let Some(copied) = self.interaction_state.clipboard.as_ref() else {
            return;
        };
        let source = copied.payload.fragment().clone();
        let mut input = state.input().ok();
        if let Some(input) = input.as_mut() {
            // The mapping always shows the Next free numbers.
            input.rewrite = None;
        }
        let next_free_text = input.map_or_else(String::new, |input| {
            let combined = crate::smart_paste::plan::copies_payload(&copied.payload, &input);
            let numbered = self
                .paste_numbering_with(
                    &state.target,
                    crate::paste_prefs::PasteDesignators::NextFree,
                )
                .apply(&combined);
            let mapping = crate::smart_paste::plan::designator_mapping(
                &source,
                numbered.fragment(),
                input.copies,
            );
            crate::smart_paste::plan::mapping_text(&mapping, input.copies)
        });
        let summary = crate::smart_paste::ClipboardSummary {
            parts: source
                .symbols
                .iter()
                .filter(|s| !s.is_power && !s.reference.starts_with('#'))
                .map(|s| s.reference.clone())
                .collect(),
            wires: source.wires.len(),
            labels: source.labels.len(),
        };
        let copied_nets = crate::smart_paste::plan::copied_nets(&source);
        let (shapes, bounds) = preview_shapes(&source);
        if let Some(state) = self.ui_state.smart_paste.as_deref_mut() {
            state.summary = summary;
            state.copied_nets = copied_nets;
            state.next_free_text = next_free_text;
            state.shapes = shapes;
            state.bounds = bounds;
        }
    }

    pub(crate) fn handle_smart_paste_message(
        &mut self,
        msg: crate::smart_paste::SmartPasteMsg,
    ) -> Task<Message> {
        use crate::smart_paste::SmartPasteMsg as M;
        let target = self.ui_state.smart_paste.as_ref().map(|s| s.target.clone());
        let moved = target
            .as_ref()
            .is_some_and(|t| Some(t) != self.document_state.active_path.as_ref());
        // The dialog paints in the main window; asked from an undocked
        // window, its events act on that window's sheet (#1910).
        if moved
            && let Some(window) = target
                .as_deref()
                .and_then(|t| self.undocked_canvas_window(t))
        {
            return self.run_in_window_document(window, |app| app.handle_smart_paste_message(msg));
        }
        if moved {
            self.ui_state.smart_paste = None;
            crate::diagnostics::log_info(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::SMART_PASTE_SHEET_CHANGED
                )
                .into_string(),
            );
            return self.finish_update();
        }
        let Some(state) = self.ui_state.smart_paste.as_deref_mut() else {
            return self.finish_update();
        };
        match msg {
            M::ModePicked(mode) => state.mode = mode,
            M::OffsetXEdited(text) => state.offset_x = text,
            M::OffsetYEdited(text) => state.offset_y = text,
            M::CopiesEdited(text) => state.copies = text,
            M::StepXEdited(text) => state.step_x = text,
            M::StepYEdited(text) => state.step_y = text,
            M::DesignatorsPicked(choice) => state.designators = choice,
            M::NetsPicked(choice) => state.nets = choice,
            M::PatternEdited(text) => state.pattern = text,
            M::StartEdited(text) => state.start = text,
            M::FirstCopyDragged { dx, dy } => state.set_first_offset(dx, dy),
            M::Cancelled => {
                self.ui_state.smart_paste = None;
                return self.finish_update();
            }
            M::PasteConfirmed => {
                self.put_smart_paste_on_cursor();
                return self.finish_update();
            }
        }
        self.refresh_smart_paste();
        self.finish_update()
    }

    /// Paste: the copies attach to the cursor; one click places them all as
    /// ONE undo step (the paste session's click), Esc cancels. Designators
    /// follow the dialog's choice at that click.
    fn put_smart_paste_on_cursor(&mut self) {
        let Some(state) = self.ui_state.smart_paste.as_deref() else {
            return;
        };
        let input = match state.input() {
            Ok(input) => input,
            Err(_) => return,
        };
        let Some((payload, target, _origin)) = self.prepared_paste() else {
            return;
        };
        let copies = crate::smart_paste::plan::copies_payload(&payload, &input);
        self.ui_state.smart_paste = None;
        // A library read still in flight must not replace the paste on the
        // cursor when it finishes: a paste is the newest gesture.
        self.interaction_state.placement_epoch += 1;
        self.arm_placement_session(copies, &target, crate::app::state::PlacementKind::Paste);
        self.interaction_state.paste_designators_override =
            Some(designator_rule(input.designators));
        self.refresh_paste_ghost();
    }
}
