//! Footprint-editor sketch-entity interaction handlers — the methods
//! behind the `FpEditor*` dock-panel messages that jump between Pads
//! and Sketch modes, select / hover sketch entities from the
//! Properties "Conflicts" list, and forward parametric-handle /
//! parameter-row / corner-radius edits to the sketch dispatcher on
//! the active `.snxfpt` editor. The dispatcher in `mod.rs` routes
//! these panel messages here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use iced::Task;

use super::super::*;

impl Signex {
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_edit_pad_in_sketch(&mut self, pad_idx: &usize) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let entity_id = editor
                .state
                .pads
                .get(*pad_idx)
                .and_then(|p| p.sketch_entity_id);
            editor.state.mode = crate::library::editor::footprint::state::EditorMode::Sketch;
            editor.state.selected_pad = None;
            editor.state.selected_sketch = entity_id;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_edit_pad_shape_param(
        &mut self,
        pad_idx: &usize,
        key: &str,
        value: &str,
    ) -> Task<Message> {
        let mut follow = Task::none();
        // v0.24 Phase 3 (Track A2) — Properties-panel
        // parametric-handle edit. Resolve the bound sketch
        // parameter via `pad.shape_params[key]`, then forward
        // to `FootprintSketchEditParameter` which upserts the
        // expression and triggers a solve+rebake. Undo
        // snapshot is captured at the dispatcher level via
        // `mutates_footprint_state` (defaults to true for any
        // unrecognised FootprintEditorMsg variant — verified
        // for `FootprintSketchEditParameter` already).
        let parameter_name = self.active_footprint_editor_mut().and_then(|editor| {
            editor
                .state
                .pads
                .get(*pad_idx)
                .and_then(|pad| pad.shape_params.get(key).cloned())
        });
        if let Some(name) = parameter_name {
            if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
                if let Some(path) = active_tab.kind.as_footprint_editor() {
                    let path = path.clone();
                    follow = self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::SketchEditParameter {
                                    name,
                                    expr: value.to_string(),
                                },
                            ),
                        },
                    ));
                    self.refresh_panel_ctx();
                }
            }
        } else {
            tracing::warn!(
                target: "signex::v024",
                "FpEditorEditPadShapeParam: pad {pad_idx} has no shape_params[{key}] \
                 binding; ignoring edit"
            );
        }
        follow
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_unlink_corner_radius(
        &mut self,
        arc_entity_id: &signex_sketch::id::SketchEntityId,
    ) -> Task<Message> {
        let mut follow = Task::none();
        // v0.24 Phase 3 (Track A3) — forward to the
        // `FootprintEditorMsg::SketchUnlinkCornerRadius`.
        // The dispatcher walks pads for the matching arc,
        // mints the per-corner parameter, and triggers a
        // solve+rebake. Undo snapshot captured at dispatcher
        // level via mutates_footprint_state.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            if let Some(path) = active_tab.kind.as_footprint_editor() {
                let path = path.clone();
                follow = self.update(Message::Library(
                    crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                        path,
                        msg: crate::library::messages::PrimitiveEdit::Footprint(
                            crate::library::messages::FootprintEditorMsg::SketchUnlinkCornerRadius {
                                arc_entity_id: *arc_entity_id,
                            },
                        ),
                    },
                ));
                self.refresh_panel_ctx();
            }
        }
        follow
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_edit_sketch_pad_in_pads(
        &mut self,
        id: &signex_sketch::id::SketchEntityId,
    ) -> bool {
        // v0.22 Phase D6 — mirror of FpEditorEditPadInSketch:
        // resolve the EditorPad whose `sketch_entity_id` ==
        // `id`, switch to Pads mode, and select that pad.
        if let Some(editor) = self.active_footprint_editor_mut() {
            let pad_idx = editor
                .state
                .pads
                .iter()
                .position(|p| p.sketch_entity_id == Some(*id));
            editor.state.mode = crate::library::editor::footprint::state::EditorMode::Normal;
            editor.state.selected_sketch = None;
            editor.state.selected_sketch_secondary = None;
            editor.state.selected_pad = pad_idx;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_select_sketch_entity(
        &mut self,
        id: &signex_sketch::id::SketchEntityId,
    ) -> bool {
        // v0.22 Phase E3+E4 — Properties-panel "Conflicts"
        // row click → set the sketch entity as the primary
        // selection so the canvas re-renders with that
        // entity's constraint icons highlighted in red.
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.selected_sketch = Some(*id);
            editor.state.selected_sketch_secondary = None;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_hover_over_constraint(
        &mut self,
        constraint: &Option<signex_sketch::id::ConstraintId>,
    ) -> bool {
        // v0.23 — per-row precision. `Some(id)` isolates a
        // single constraint at full red while every other
        // glyph (including other over-constraints) dims.
        // `None` clears back to the default rendering.
        if let Some(editor) = self.active_footprint_editor_mut() {
            if editor.state.conflicts_row_hovered != *constraint {
                editor.state.conflicts_row_hovered = *constraint;
                editor.canvas_cache.clear();
            }
        }
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_edit_parameter(
        &mut self,
        name: &str,
        expr: &str,
    ) -> Task<Message> {
        let mut follow = Task::none();
        // v0.16.2 — Properties-panel parameter row edit.
        // Forwards to `FootprintSketchEditParameter` which
        // upserts the parameter and triggers a solve+bake.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            if let Some(path) = active_tab.kind.as_footprint_editor() {
                let path = path.clone();
                follow = self.update(Message::Library(
                    crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                        path,
                        msg: crate::library::messages::PrimitiveEdit::Footprint(
                            crate::library::messages::FootprintEditorMsg::SketchEditParameter {
                                name: name.to_string(),
                                expr: expr.to_string(),
                            },
                        ),
                    },
                ));
                self.refresh_panel_ctx();
            }
        }
        follow
    }
}
