//! The active sheet's frame and title block for the canvas (#1079).
//!
//! Lowered with `signex_output::template::lower_sheet_frame`, the lowering
//! the PDF export uses, and resolved with the same title-block values, so
//! the canvas shows the sheet as it prints.

use signex_output::template::{Frame, FramePrimitive, lower_sheet_frame, standard_title_block};
use signex_output::{PdfOptions, SubstitutionContext};

use super::super::*;

impl Signex {
    /// Re-lower the active canvas's frame for its current paper, clearing the
    /// background cache only when the frame changed.
    pub(crate) fn refresh_canvas_sheet_frame(&mut self) {
        let canvas = self.interaction_state.active_canvas();
        let (w, h) = (canvas.paper_width_mm, canvas.paper_height_mm);
        let frame = self.sheet_frame_for_active(w, h);
        let canvas = self.interaction_state.active_canvas_mut();
        if canvas.sheet_frame != frame {
            canvas.sheet_frame = frame;
            canvas.clear_bg_cache();
        }
    }

    /// Frame + title block for the active schematic on a `w` × `h` mm sheet.
    /// Empty when no schematic is active.
    pub(crate) fn sheet_frame_for_active(&self, w_mm: f32, h_mm: f32) -> Vec<FramePrimitive> {
        let document_state = &self.document_state;
        let Some(path) = document_state.active_path.as_deref() else {
            return Vec::new();
        };
        let Some(engine) = document_state.engines.get(path) else {
            return Vec::new();
        };
        let project = document_state.document_project(path);
        let metadata = crate::app::handlers::menu::export::export_metadata(engine, project);
        let (sheet_number, sheet_count) = sheet_position(project, path);
        let defaults = PdfOptions::default();
        let sub_ctx = SubstitutionContext {
            metadata: &metadata,
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
            signex_version: env!("CARGO_PKG_VERSION"),
            variant: defaults.variant,
            physical_structure: defaults.use_physical_structure,
            physical_sheet_number: defaults.physical_sheet_number,
            physical_document_number: defaults.physical_document_number,
        };
        lower_sheet_frame(
            f64::from(w_mm),
            f64::from(h_mm),
            &Frame::default(),
            Some(&standard_title_block()),
            |text| signex_output::resolve(text, &sub_ctx),
        )
    }
}

/// `(number, count)` of the sheet at `path` in its project's sheet list,
/// numbered as the PDF export numbers its pages. A loose sheet is 1 of 1.
fn sheet_position(
    project: Option<&crate::app::state::LoadedProject>,
    path: &std::path::Path,
) -> (usize, usize) {
    let Some(project) = project else {
        return (1, 1);
    };
    let fold = crate::app::state::project_case::fold_of(Some(project));
    let key = crate::app::state::scope::path_key_with(path, fold);
    let dir = project.path.parent().unwrap_or(std::path::Path::new(""));
    let sheets = &project.data.sheets;
    let number = sheets
        .iter()
        .position(|s| crate::app::state::scope::path_key_with(&dir.join(&s.filename), fold) == key)
        .map_or(1, |i| i + 1);
    (number, sheets.len().max(1))
}
