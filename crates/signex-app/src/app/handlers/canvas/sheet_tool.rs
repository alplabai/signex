//! Place Sheet tool: two clicks (opposite corners) place a hierarchical
//! sheet symbol. The rules (minimum size, unique default name) live in
//! `signex_engine`; this only collects the corners and dispatches
//! `Command::PlaceChildSheet` through the normal mutation path.

use std::path::PathBuf;

use iced::Task;
use signex_types::schematic::Point;

use super::super::super::*;

/// The Place Sheet commit waiting for the listing of the `.snxsch` files in
/// the sheet's and the project's folder (#859). At most one: a second commit
/// while it waits is refused, since both would pick the same default name.
#[derive(Debug, Default)]
pub struct SheetPlaceJobs {
    ticket: u64,
    waiting: Option<WaitingPlace>,
}

#[derive(Debug)]
struct WaitingPlace {
    ticket: u64,
    anchor: Point,
    corner: Point,
    /// The sheet the corners were drawn on, and that engine instance: the
    /// placement lands only on that very sheet.
    sheet: PathBuf,
    engine: u64,
}

impl Signex {
    /// Handle one canvas click while `Tool::Sheet` is active.
    ///
    /// The caller has already snapped the point. First click arms the ghost
    /// rectangle at that corner. The
    /// second commits, or drops the gesture with a warning when the drag is
    /// under the minimum size. No file is created on disk.
    pub(super) fn place_sheet_click(&mut self, world_x: f64, world_y: f64) -> Task<Message> {
        let corner = Point::new(world_x, world_y);
        let Some(anchor) = self.interaction_state.shape_anchor.take() else {
            self.interaction_state.shape_anchor = Some(corner);
            self.interaction_state.active_canvas_mut().shape_anchor =
                Some((corner, crate::canvas::ShapePreviewKind::Rect));
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
            return Task::none();
        };
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.shape_anchor = None;
        canvas.clear_overlay_cache();
        self.begin_sheet_place(anchor, corner)
    }

    /// Ask a worker which `.snxsch` files already sit beside the sheet; the
    /// completion places it. Nothing is read in `update`.
    fn begin_sheet_place(&mut self, anchor: Point, corner: Point) -> Task<Message> {
        let (Some(sheet), Some(engine)) = (
            self.active_tab_path(),
            self.document_state.active_engine().map(|e| e.instance_id()),
        ) else {
            crate::diagnostics::log_warning("Sheet not placed: no sheet is open");
            return Task::none();
        };
        if self.ui_state.sheet_place.waiting.is_some() {
            crate::diagnostics::log_warning(
                "Sheet not placed: the previous sheet is still being placed",
            );
            return Task::none();
        }
        let runs = &mut self.ui_state.sheet_place;
        runs.ticket += 1;
        let ticket = runs.ticket;
        runs.waiting = Some(WaitingPlace {
            ticket,
            anchor,
            corner,
            sheet,
            engine,
        });
        let dirs = self.sheet_scan_dirs();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || list_sheet_files(&dirs)),
            move |result| Message::Project(ProjectMsg::SheetFilesListed { ticket, result }),
        )
    }

    /// The listing arrived: place the sheet unless the user moved on.
    pub(crate) fn handle_sheet_files_listed(
        &mut self,
        ticket: u64,
        result: Result<Vec<String>, String>,
    ) -> Task<Message> {
        let waiting = &mut self.ui_state.sheet_place.waiting;
        let Some(place) = waiting.take_if(|w| w.ticket == ticket) else {
            return Task::none();
        };
        let on_disk = match result {
            Ok(names) => names,
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Sheet not placed: the folder could not be listed ({why})"
                ));
                return Task::none();
            }
        };
        let still_there = self.active_tab_path().as_deref() == Some(place.sheet.as_path())
            && self
                .document_state
                .active_engine()
                .is_some_and(|e| e.instance_id() == place.engine);
        if !still_there {
            crate::diagnostics::log_warning(
                "Sheet not placed: the sheet it was drawn on is no longer the active one",
            );
            return Task::none();
        }
        self.place_sheet(place.anchor, place.corner, on_disk);
        Task::none()
    }

    fn place_sheet(&mut self, anchor: Point, corner: Point, on_disk: Vec<String>) {
        let stem = self.unused_child_sheet_stem(on_disk);
        match signex_engine::child_sheet_from_corners(anchor, corner, &stem) {
            Some(child_sheet) => {
                let symbol = child_sheet.uuid;
                let placed = self.apply_engine_command(
                    signex_engine::Command::PlaceChildSheet { child_sheet },
                    false,
                    true,
                );
                if placed {
                    // Ask where the new sheet's file goes (Later keeps the
                    // symbol alone); the tool stays armed for the next one.
                    self.open_create_child_sheet_dialog(symbol, false);
                } else {
                    crate::diagnostics::log_warning("Sheet not placed: the edit was refused");
                }
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
    fn unused_child_sheet_stem(&self, on_disk: Vec<String>) -> String {
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

    /// The folders whose `.snxsch` files a new sheet name must not collide
    /// with: the active sheet's and the project's. Memory only.
    fn sheet_scan_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = Vec::new();
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
        dirs
    }
}

/// File names of the `.snxsch` files in `dirs`. Blocking: run it off the UI
/// thread. An unreadable folder contributes nothing.
fn list_sheet_files(dirs: &[PathBuf]) -> Vec<String> {
    dirs.iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.to_ascii_lowercase().ends_with(".snxsch"))
        .collect()
}
