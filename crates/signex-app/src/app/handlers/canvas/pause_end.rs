//! A paused placement ends when its sheet is read again (mockup row 157): the
//! clicked points go, the canvas unfreezes and the tool stays armed. What was
//! typed for the next placement stays.

use std::path::{Path, PathBuf};

use super::super::super::*;
use crate::app::state::{SheetPlacementPause, WindowKind};

impl Signex {
    /// The window of a sheet pause and the sheet it was taken on. Main's sheet
    /// is out of reach while a window acts.
    fn paused_sheet(&self) -> Option<(Option<iced::window::Id>, PathBuf)> {
        let state = &self.interaction_state;
        let Some(SheetPlacementPause::Sheet { window, .. }) = &state.placement_pause else {
            return None;
        };
        let path = match window {
            Some(id) => match self.ui_state.windows.get(id)? {
                WindowKind::UndockedTab { path, .. } => path.clone(),
                _ => return None,
            },
            None if state.acting_window.is_none() => self.document_state.active_path.clone()?,
            None => return None,
        };
        Some((*window, path))
    }

    /// End the sheet pause when `is_changed` says its sheet was read again.
    pub(crate) fn end_pause_on_sheet(
        &mut self,
        is_changed: impl FnOnce(&Path) -> bool,
    ) -> Task<Message> {
        let Some((owner, path)) = self.paused_sheet() else {
            return Task::none();
        };
        if !is_changed(&path) {
            return Task::none();
        }
        let state = &mut self.interaction_state;
        state.placement_pause = None;
        state.annotation_tool.paused_with = None;
        let keep_wire = state.wire_drawing && state.wire_window != owner;
        // A Tab pause can sit over a first point another window clicked.
        let keep_points = state
            .dimension_points_window()
            .is_some_and(|window| window != owner);
        let acting = state.acting_window;
        let slot = if owner == acting {
            Some(&mut state.canvas)
        } else {
            owner.and_then(|id| state.canvases.get_mut(&id))
        };
        if let Some(slot) = slot {
            slot.placement_paused = false;
            slot.clear_overlay_cache();
        }
        self.clear_drawing_buffers_keeping(keep_wire, keep_points);
        self.drop_dimension_form_draft();
        self.sync_annotation_draft();
        let window_form = match owner {
            Some(id) if acting != Some(id) => self.run_in_window_document(id, |app| {
                app.drop_dimension_form_draft();
                app.sync_annotation_draft();
                Task::none()
            }),
            _ => Task::none(),
        };
        let released = crate::app::handlers::detached_modals::unfocus_text_inputs();
        Task::batch([window_form, released])
    }
}
