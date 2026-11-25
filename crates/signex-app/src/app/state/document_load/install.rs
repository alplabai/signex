//! A sheet's engine installed, at the active tab or behind another one.

use std::path::{Path, PathBuf};

use signex_engine::{Engine, EngineError};
use signex_i18n_ids::{MessageId, Translatable, ids};
use signex_types::schematic::SchematicSheet;

use super::SheetInstall;
use crate::app::core::event::{Notice, NoticeLevel};
use crate::app::state::DocumentState;

impl DocumentState {
    /// Install `schematic` as the engine of the active tab. With no active
    /// tab, or no sheet, the active engine is dropped.
    pub(crate) fn sync_engine_from_schematic(
        &mut self,
        schematic: Option<SchematicSheet>,
    ) -> SheetInstall {
        let path = self.active_tab_path();
        match (path, schematic) {
            (Some(p), Some(sheet)) => {
                let result = Engine::new_with_path(sheet, Some(p.clone()));
                self.install_active(p, result)
            }
            _ => {
                self.clear_active_engine();
                SheetInstall::failed(None)
            }
        }
    }

    /// Make `result` the engine of the active tab at `p`. A failed engine
    /// leaves the one already open there: it may hold unsaved edits that
    /// dropping it would lose silently.
    pub(crate) fn install_active(
        &mut self,
        p: PathBuf,
        result: Result<Engine, EngineError>,
    ) -> SheetInstall {
        match result {
            Ok(engine) => {
                self.insert_engine(p.clone(), engine);
                self.active_path = Some(p);
                SheetInstall::installed()
            }
            Err(error) => SheetInstall::failed(Some(failed_install(
                ids::NOTICE_LOAD_NOT_INSTALLED,
                &p,
                &error,
            ))),
        }
    }

    /// Make `result` the engine of the sheet at `path`, behind a tab not in
    /// view. A failed engine drops the stale one, so a save cannot undo the
    /// restore. The map is written directly: the documents epoch and the id
    /// header stay as they were (F5, #2326 D5).
    pub(crate) fn replace_engine_at(
        &mut self,
        path: &Path,
        result: Result<Engine, EngineError>,
    ) -> SheetInstall {
        match result {
            Ok(engine) => {
                self.engines.insert(path.to_path_buf(), engine);
                SheetInstall::installed()
            }
            Err(error) => {
                self.engines.remove(path);
                SheetInstall::failed(Some(failed_install(
                    ids::NOTICE_LOAD_RESTORE_ENGINE_FAILED,
                    path,
                    &error,
                )))
            }
        }
    }
}

/// The warning for the sheet at `path` whose engine `error` kept from
/// starting; `line` is a catalog line with `{ $file }` and `{ $error }`.
fn failed_install(line: MessageId, path: &Path, error: &EngineError) -> Notice {
    let text = Translatable::new(line)
        .arg("file", path.display().to_string())
        .arg("error", error.to_string());
    Notice::new(NoticeLevel::Warning, text)
}
