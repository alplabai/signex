//! A paused dimension or Bezier belongs to its window (mockup row 157): only
//! that window's Esc acts on it, other canvases ignore clicks, other
//! Properties forms are read-only, a detached Properties follows it, and
//! picking another tool ends it.

use super::super::super::*;
use crate::app::state::{InteractionState, SheetPlacementPause};
use crate::panels::{PanelContext, PanelKind};

impl InteractionState {
    /// The window of a paused dimension or Bezier: `None` when neither is
    /// paused, `Some(None)` for main.
    pub(crate) fn paused_annotation_window(&self) -> Option<Option<iced::window::Id>> {
        let is_annotation = matches!(
            self.current_tool,
            Tool::Bezier | Tool::LinearDimension | Tool::AlignedDimension
        );
        match self.placement_pause {
            Some(SheetPlacementPause::Sheet { window, .. }) if is_annotation => Some(window),
            _ => None,
        }
    }
}

/// One Properties context's view of the pause; `this` is its window.
fn sync_form(
    ctx: &mut PanelContext,
    this: Option<iced::window::Id>,
    owner: Option<Option<iced::window::Id>>,
    is_dimension: bool,
) {
    let annotation = &mut ctx.annotation;
    annotation.is_tool_paused_elsewhere = owner.is_some_and(|owner| owner != this);
    if !is_dimension {
        annotation.dimension_tool = None;
        annotation.draft.take_if(|(id, _, _)| id.is_nil());
    } else if let Some(form) = annotation.dimension_tool.as_mut() {
        form.is_paused = owner == Some(this);
    }
}

impl Signex {
    /// The window an input came from: the one acting, else `window` with
    /// main as `None`.
    fn input_window(&self, window: Option<iced::window::Id>) -> Option<iced::window::Id> {
        let main = self.ui_state.main_window_id;
        self.interaction_state
            .acting_window
            .or(window.filter(|id| main != Some(*id)))
    }

    /// Whether a paused dimension or Bezier belongs to a window other than
    /// the one this input came from.
    pub(crate) fn is_annotation_pause_of_another(&self, window: Option<iced::window::Id>) -> bool {
        self.interaction_state
            .paused_annotation_window()
            .is_some_and(|owner| owner != self.input_window(window))
    }

    /// Esc in `window` with a dimension or Bezier paused: the owner resumes,
    /// any other window leaves it alone. `None` when neither is paused.
    pub(in crate::app) fn escape_annotation_pause(
        &mut self,
        window: Option<iced::window::Id>,
    ) -> Option<Task<Message>> {
        let owner = self.interaction_state.paused_annotation_window()?;
        let typed_in = self.input_window(window);
        if owner != typed_in {
            if typed_in.is_none() && self.has_active_pcb() {
                self.clear_pcb_selection();
            }
            return Some(Task::none());
        }
        Some(match owner {
            Some(id) if self.interaction_state.acting_window != Some(id) => {
                self.run_in_window_document(id, Self::resume_placement_without_edits)
            }
            _ => self.resume_placement_without_edits(),
        })
    }

    /// Another tool was picked: the pause ends and its window unfreezes.
    /// Main's slot is out of reach while a window acts:
    /// `end_main_canvas_work` clears it.
    pub(crate) fn end_annotation_pause(&mut self) {
        let state = &mut self.interaction_state;
        let Some(owner) = state.paused_annotation_window() else {
            return;
        };
        state.placement_pause = None;
        let slot = if owner == state.acting_window {
            Some(&mut state.canvas)
        } else {
            owner.and_then(|id| state.canvases.get_mut(&id))
        };
        if let Some(slot) = slot {
            slot.placement_paused = false;
            slot.tool_preview = None;
            slot.clear_overlay_cache();
        }
    }

    /// Bring every Properties context in line with the pause. Skipped inside
    /// a window's swap, where the main slot holds that window's context.
    pub(in crate::app) fn sync_paused_tool_forms(&mut self) {
        let state = &self.interaction_state;
        if state.acting_window.is_some() {
            return;
        }
        let owner = state.paused_annotation_window();
        let is_dimension = matches!(
            state.current_tool,
            Tool::LinearDimension | Tool::AlignedDimension
        );
        let document = &mut self.document_state;
        sync_form(&mut document.panel_ctx, None, owner, is_dimension);
        for (window, ctx) in &mut document.window_panel_ctx {
            sync_form(ctx, Some(*window), owner, is_dimension);
        }
    }

    /// The undocked window whose pause a detached Properties follows.
    pub(crate) fn detached_properties_owner(&self, kind: PanelKind) -> Option<iced::window::Id> {
        (kind == PanelKind::Properties)
            .then(|| self.interaction_state.paused_annotation_window())
            .flatten()
            .flatten()
            .filter(|window| self.is_document_window(*window))
    }
}
