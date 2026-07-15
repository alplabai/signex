//! Tools ▸ Document Options modal handlers — opening the modal against
//! a library's display settings, the draft field setters, and Apply.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

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

    /// Modal — pick a new pin selection mode.
    pub(super) fn handle_document_options_set_pin_selection(
        &mut self,
        mode: crate::library::state::PinSelectionMode,
    ) -> Task<Message> {
        if let Some(s) = self.library.document_options.as_mut() {
            s.draft.pin_selection = mode;
        }
        Task::none()
    }

    /// Modal — toggle the visible-grid checkbox.
    pub(super) fn handle_document_options_toggle_grid(&mut self) -> Task<Message> {
        if let Some(s) = self.library.document_options.as_mut() {
            s.draft.grid_visible = !s.draft.grid_visible;
        }
        Task::none()
    }

    /// Modal — cycle the visible grid spacing.
    pub(super) fn handle_document_options_cycle_grid_size(&mut self) -> Task<Message> {
        if let Some(s) = self.library.document_options.as_mut() {
            let sizes = crate::canvas::grid::GRID_SIZES_MM;
            let i = sizes
                .iter()
                .position(|sz| (sz - s.draft.grid_size_mm).abs() < f32::EPSILON)
                .unwrap_or(2);
            s.draft.grid_size_mm = sizes[(i + 1) % sizes.len()];
        }
        Task::none()
    }

    /// Modal — cycle the coordinate display unit.
    pub(super) fn handle_document_options_cycle_unit(&mut self) -> Task<Message> {
        use signex_types::coord::Unit;
        if let Some(s) = self.library.document_options.as_mut() {
            s.draft.unit = match s.draft.unit {
                Unit::Mm => Unit::Mil,
                Unit::Mil => Unit::Inch,
                Unit::Inch => Unit::Micrometer,
                Unit::Micrometer => Unit::Mm,
            };
        }
        Task::none()
    }

    /// Modal — apply the draft to the library and close.
    pub(super) fn handle_document_options_apply(&mut self) -> Task<Message> {
        if let Some(s) = self.library.document_options.take()
            && let Some(lib) = self.library.containing_library_mut(&s.library_path)
        {
            lib.display = s.draft;
        }
        // Clear every primitive editor's canvas cache so the
        // new sheet color / grid paints immediately. Cheap.
        for editor in self.document_state.symbol_editors.values_mut() {
            editor.canvas_cache.clear();
        }
        for editor in self.document_state.footprint_editors.values_mut() {
            editor.canvas_cache.clear();
        }
        Task::none()
    }
}
