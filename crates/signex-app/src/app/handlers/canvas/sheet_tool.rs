//! Place Sheet tool: two clicks (opposite corners) place a hierarchical
//! sheet symbol. The rules (minimum size, unique default name) live in
//! `signex_engine`; this only collects the corners and dispatches
//! `Command::PlaceChildSheet` through the normal mutation path.

use signex_types::schematic::Point;

use super::super::super::*;

impl Signex {
    /// Handle one canvas click while `Tool::Sheet` is active.
    ///
    /// First click arms the ghost rectangle at the snapped corner. The
    /// second commits, or drops the gesture with a warning when the drag is
    /// under the minimum size. No file is created on disk.
    pub(super) fn place_sheet_click(&mut self, world_x: f64, world_y: f64) {
        let corner = self.snapped_world_point(world_x, world_y);
        let Some(anchor) = self.interaction_state.shape_anchor.take() else {
            self.interaction_state.shape_anchor = Some(corner);
            self.interaction_state.active_canvas_mut().shape_anchor =
                Some((corner, crate::canvas::ShapePreviewKind::Rect));
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
            return;
        };
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.shape_anchor = None;
        canvas.clear_overlay_cache();

        let stem = self.unused_child_sheet_stem();
        match signex_engine::child_sheet_from_corners(anchor, corner, &stem) {
            Some(child_sheet) => {
                self.apply_engine_command(
                    signex_engine::Command::PlaceChildSheet { child_sheet },
                    false,
                    true,
                );
            }
            None => crate::diagnostics::log_warning(format!(
                "Sheet not placed: each edge must be at least {} mm",
                signex_engine::MIN_CHILD_SHEET_EDGE_MM
            )),
        }
    }

    fn snapped_world_point(&self, x: f64, y: f64) -> Point {
        let grid = self.ui_state.grid_size_mm as f64;
        if self.ui_state.snap_enabled && grid > 0.0 {
            Point::new((x / grid).round() * grid, (y / grid).round() * grid)
        } else {
            Point::new(x, y)
        }
    }

    /// `sheetN` not used as a name or file by the active sheet's children or
    /// by any sheet listed in the owning project.
    fn unused_child_sheet_stem(&self) -> String {
        let mut taken: Vec<&str> = Vec::new();
        if let Some(engine) = self.document_state.active_engine() {
            for child in &engine.document().child_sheets {
                taken.push(&child.name);
                taken.push(&child.filename);
            }
        }
        if let Some(project) = self.document_state.active_document_project() {
            for sheet in &project.data.sheets {
                taken.push(&sheet.filename);
            }
        }
        signex_engine::unused_sheet_stem(taken)
    }
}
