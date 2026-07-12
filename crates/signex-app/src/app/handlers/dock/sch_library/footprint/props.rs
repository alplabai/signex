//! Footprint-editor component-level property handlers — the methods
//! behind the `FpEditor*` dock-panel messages that edit the active
//! `.snxfpt` footprint's own metadata (name, description, default
//! designator, component type, height, sketch role, auto-fit
//! courtyard, selection filter). The dispatcher in `mod.rs` routes
//! these panel messages here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use iced::Task;

use super::super::*;

impl Signex {
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_auto_fit_courtyard(&mut self) -> Task<Message> {
        let mut follow = Task::none();
        // v0.14.2 — resolve the active footprint editor's
        // path and route through the existing
        // `FootprintToggleAutoFit` dispatch so the toggle
        // shares its dirty / panel-refresh behaviour with
        // the active-bar button.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            if let Some(path) = active_tab.kind.as_footprint_editor() {
                let path = path.clone();
                follow = self.update(Message::Library(
                    crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                        path,
                        msg: crate::library::messages::PrimitiveEdit::Footprint(
                            crate::library::messages::FootprintEditorMsg::ToggleAutoFit,
                        ),
                    },
                ));
                // v0.16.x — rebuild the panel context so the
                // pill's pressed-state style reflects the new
                // `auto_fit_courtyard` bool. Without this the
                // button click looked like a no-op because
                // `PanelContext.footprint_editor.auto_fit_courtyard`
                // was stale until the next unrelated panel
                // refresh.
                self.refresh_panel_ctx();
            }
        }
        follow
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_role(
        &mut self,
        id: &signex_sketch::id::SketchEntityId,
        role: &crate::library::messages::RoleTag,
    ) -> Task<Message> {
        let mut follow = Task::none();
        // v0.16.2 — Properties-panel Role pick_list. Resolve
        // the active footprint editor tab and forward through
        // the standard PrimitiveEditorEvent path so the role
        // mutation goes through `apply_sketch_role_with_warnings`
        // (clears all attrs, sets matching one, runs solve+bake).
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            if let Some(path) = active_tab.kind.as_footprint_editor() {
                let path = path.clone();
                follow = self.update(Message::Library(
                    crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                        path,
                        msg: crate::library::messages::PrimitiveEdit::Footprint(
                            crate::library::messages::FootprintEditorMsg::SketchSetRole {
                                id: *id,
                                role: *role,
                            },
                        ),
                    },
                ));
                self.refresh_panel_ctx();
            }
        }
        follow
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_description(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.primitive_mut().description = v.to_string();
            editor.dirty = true;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_default_designator(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.primitive_mut().default_designator = v.to_string();
            editor.dirty = true;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_component_type(
        &mut self,
        t: &signex_library::primitive::footprint::ComponentType,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.primitive_mut().component_type = *t;
            editor.dirty = true;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_height(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.primitive_mut().height_mm = fp_parse_optional_mm(v);
            editor.dirty = true;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_name(&mut self, name: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.primitive_mut().name = name.to_string();
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_selection_filter(
        &mut self,
        kind: &crate::library::editor::footprint::state::SelectionFilterKind,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.selection_filter.toggle(*kind);
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }
}
