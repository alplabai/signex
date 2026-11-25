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
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_auto_fit_courtyard(
        &mut self,
    ) -> Task<Message> {
        let mut follow = Task::none();
        // v0.14.2 — resolve the active footprint editor's
        // path and route through the existing
        // `FootprintToggleAutoFit` dispatch so the toggle
        // shares its dirty / panel-refresh behaviour with
        // the active-bar button.
        // The footprint tab in view, or a board's sketch editor (#2234).
        if let Some(path) = self.active_footprint_editor().map(|e| e.path.clone()) {
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
        follow
    }

    /// Forward a layer-visibility message of the Layers panel to the
    /// active footprint editor and refresh the panel context so the eyes
    /// show the new state.
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_layer_msg(
        &mut self,
        msg: crate::library::messages::FootprintEditorMsg,
    ) -> Task<Message> {
        let mut follow = Task::none();
        // The footprint tab in view, or a board's sketch editor (#2234).
        if let Some(path) = self.active_footprint_editor().map(|e| e.path.clone()) {
            follow = self.update(Message::Library(
                crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                    path,
                    msg: crate::library::messages::PrimitiveEdit::Footprint(msg),
                },
            ));
            self.refresh_panel_ctx();
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
        // The footprint tab in view, or a board's sketch editor (#2234).
        if self.say_role_does_not_fit(id, *role) {
            return follow;
        }
        if let Some(path) = self.active_footprint_editor().map(|e| e.path.clone()) {
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
        follow
    }

    /// A board role picked for a shape it cannot take (#2518): Messages
    /// says why and the entity keeps its role. Whether it was refused.
    fn say_role_does_not_fit(
        &self,
        id: &signex_sketch::id::SketchEntityId,
        role: crate::library::messages::RoleTag,
    ) -> bool {
        let Some(editor) = self.active_footprint_editor() else {
            return false;
        };
        let misfit = editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|sketch| sketch.entities.iter().find(|e| e.id == *id))
            .is_some_and(|entity| !role.fits(&entity.kind));
        if misfit {
            let i18n = &self.ui_state.i18n;
            let label = crate::library::editor::footprint::sketch_mode::roles::role_label(
                role,
                editor.state.sketch_host(),
                i18n,
            );
            crate::diagnostics::log_warning(
                signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::EDITORS_SKETCH_ROLE_DOES_NOT_FIT,
                    "role" => label
                )
                .into_string(),
            );
        }
        misfit
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_description(
        &mut self,
        v: &str,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.record_edit(Some(("description", 0)), |editor| {
                editor.primitive_mut().description = v.to_string();
                editor.mark_dirty();
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_default_designator(
        &mut self,
        v: &str,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.record_edit(Some(("default_designator", 0)), |editor| {
                editor.primitive_mut().default_designator = v.to_string();
                editor.mark_dirty();
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_component_type(
        &mut self,
        t: &signex_library::primitive::footprint::ComponentType,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.record_edit(None, |editor| {
                editor.primitive_mut().component_type = *t;
                editor.mark_dirty();
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_height(
        &mut self,
        v: &str,
    ) -> bool {
        use crate::library::editor::footprint::FOOTPRINT_HEIGHT_BUFFER;
        // Empty clears the height; text that is no number is refused and
        // stays in the field (mockup row 124).
        let edit = super::pad_input::optional_mm_edit(v);
        if let Some(editor) = self.active_footprint_editor_mut() {
            match edit {
                Some(height) => {
                    editor.state.numeric_buffers.remove(FOOTPRINT_HEIGHT_BUFFER);
                    editor.record_edit(Some(("height", 0)), |editor| {
                        if editor.primitive().height_mm == height {
                            return false;
                        }
                        editor.primitive_mut().height_mm = height;
                        editor.mark_dirty();
                        true
                    });
                }
                None => {
                    editor
                        .state
                        .numeric_buffers
                        .insert(FOOTPRINT_HEIGHT_BUFFER.to_owned(), v.to_owned());
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_footprint_name(
        &mut self,
        name: &str,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.record_edit(Some(("name", 0)), |editor| {
                editor.primitive_mut().name = name.to_string();
                editor.mark_dirty();
                editor.canvas_cache.clear();
                true
            });
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

#[cfg(test)]
mod height_tests {
    use std::path::{Path, PathBuf};

    use signex_library::{Footprint, FootprintFile};

    use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};
    use crate::library::editor::footprint::FOOTPRINT_HEIGHT_BUFFER;

    fn app_on_a_footprint(path: &Path) -> Signex {
        let path: PathBuf = path.to_path_buf();
        let file = FootprintFile::from_footprint(Footprint::empty("height"));
        let (mut app, _task) = Signex::new();
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
        app.document_state.tabs.push(TabInfo {
            title: "height".into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: TabKind::FootprintEditor(path),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app
    }

    #[test]
    fn text_that_is_no_number_keeps_the_height_and_empty_clears_it() {
        let path = PathBuf::from("height.snxfpt");
        let mut app = app_on_a_footprint(&path);
        app.handle_fp_editor_set_footprint_height("0.9");
        let steps = app.document_state.footprint_editors[&path]
            .history
            .undo_len();

        app.handle_fp_editor_set_footprint_height("abc");
        let editor = &app.document_state.footprint_editors[&path];
        assert_eq!(editor.primitive().height_mm, Some(0.9));
        assert_eq!(editor.history.undo_len(), steps, "a refusal is no step");
        assert_eq!(
            editor
                .state
                .numeric_buffers
                .get(FOOTPRINT_HEIGHT_BUFFER)
                .map(String::as_str),
            Some("abc"),
            "the field keeps the text"
        );

        app.handle_fp_editor_set_footprint_height("");
        let editor = &app.document_state.footprint_editors[&path];
        assert_eq!(editor.primitive().height_mm, None);
        assert!(
            !editor
                .state
                .numeric_buffers
                .contains_key(FOOTPRINT_HEIGHT_BUFFER)
        );
    }
}
