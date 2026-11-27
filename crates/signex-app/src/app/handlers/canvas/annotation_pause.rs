//! A paused dimension or Bezier belongs to its window (mockup row 157): only
//! that window's Esc acts on it, other canvases ignore clicks, other
//! Properties forms are read-only, a detached Properties follows it, and
//! picking another tool ends it.

use super::super::super::*;
use crate::app::state::{InteractionState, SheetPlacementPause};
use crate::panels::{DimensionToolForm, PanelContext, PanelKind};

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

    /// No placement tool is armed, so a stored pre-placement form waits for
    /// the next one instead of taking Properties.
    pub(crate) fn is_placement_form_idle(&self) -> bool {
        self.current_tool == Tool::Select && self.placement.is_none()
    }
}

/// What every Properties context is brought in line with.
struct ToolFormSync {
    /// The window of a paused dimension or Bezier.
    pause_owner: Option<Option<iced::window::Id>>,
    is_dimension: bool,
    /// No placement tool is armed: a stored form waits, unshown.
    is_form_idle: bool,
    /// The holder of an unpaused dimension, and the form for everyone else.
    held: Option<(Option<iced::window::Id>, Box<DimensionToolForm>)>,
}

/// One Properties context's view of the tool; `this` is its window.
fn sync_form(ctx: &mut PanelContext, this: Option<iced::window::Id>, sync: &ToolFormSync) {
    let annotation = &mut ctx.annotation;
    let owner = sync.pause_owner;
    annotation.is_tool_paused_elsewhere = owner.is_some_and(|owner| owner != this);
    annotation.is_placement_form_idle = sync.is_form_idle;
    if !sync.is_dimension {
        annotation.dimension_tool = None;
        annotation.draft.take_if(|(id, _, _)| id.is_nil());
        return;
    }
    if let Some((_, form)) = sync.held.as_ref().filter(|(holder, _)| *holder != this)
        && annotation.dimension_tool.is_some()
    {
        annotation.dimension_tool = Some(form.clone());
        annotation.draft.take_if(|(id, _, _)| id.is_nil());
    }
    if let Some(form) = annotation.dimension_tool.as_mut() {
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
        let sync = ToolFormSync {
            pause_owner: state.paused_annotation_window(),
            is_dimension: matches!(
                state.current_tool,
                Tool::LinearDimension | Tool::AlignedDimension
            ),
            is_form_idle: state.is_placement_form_idle(),
            held: self.form_for_other_windows(),
        };
        let document = &mut self.document_view;
        sync_form(&mut document.panel_ctx, None, &sync);
        for (window, ctx) in &mut document.window_panel_ctx {
            sync_form(ctx, Some(*window), &sync);
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
