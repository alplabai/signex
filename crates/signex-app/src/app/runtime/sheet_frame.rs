//! The active sheet's frame and title block for the canvas (#1079).
//!
//! Lowered with `signex_output::template::lower_sheet_frame`, the lowering
//! the PDF export uses, and resolved with the same title-block values, so
//! the canvas shows the sheet as it prints.

use crate::canvas::SpecialStrings;
use signex_output::template::{Frame, FramePrimitive, lower_sheet_frame, standard_title_block};

use super::super::*;

impl Signex {
    /// Re-lower the active canvas's frame for its current paper, clearing the
    /// background cache only when the frame changed.
    pub(crate) fn refresh_canvas_sheet_frame(&mut self) {
        let canvas = self.interaction_state.active_canvas();
        let (w, h) = (canvas.paper_width_mm, canvas.paper_height_mm);
        let frame = self.sheet_frame_for_active(w, h);
        let special = self
            .document_state
            .active_path
            .as_deref()
            .and_then(|path| self.special_strings_for_path(path));
        let canvas = self.interaction_state.active_canvas_mut();
        if canvas.sheet_frame != frame {
            canvas.sheet_frame = frame;
            canvas.clear_bg_cache();
        }
        // Text notes draw from the content scene, so a change of what the
        // tokens resolve to redraws it.
        if canvas.special_strings != special {
            canvas.special_strings = special;
            canvas.clear_content_cache();
        }
    }

    /// Frame + title block for the active schematic on a `w` × `h` mm sheet.
    /// Empty when no schematic is active.
    pub(crate) fn sheet_frame_for_active(&self, w_mm: f32, h_mm: f32) -> Vec<FramePrimitive> {
        match self.document_state.active_path.as_deref() {
            Some(path) => self.sheet_frame_for_path(path, w_mm, h_mm),
            None => Vec::new(),
        }
    }

    /// What `${TOKEN}` special strings resolve to on the schematic at `path`;
    /// `None` when `path` has no open engine. The title block and every text
    /// item of the canvas read this, as the PDF and SVG read the same values.
    pub(crate) fn special_strings_for_path(
        &self,
        path: &std::path::Path,
    ) -> Option<SpecialStrings> {
        let document_state = &self.document_state;
        let engine = document_state.engines.get(path)?;
        let project = document_state.document_project(path);
        let (sheet_number, sheet_count) = match project {
            Some(project) => project_sheet_position(project, path),
            None => loose_sheet_position(document_state, path),
        };
        Some(SpecialStrings {
            metadata: crate::app::handlers::menu::export::export_metadata(engine, project),
            filename: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            sheet_name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            sheet_number,
            sheet_count,
        })
    }

    /// Frame + title block for the schematic at `path` on a `w` × `h` mm
    /// sheet. Empty when `path` has no open engine.
    pub(crate) fn sheet_frame_for_path(
        &self,
        path: &std::path::Path,
        w_mm: f32,
        h_mm: f32,
    ) -> Vec<FramePrimitive> {
        let (Some(engine), Some(special)) = (
            self.document_state.engines.get(path),
            self.special_strings_for_path(path),
        ) else {
            return Vec::new();
        };
        let settings = &engine.document().frame;
        lower_sheet_frame(
            f64::from(w_mm),
            f64::from(h_mm),
            &Frame::default().with_sheet_settings(settings),
            settings
                .show_title_block
                .then(standard_title_block)
                .as_ref(),
            |text| special.resolve(text),
        )
    }
}

/// `(number, count)` of the sheet at `path` in its project's sheet list,
/// numbered as the PDF export numbers its pages. `None` when the sheet is
/// not in that list (a hierarchical child, say): the export has no page
/// number for it either, and a guess would be another sheet's number.
fn project_sheet_position(
    project: &crate::app::state::LoadedProject,
    path: &std::path::Path,
) -> (Option<usize>, usize) {
    let fold = crate::app::state::project_case::fold_of(Some(project));
    let key = crate::app::state::scope::path_key_with(path, fold);
    let dir = project.path.parent().unwrap_or(std::path::Path::new(""));
    let sheets = &project.data.sheets;
    let number = sheets
        .iter()
        .position(|s| crate::app::state::scope::path_key_with(&dir.join(&s.filename), fold) == key)
        .map(|i| i + 1);
    (number, sheets.len().max(1))
}

/// `(number, count)` of a loose sheet among the open loose sheets, in the
/// order the PDF export prints them: the active one first, the rest by path.
fn loose_sheet_position(
    document_state: &crate::app::state::DocumentState,
    path: &std::path::Path,
) -> (Option<usize>, usize) {
    let mut paths = document_state.unowned_engine_paths();
    let active = document_state.active_path.as_deref();
    paths.sort_by_key(|p| Some(p.as_path()) != active);
    let number = paths.iter().position(|p| p == path).map(|i| i + 1);
    (number, paths.len().max(1))
}
