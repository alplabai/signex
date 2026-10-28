//! Place Sheet tool: two clicks (opposite corners) place a hierarchical
//! sheet symbol. The rules (minimum size, unique default name) live in
//! `signex_engine`; this only collects the corners and dispatches
//! `Command::PlaceChildSheet` through the normal mutation path.

use signex_types::schematic::Point;

use super::super::super::*;

impl Signex {
    /// Handle one canvas click while `Tool::Sheet` is active.
    ///
    /// The caller has already snapped the point. First click arms the ghost
    /// rectangle at that corner. The
    /// second commits, or drops the gesture with a warning when the drag is
    /// under the minimum size. No file is created on disk.
    pub(super) fn place_sheet_click(&mut self, world_x: f64, world_y: f64) {
        let corner = Point::new(world_x, world_y);
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
                let symbol = child_sheet.uuid;
                self.apply_engine_command(
                    signex_engine::Command::PlaceChildSheet { child_sheet },
                    false,
                    true,
                );
                // Ask where the new sheet's file goes (Later keeps the symbol
                // alone).
                self.open_create_child_sheet_dialog(symbol, false);
            }
            None => crate::diagnostics::log_warning(format!(
                "Sheet not placed: each edge must be at least {} mm",
                signex_engine::MIN_CHILD_SHEET_EDGE_MM
            )),
        }
    }

    /// One click while `Tool::SheetEntry` is active: put an entry on the border
    /// of the sheet symbol under the cursor, snapped to its nearest edge. The
    /// name (`P1`, `P2`, ...) and the direction (by edge) are defaults the user
    /// edits afterwards; the rules live in `signex_engine`.
    pub(super) fn place_sheet_entry_click(&mut self, world_x: f64, world_y: f64) {
        let cursor = Point::new(world_x, world_y);
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let Some(symbol) = signex_engine::sheet_symbol_at(engine.document(), cursor) else {
            crate::diagnostics::log_info(
                "Sheet entry not placed: click on or next to a sheet symbol",
            );
            return;
        };
        let pin = signex_engine::new_sheet_entry(
            symbol,
            cursor,
            &signex_engine::unused_entry_name(symbol),
            None,
        );
        let command = signex_engine::Command::PlaceSheetPin {
            sheet_id: symbol.uuid,
            pin,
        };
        self.apply_engine_command(command, false, true);
    }

    /// `sheetN` not used as a name or file by the children of any open sheet
    /// in the active document's project, by any sheet the project lists, or
    /// by a `.snxsch` already sitting in the sheet's or the project's folder
    /// (#876) — an unlisted file there would otherwise be adopted silently.
    fn unused_child_sheet_stem(&self) -> String {
        let on_disk = self.sheet_files_on_disk();
        let mut taken: Vec<&str> = on_disk.iter().map(String::as_str).collect();
        for engine in self.document_state.engines_in_active_document_project() {
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

    /// File names of the `.snxsch` files in the active sheet's folder and the
    /// project folder. Read-only; an unreadable folder contributes nothing.
    fn sheet_files_on_disk(&self) -> Vec<String> {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        if let Some(path) = self.active_tab_path()
            && let Some(parent) = path.parent()
        {
            dirs.push(parent.to_path_buf());
        }
        if let Some(project_dir) = self
            .document_state
            .active_loaded_project()
            .and_then(|p| p.path.parent())
        {
            dirs.push(project_dir.to_path_buf());
        }
        dirs.dedup();
        dirs.iter()
            .filter_map(|dir| std::fs::read_dir(dir).ok())
            .flatten()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.to_ascii_lowercase().ends_with(".snxsch"))
            .collect()
    }
}
