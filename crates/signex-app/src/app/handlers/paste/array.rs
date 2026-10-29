//! Paste Array (#1264): the dialog's handler and the one-undo-step commit.

use iced::Task;
use signex_engine::Command;
use signex_types::schematic::Point;

use super::designators::PasteNumbering;
use crate::app::{Message, Signex};
use crate::paste_array::{PasteArrayInput, PasteArrayMsg, PasteArrayState};

/// Width of the copy in the default X spacing is padded by this many grid
/// pitches so neighbouring copies do not touch.
const DEFAULT_GAP_PITCHES: f64 = 1.0;
/// X spacing when the copy has no measurable width.
const FALLBACK_SPACING_MM: f64 = 5.08;

impl Signex {
    /// Open the dialog for the in-app copy. The copy is the system
    /// clipboard's already when the request came through
    /// `handle_paste_text_read`.
    pub(crate) fn open_paste_array(&mut self) {
        let Some(copied) = self.interaction_state.clipboard.as_ref() else {
            crate::diagnostics::log_info(
                "Paste Array: nothing to paste yet; copy a selection first.",
            );
            return;
        };
        if !self.has_active_schematic() {
            return;
        }
        let grid = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
        let dx_mm = copied
            .bounds
            .map(|b| ((b.max_x - b.min_x) / grid).ceil() * grid + DEFAULT_GAP_PITCHES * grid)
            .filter(|dx| dx.is_finite() && *dx > 0.0)
            .unwrap_or(FALLBACK_SPACING_MM);
        self.ui_state.paste_array = Some(PasteArrayState::new(
            self.ui_state.unit,
            dx_mm,
            0.0,
            self.ui_state.increment_step,
            self.ui_state.paste_designators,
            copied.payload.item_count(),
            self.document_state.active_path.clone(),
        ));
    }

    pub(crate) fn handle_paste_array_message(&mut self, msg: PasteArrayMsg) -> Task<Message> {
        self.close_paste_array_if_sheet_changed();
        if let Some(state) = self.ui_state.paste_array.as_mut() {
            match msg {
                PasteArrayMsg::CountChanged(text) => state.count = text,
                PasteArrayMsg::XSpacingChanged(text) => state.x_spacing = text,
                PasteArrayMsg::YSpacingChanged(text) => state.y_spacing = text,
                PasteArrayMsg::StepChanged(text) => state.step = text,
                PasteArrayMsg::DesignatorsChosen(mode) => state.designators = mode,
                PasteArrayMsg::ApplyPressed => self.apply_paste_array(),
                PasteArrayMsg::Closed => self.ui_state.paste_array = None,
            }
        }
        self.finish_update()
    }

    /// Close the dialog when the active sheet is not the one it was opened
    /// on, so an array never lands on a different sheet.
    pub(crate) fn close_paste_array_if_sheet_changed(&mut self) {
        let moved = self
            .ui_state
            .paste_array
            .as_ref()
            .is_some_and(|s| s.origin != self.document_state.active_path);
        if moved {
            self.ui_state.paste_array = None;
            crate::diagnostics::log_info("Paste Array closed: the active sheet changed");
        }
    }

    /// OK: place every copy as ONE engine batch, so one undo takes the whole
    /// array back. Copy `k` (1 to count) sits `k` spacings from the original
    /// and its label numbers have moved `k` steps; designators follow the
    /// "Designators on paste" preference, numbered in turn across the copies.
    fn apply_paste_array(&mut self) {
        let Some(state) = self.ui_state.paste_array.as_ref() else {
            return;
        };
        let input = match state.input() {
            Ok(input) => input,
            Err(reason) => {
                crate::diagnostics::log_warning(format!("Paste Array: {reason}"));
                return;
            }
        };
        let Some((payload, target, origin)) = self.prepared_paste() else {
            return;
        };
        let (commands, numbering) = match self.array_commands(&payload, &target, origin, input) {
            Ok(built) => built,
            Err(error) => {
                crate::diagnostics::log_warning(format!(
                    "Paste Array failed: the copies could not be placed ({error})."
                ));
                return;
            }
        };
        if self.apply_engine_commands(commands, true, true) {
            self.remember_numbering(&target, numbering);
            self.ui_state.paste_array = None;
            crate::diagnostics::log_info(format!(
                "Paste Array: placed {} copies as one undo step.",
                input.count
            ));
        } else {
            crate::diagnostics::log_warning(
                "Paste Array: nothing was placed; see the Messages panel for details.",
            );
        }
    }

    fn array_commands(
        &self,
        payload: &signex_engine::PlacementPayload,
        target: &std::path::Path,
        origin: Point,
        input: PasteArrayInput,
    ) -> Result<(Vec<Command>, PasteNumbering), signex_engine::EngineError> {
        let mut numbering = self.paste_numbering_with(target, input.designators);
        // The bodies are registered once; the per-copy clones carry none.
        let mut commands = payload.register_commands();
        let bare = payload.without_bodies();
        for k in 1..=input.count {
            let steps = i64::from(k) * i64::from(input.step);
            let copy = numbering.apply(&bare.with_labels_stepped(steps));
            let at = Point::new(
                origin.x + f64::from(k) * input.dx_mm,
                origin.y + f64::from(k) * input.dy_mm,
            );
            commands.extend(copy.place_commands(at)?);
        }
        Ok((commands, numbering))
    }
}
