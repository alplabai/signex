//! Place Design Block (#1667): the content of another sheet file lands on
//! the cursor. Sheet symbols in that file are left out (each names a child
//! file relative to its own sheet), and the Messages panel says how many.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_engine::DesignBlock;
use signex_i18n::ids;
use signex_types::reuse::{ReuseKind, ReuseScope};
use signex_types::schematic::SchematicSheet;

use crate::app::{EditMsg, Message, PlacementExtrasMsg, Signex};

const WHAT: &str = "Place design block";

fn extras(msg: PlacementExtrasMsg) -> Message {
    Message::Edit(EditMsg::Extras(msg))
}

/// Read the sheet at `path`. Blocking: run it off the UI thread.
pub(crate) fn read_block_sheet(path: &Path) -> Result<Arc<SchematicSheet>, String> {
    match crate::app::project_sheets::load_sheet_file(path) {
        Ok(Some(sheet)) => Ok(Arc::new(sheet)),
        Ok(None) => Err(format!("there is no file at {}", path.display())),
        Err(error) => Err(error),
    }
}

impl Signex {
    /// Ask for the sheet file to place.
    pub(super) fn design_block_dialog(&mut self) -> Task<Message> {
        if self.fragment_target(WHAT).is_none() {
            return Task::none();
        }
        self.pick_block_file(ids::FILE_DIALOG_PLACE_DESIGN_BLOCK, None)
    }

    /// Place Snippet without operands (#1283): the same dialog, opened on
    /// the snippets folder of the sheet's project, else of the user's
    /// library. The picked snippet then lands on the cursor as a design
    /// block does.
    pub(super) fn snippet_dialog(&mut self) -> Task<Message> {
        let Some(target) = self.fragment_target("Place snippet") else {
            return Task::none();
        };
        let folder = [ReuseScope::Project, ReuseScope::Library]
            .into_iter()
            .find_map(|scope| self.reuse_root(scope, Some(&target)).ok())
            .map(|root| ReuseKind::Snippet.folder(&root));
        self.pick_block_file(ids::FILE_DIALOG_PLACE_SNIPPET, folder)
    }

    /// The sheet file dialog of a design block, titled `title`, opened on
    /// `folder` when one is given.
    fn pick_block_file(
        &self,
        title: signex_i18n::MessageId,
        folder: Option<PathBuf>,
    ) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, title)
            .filter_in(i18n, ids::FILE_FILTER_SCHEMATIC, &["snxsch"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        let dialog = match folder {
            Some(folder) => dialog.set_directory(folder),
            None => dialog,
        };
        Task::perform(dialog.pick_file(), |path| {
            extras(PlacementExtrasMsg::DesignBlockPicked(path))
        })
    }

    /// Read the picked file off the UI thread, for the active sheet.
    pub(super) fn read_design_block(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let Some(target) = self.fragment_target(WHAT) else {
            return Task::none();
        };
        let epoch = self.interaction_state.placement_epoch;
        Task::perform(
            async move {
                let read = path.clone();
                let sheet = tokio::task::spawn_blocking(move || read_block_sheet(&read))
                    .await
                    .unwrap_or_else(|error| Err(format!("the read failed: {error}")));
                (path, sheet)
            },
            move |(path, sheet)| {
                extras(PlacementExtrasMsg::DesignBlockRead {
                    path,
                    target: target.clone(),
                    epoch,
                    sheet,
                })
            },
        )
    }

    /// The file was read: put its content on the cursor.
    pub(super) fn design_block_read(
        &mut self,
        path: &Path,
        target: &Path,
        epoch: u64,
        sheet: Result<Arc<SchematicSheet>, String>,
    ) {
        if !self.fragment_read_is_current(target, epoch, WHAT) {
            return;
        }
        match self.design_block_from(path, sheet) {
            Ok(block) => self.arm_fragment(block.copied, target),
            Err(reason) => crate::diagnostics::log_warning(format!("{WHAT}: {reason}")),
        }
    }

    /// The design block in `sheet`, read from `path`, or why there is none.
    /// Says in the Messages panel what was left out.
    pub(crate) fn design_block_from(
        &self,
        path: &Path,
        sheet: Result<Arc<SchematicSheet>, String>,
    ) -> Result<DesignBlock, String> {
        let sheet =
            sheet.map_err(|error| format!("{} could not be read ({error})", path.display()))?;
        let block = DesignBlock::from_sheet(
            &sheet,
            self.fragment_grid_mm(),
            &mut signex_types::object_id::UuidV7Ids,
        )
        .map_err(|error| format!("{} could not be placed ({error})", path.display()))?
        .ok_or_else(|| format!("{} holds nothing to place", path.display()))?;
        if block.skipped_sheet_symbols > 0 {
            crate::diagnostics::log_info(format!(
                "{WHAT}: {} sheet symbol(s) in {} are not placed; a design block carries parts, wires, labels, text and drawings.",
                block.skipped_sheet_symbols,
                path.display()
            ));
        }
        Ok(block)
    }
}
