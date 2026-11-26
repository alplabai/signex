//! Tools ▸ Document Options modal handlers — opening the modal against
//! a library's display settings, the draft field setters, and Apply.

use super::*;

impl Signex {
    /// Tools menu fired Document Options for the library at
    /// `library_path` — opens the modal pre-filled with its display
    /// settings.
    pub(super) fn handle_open_document_options(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(lib) = self.library.library_at(&library_path) {
            self.library.document_options = Some(DocumentOptionsModalState {
                library_path: lib.root.clone(),
                library_name: lib.display_name.clone(),
                draft: lib.display,
            });
        }
        Task::none()
    }

    /// Modal — pick a new sheet color preset.
    pub(super) fn handle_document_options_set_sheet_color(
        &mut self,
        c: crate::panels::SheetColor,
    ) -> Task<Message> {
        if let Some(s) = self.library.document_options.as_mut() {
            s.draft.sheet_color = c;
        }
        Task::none()
    }

    /// Modal — toggle the visible-grid switch.
    pub(super) fn handle_document_options_toggle_grid(&mut self) -> Task<Message> {
        if let Some(s) = self.library.document_options.as_mut() {
            s.draft.grid_visible = !s.draft.grid_visible;
        }
        Task::none()
    }

    /// Modal — a grid spacing was picked. The list offers the presets
    /// plus the library's current spacing, so any positive finite pitch
    /// is taken as is.
    pub(super) fn handle_document_options_grid_spacing_picked(&mut self, mm: f32) -> Task<Message> {
        if mm.is_finite()
            && mm > 0.0
            && let Some(s) = self.library.document_options.as_mut()
        {
            s.draft.grid_size_mm = mm;
        }
        Task::none()
    }

    /// Modal — a coordinate display unit was picked.
    pub(super) fn handle_document_options_unit_picked(
        &mut self,
        unit: signex_types::coord::Unit,
    ) -> Task<Message> {
        if let Some(s) = self.library.document_options.as_mut() {
            s.draft.unit = unit;
        }
        Task::none()
    }

    /// Modal — apply the draft to the library and close.
    pub(super) fn handle_document_options_apply(&mut self) -> Task<Message> {
        if let Some(s) = self.library.document_options.take() {
            let fold = self.identity_fold_for(&s.library_path);
            if let Some(lib) = self
                .library
                .containing_library_mut_with(&s.library_path, fold)
            {
                lib.display = s.draft;
            }
        }
        // Clear every primitive editor's canvas cache so the
        // new sheet color / grid paints immediately. Cheap.
        for editor in self.document_state.symbol_editors.values_mut() {
            editor.gui.canvas_cache.clear();
        }
        for editor in self.document_state.footprint_editors.values_mut() {
            editor.canvas_cache.clear();
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{Message, Signex};
    use crate::library::messages::LibraryMessage;
    use signex_types::coord::Unit;

    /// A fresh app with one mounted library at a known grid (2.54 mm, mm,
    /// visible) and the Document Options modal open on it.
    fn app_with_open_modal() -> (Signex, tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("options.snxlib");
        crate::library::commands::create_library_file(&root, &Default::default()).expect("library");
        let (mut app, _boot) = Signex::new();
        app.library.open_library(root.clone()).expect("mounts");
        let display = &mut app.library.library_at_mut(&root).expect("library").display;
        display.grid_size_mm = 2.54;
        display.unit = Unit::Mm;
        display.grid_visible = true;
        send(
            &mut app,
            LibraryMessage::OpenDocumentOptions {
                library_path: root.clone(),
            },
        );
        assert!(app.library.document_options.is_some(), "the modal opens");
        (app, dir, root)
    }

    fn send(app: &mut Signex, msg: LibraryMessage) {
        let _ = app.update(Message::Library(msg));
    }

    /// A pick lands in the draft only; the library keeps its settings until
    /// Apply.
    #[test]
    fn a_picked_spacing_and_unit_land_in_the_draft() {
        let (mut app, _dir, root) = app_with_open_modal();

        send(
            &mut app,
            LibraryMessage::DocumentOptionsGridSpacingPicked(0.635),
        );
        send(
            &mut app,
            LibraryMessage::DocumentOptionsUnitPicked(Unit::Inch),
        );

        let draft = app.library.document_options.as_ref().expect("open").draft;
        assert_eq!(draft.grid_size_mm, 0.635);
        assert_eq!(draft.unit, Unit::Inch);
        let display = app.library.library_at(&root).expect("library").display;
        assert_eq!(display.unit, Unit::Mm, "not applied yet");
        assert_eq!(display.grid_size_mm, 2.54, "not applied yet");
    }

    /// Every unit the model knows is picked directly, the micrometre
    /// included (the old click-to-cycle reached it last).
    #[test]
    fn each_unit_is_picked_directly() {
        let (mut app, _dir, _root) = app_with_open_modal();
        for unit in [Unit::Micrometer, Unit::Mil, Unit::Mm, Unit::Inch] {
            send(&mut app, LibraryMessage::DocumentOptionsUnitPicked(unit));
            let draft = app.library.document_options.as_ref().expect("open").draft;
            assert_eq!(draft.unit, unit);
        }
    }

    /// A spacing that is no length is refused; the draft keeps its pitch.
    #[test]
    fn a_spacing_that_is_no_length_is_ignored() {
        let (mut app, _dir, _root) = app_with_open_modal();
        for bad in [0.0, -1.27, f32::NAN, f32::INFINITY] {
            send(
                &mut app,
                LibraryMessage::DocumentOptionsGridSpacingPicked(bad),
            );
        }
        let draft = app.library.document_options.as_ref().expect("open").draft;
        assert_eq!(draft.grid_size_mm, 2.54);
    }

    /// Apply writes the picked spacing, unit and grid visibility to the
    /// library's display settings and closes the modal.
    #[test]
    fn apply_writes_the_picks_to_the_library() {
        let (mut app, _dir, root) = app_with_open_modal();

        send(
            &mut app,
            LibraryMessage::DocumentOptionsGridSpacingPicked(5.08),
        );
        send(
            &mut app,
            LibraryMessage::DocumentOptionsUnitPicked(Unit::Mil),
        );
        send(&mut app, LibraryMessage::DocumentOptionsToggleGrid);
        send(&mut app, LibraryMessage::DocumentOptionsApply);

        assert!(app.library.document_options.is_none(), "Apply closes it");
        let display = app.library.library_at(&root).expect("library").display;
        assert_eq!(display.grid_size_mm, 5.08);
        assert_eq!(display.unit, Unit::Mil);
        assert!(!display.grid_visible);
    }

    /// Cancel drops the picks.
    #[test]
    fn cancel_drops_the_picks() {
        let (mut app, _dir, root) = app_with_open_modal();

        send(
            &mut app,
            LibraryMessage::DocumentOptionsGridSpacingPicked(5.08),
        );
        send(
            &mut app,
            LibraryMessage::DocumentOptionsUnitPicked(Unit::Mil),
        );
        send(&mut app, LibraryMessage::DocumentOptionsCancel);

        assert!(app.library.document_options.is_none());
        let display = app.library.library_at(&root).expect("library").display;
        assert_eq!(display.grid_size_mm, 2.54);
        assert_eq!(display.unit, Unit::Mm);
    }
}
