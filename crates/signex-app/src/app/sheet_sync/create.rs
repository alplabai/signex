//! Create Sheet Symbol From Sheet: pick a sheet file, read what its checks
//! need off the UI thread (the file, the project's closed sheets, a link out
//! of the project folder), and place a sheet symbol for it on the active sheet
//! with an entry for every port, as one undo step. The checks and the
//! placement are the operand form's (`place_symbol_from_sheet_file`), so one
//! command id has one behaviour. The symbol goes to the cursor's last point on
//! this sheet, else under the sheet's content.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::ids;
use signex_types::schematic::Point;

use super::SheetSyncMsg;
use crate::app::command::EditReads;
use crate::app::{Message, Signex};

const LABEL: &str = "Create Sheet Symbol From Sheet";

/// A refusal of the shared checks as the dialog's user reads it. The checks
/// word their refusals for the operand form (`file` is the operand, and the
/// way past is `allow_merge=true`, which the dialog does not offer); the
/// dialog names the file and says what to do in the editor instead.
fn gui_wording(why: &str) -> String {
    const CLI_TO_GUI: [(&str, &str); 6] = [
        (" (see docs/CLI.md, \"Why allow_merge\")", ""),
        (
            "fix the project, or pass allow_merge=true to place it anyway",
            "fix the project first",
        ),
        (
            "pass allow_merge=true to merge on purpose",
            "rename the entries first",
        ),
        (
            "pass allow_merge=true if that is intended",
            "rename the entries first",
        ),
        (
            "pass allow_merge=true to place it anyway",
            "it was not placed",
        ),
        ("`file`", "the file"),
    ];
    let mut text = why.to_string();
    for (cli, gui) in CLI_TO_GUI {
        text = text.replace(cli, gui);
    }
    // A refusal that began with the operand name begins a sentence.
    match text.strip_prefix("the file") {
        Some(rest) => format!("The file{rest}"),
        None => text,
    }
}

/// The reads Create Sheet Symbol From Sheet waited for, for the sheet
/// `target`, and the revisions they depend on.
#[derive(Debug, Clone)]
pub struct SymbolSheetReads {
    pub(super) path: PathBuf,
    pub(super) target: PathBuf,
    pub(super) revisions: BTreeMap<PathBuf, u64>,
    pub(super) reads: Arc<EditReads>,
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

    /// Read what placing the picked file on the active sheet needs, off the
    /// UI thread.
    pub(super) fn read_symbol_sheet(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let Some(target) = self.create_symbol_target() else {
            return Task::none();
        };
        let (revisions, read) = self.symbol_sheet_reads(&target, &path);
        Task::perform(read, move |reads| {
            Message::from(SheetSyncMsg::CreateSymbolRead(SymbolSheetReads {
                path: path.clone(),
                target: target.clone(),
                revisions: revisions.clone(),
                reads: Arc::new(reads),
            }))
        })
    }

    /// The reads landed: place the sheet symbol, unless the user moved to
    /// another sheet meanwhile. The outcome goes to the Messages panel.
    pub(super) fn symbol_sheet_read(&mut self, read: SymbolSheetReads) {
        let SymbolSheetReads {
            path,
            target,
            revisions,
            reads,
        } = read;
        if self.document_state.active_path.as_deref() != Some(target.as_path()) {
            crate::diagnostics::log_info(format!(
                "{LABEL} cancelled: the sheet changed before the file was read."
            ));
            return;
        }
        let at = self.symbol_from_sheet_spot(&target);
        let file = path.to_string_lossy().into_owned();
        let placed = self.with_gui_reads(&target, &revisions, reads, |app| {
            app.place_symbol_from_sheet_file(&file, at, false)
        });
        match placed {
            Ok(placed) => {
                let entries = placed.entries.len();
                crate::diagnostics::log_info(format!(
                    "{LABEL}: placed sheet symbol '{}' for {} with {entries} sheet entr{}",
                    placed.name,
                    placed.reference,
                    if entries == 1 { "y" } else { "ies" }
                ));
            }
            Err(why) => crate::diagnostics::log_warning(format!("{LABEL}: {}", gui_wording(&why))),
        }
    }

    /// The top-left corner of the new symbol: the cursor's last point on
    /// `target`, on the grid; `None` puts it under the sheet's content.
    fn symbol_from_sheet_spot(&self, target: &Path) -> Option<Point> {
        let ui = &self.ui_state;
        if ui.cursor_document.as_deref() != Some(target) {
            return None;
        }
        let (x, y) = self.snap_to_grid(ui.cursor_x, ui.cursor_y);
        Some(Point::new(x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::gui_wording;

    /// The dialog never tells its user to pass an operand: the operand form's
    /// way past (`allow_merge=true`) becomes what to do in the editor.
    #[test]
    fn a_refusal_in_the_dialog_has_no_cli_wording() {
        let merge = "a sheet symbol at 10,20 would give a net of the project the names {A, B}, \
                     some of which reach it from other sheets; pass allow_merge=true to merge on \
                     purpose (see docs/CLI.md, \"Why allow_merge\")";
        assert_eq!(
            gui_wording(merge),
            "a sheet symbol at 10,20 would give a net of the project the names {A, B}, \
             some of which reach it from other sheets; rename the entries first"
        );
        let placed = "`file` is already placed on top.snxsch; placing it again changes its nets; \
             pass allow_merge=true to place it anyway";
        assert_eq!(
            gui_wording(placed),
            "The file is already placed on top.snxsch; placing it again changes its nets; \
             it was not placed"
        );
        for refusal in [merge, placed] {
            assert!(!gui_wording(refusal).contains("allow_merge"), "{refusal}");
        }
    }
}
