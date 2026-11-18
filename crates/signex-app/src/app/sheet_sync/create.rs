//! Create Sheet Symbol From Sheet: pick a sheet file, read it off the UI
//! thread, and place a sheet symbol for it on the active sheet with an entry
//! for every port, as one undo step. The symbol goes to the cursor's last
//! point on this sheet, else under the sheet's content.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::ids;
use signex_types::schematic::{Point, SchematicSheet};

use super::SheetSyncMsg;
use crate::app::handlers::canvas::reference_from;
use crate::app::project_sheets::{fold_key, load_sheet_file, resolve_child_reference};
use crate::app::sheet_hierarchy::normalised_key;
use crate::app::{Message, Signex};

const LABEL: &str = "Create Sheet Symbol From Sheet";

/// Read the sheet at `path`. Blocking: run it off the UI thread.
fn read_sheet(path: &Path) -> Result<Arc<SchematicSheet>, String> {
    match load_sheet_file(path) {
        Ok(Some(sheet)) => Ok(Arc::new(sheet)),
        Ok(None) => Err(format!("there is no file at {}", path.display())),
        Err(why) => Err(why),
    }
}

impl Signex {
    /// The sheet a new sheet symbol lands on: the active schematic, when it
    /// takes an edit. Says why not in the Messages panel.
    fn create_symbol_target(&mut self) -> Option<PathBuf> {
        if !self.has_active_schematic() {
            crate::diagnostics::log_warning(format!("{LABEL} works on a schematic sheet."));
            return None;
        }
        if let Some(reason) = self.refuse_if_active_read_only() {
            crate::diagnostics::log_warning(format!("{LABEL}: {reason}"));
            return None;
        }
        self.document_state.active_path.clone()
    }

    /// Ask for the sheet file to make a sheet symbol for.
    pub(super) fn create_symbol_dialog(&mut self) -> Task<Message> {
        if self.create_symbol_target().is_none() {
            return Task::none();
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::SHEET_SYNC_PICK_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_SCHEMATIC, &["snxsch"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        Task::perform(dialog.pick_file(), |path| {
            Message::from(SheetSyncMsg::CreateSymbolPicked(path))
        })
    }

    /// Read the picked file off the UI thread, for the active sheet.
    pub(super) fn read_symbol_sheet(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let Some(target) = self.create_symbol_target() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let read = path.clone();
                let sheet = crate::app::path_op_jobs::run_blocking(move || read_sheet(&read))
                    .await
                    .and_then(std::convert::identity);
                (path, sheet)
            },
            move |(path, sheet)| {
                Message::from(SheetSyncMsg::CreateSymbolRead {
                    path,
                    target: target.clone(),
                    sheet,
                })
            },
        )
    }

    /// The file was read: place its sheet symbol, unless the user moved to
    /// another sheet meanwhile. The outcome goes to the Messages panel.
    pub(super) fn symbol_sheet_read(
        &mut self,
        path: &Path,
        target: &Path,
        sheet: Result<Arc<SchematicSheet>, String>,
    ) {
        if self.document_state.active_path.as_deref() != Some(target) {
            crate::diagnostics::log_info(format!(
                "{LABEL} cancelled: the sheet changed before the file was read."
            ));
            return;
        }
        match self.place_symbol_from_sheet(path, target, sheet) {
            Ok(done) => crate::diagnostics::log_info(done),
            Err(why) => crate::diagnostics::log_warning(format!("{LABEL}: {why}")),
        }
    }

    /// Place the sheet symbol for `sheet`, read from `path`, on `target`
    /// (the active sheet): what was placed, or why nothing was.
    fn place_symbol_from_sheet(
        &mut self,
        path: &Path,
        target: &Path,
        sheet: Result<Arc<SchematicSheet>, String>,
    ) -> Result<String, String> {
        let child = sheet.map_err(|why| format!("{} could not be read ({why})", path.display()))?;
        let docs = &self.document_state;
        let fold = crate::app::state::project_case::fold_of(docs.document_project(target));
        let target_key = fold_key(target, fold);
        if fold_key(path, fold) == target_key {
            return Err("a sheet cannot contain itself".to_string());
        }
        let child_dir = path.parent().unwrap_or_else(|| Path::new(""));
        let places_target = child.child_sheets.iter().any(|symbol| {
            resolve_child_reference(child_dir, &symbol.filename)
                .is_some_and(|placed| fold_key(&placed, fold) == target_key)
        });
        if places_target {
            return Err(format!(
                "{} places this sheet: a sheet cannot contain itself",
                path.display()
            ));
        }
        // Any sheet above this one, however far up, in the hierarchy the
        // Navigator holds (the walk the operand form's check starts with).
        if self
            .sheets_above_known(target, fold)
            .contains(&normalised_key(path, fold))
        {
            return Err(format!(
                "{} is above this sheet in the hierarchy: a sheet cannot contain itself",
                path.display()
            ));
        }
        let parent_dir = target.parent().unwrap_or_else(|| Path::new(""));
        let reference = reference_from(parent_dir, path, fold);
        let parent = docs
            .engines
            .get(target)
            .ok_or_else(|| "no schematic is open".to_string())?
            .document();
        if let Some(other) = signex_engine::other_sheet_symbol_using_file(
            &parent.child_sheets,
            uuid::Uuid::nil(),
            &reference,
        ) {
            return Err(format!(
                "sheet symbol '{}' already refers to {reference}; a file is placed once per sheet",
                other.name
            ));
        }
        let stem = path
            .file_stem()
            .map_or_else(|| "sheet".to_string(), |stem| stem.to_string_lossy().into_owned());
        let name = signex_engine::unused_sheet_symbol_name(parent, &stem);
        let at = self.symbol_from_sheet_spot(target, parent);
        let symbol = signex_engine::sheet_symbol_from_sheet(
            &child,
            &reference,
            &name,
            at,
            &mut signex_types::object_id::UuidV7Ids,
        );
        let entries = symbol.pins.len();
        let command = signex_engine::Command::PlaceChildSheet {
            child_sheet: symbol,
        };
        if !self.apply_engine_command(command, false, true) {
            return Err("the edit was refused".to_string());
        }
        Ok(format!(
            "{LABEL}: placed sheet symbol '{name}' for {reference} with {entries} sheet entr{}",
            if entries == 1 { "y" } else { "ies" }
        ))
    }

    /// The top-left corner of the new symbol: the cursor's last point on
    /// `target`, on the grid, else a spot under the sheet's content.
    fn symbol_from_sheet_spot(&self, target: &Path, parent: &SchematicSheet) -> Point {
        let ui = &self.ui_state;
        if ui.cursor_document.as_deref() == Some(target) {
            let (x, y) = self.snap_to_grid(ui.cursor_x, ui.cursor_y);
            return Point::new(x, y);
        }
        signex_engine::sheet_symbol_spot(parent)
    }
}
