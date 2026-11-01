//! A schematic whose file was changed or deleted by another program (#1826).
//!
//! The save job compares the file with what the document was loaded from or
//! last saved as (`persist::disk_baseline`) and writes nothing when they differ.
//! This module is what happens next: the failure reaches Messages, the sheet
//! stays modified, and the notice card asks Overwrite / Reload / Cancel.
//! See `docs/architecture/external-change.md`.

use std::path::{Path, PathBuf};

use iced::Task;

use super::save::file_label;
use crate::app::restore_job::RestoreJobDocument;
use crate::app::state::{ErrorNotice, NoticeAction};
use crate::app::{FileMsg, Message, Signex, TabKind};
use crate::persist::{ContentHash, DiskDrift};

/// A sheet read back from disk for a Reload, with the fingerprint of the bytes
/// it was parsed from.
#[derive(Debug, Clone)]
pub struct ReloadedSheet {
    pub sheet: Box<signex_types::schematic::SchematicSheet>,
    pub hash: ContentHash,
}

/// Blocking half of a Reload; run it on a worker.
fn read_for_reload(path: &Path) -> Result<Box<ReloadedSheet>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not be read: {e}"))?;
    let parsed = signex_types::format::SnxSchematic::parse(&text)
        .map_err(|e| format!("could not be parsed: {e}"))?;
    Ok(Box::new(ReloadedSheet {
        sheet: Box::new(parsed.sheet),
        hash: ContentHash::of(text.as_bytes()),
    }))
}

impl Signex {
    /// Record what a read of `path` learned: the fingerprint later saves
    /// compare with, and the real file behind the path.
    pub(crate) fn record_disk_read(&mut self, path: &Path, disk: crate::persist::DiskRead) {
        self.record_disk_baseline(path, disk.hash);
        if let Some(real) = disk.real_path {
            self.document_state
                .document_saves
                .set_real_path(path.to_path_buf(), real);
        }
    }

    /// `path` was reached through a symlink to a file another open document
    /// already is (#1827): show that document and report true, so no second
    /// engine is made for one file.
    pub(crate) fn focus_alias_if_open(
        &mut self,
        path: &Path,
        disk: &crate::persist::DiskRead,
    ) -> bool {
        let Some(real) = disk.real_path.as_deref() else {
            return false;
        };
        let held = self
            .document_state
            .document_saves
            .alias_of(path, real, |held| {
                self.document_state.tabs.iter().any(|tab| tab.path == held)
            });
        let Some(held) = held else {
            return false;
        };
        crate::diagnostics::log_info(format!(
            "[open] {} is the same file as the open {}; showing that one",
            file_label(path),
            file_label(&held)
        ));
        self.activate_tab_for_path(&held);
        true
    }

    /// A loaded project whose `.snxprj` is the same file as `project_path`
    /// once symlinks are followed, under another spelling.
    pub(crate) fn loaded_project_aliasing(
        &self,
        project_path: &Path,
    ) -> Option<crate::app::state::ProjectId> {
        let real = std::fs::canonicalize(project_path).ok()?;
        self.document_state
            .projects
            .iter()
            .find(|p| {
                p.path != project_path
                    && std::fs::canonicalize(&p.path).is_ok_and(|held| held == real)
            })
            .map(|p| p.id)
    }

    /// The fingerprint a read of `path` just took (open, reload, restore);
    /// later saves compare to it.
    pub(crate) fn record_disk_baseline(&mut self, path: &Path, hash: ContentHash) {
        self.document_state
            .document_saves
            .set_baseline_from_read(path.to_path_buf(), hash);
    }

    /// A schematic write landed: the file now holds `saved`. Only the newest
    /// write begun for the file says what is on disk.
    pub(crate) fn note_written_baseline(
        &mut self,
        path: &Path,
        saved: &crate::app::state::CommitBytes,
    ) {
        if super::save_async::is_pcb_path(path)
            || !self
                .document_state
                .document_saves
                .is_newest_begun(path, saved.sequence)
        {
            return;
        }
        self.document_state
            .document_saves
            .set_baseline(path.to_path_buf(), ContentHash::of(&saved.bytes));
    }

    /// For the blocking save of `try_save_dirty_path_with` (Exclude from
    /// Project on a dirty sheet), which writes the engine itself: refuse when
    /// the file drifted. No card here; the caller reports the error.
    pub(crate) fn refuse_if_disk_drifted(&self, path: &Path) -> anyhow::Result<()> {
        let drift = self
            .document_state
            .document_saves
            .drift_of(path)
            .map_err(|e| {
                anyhow::anyhow!("could not read {} before saving: {e}", file_label(path))
            })?;
        let Some(drift) = drift else {
            return Ok(());
        };
        let name = file_label(path);
        let what = drift.describe();
        crate::diagnostics::log_warning(format!("[save] {name} {what}; not saved, still modified"));
        anyhow::bail!(
            "{name} {what}, so it was not overwritten. Save it from its tab to choose \
             Overwrite or Reload"
        )
    }

    /// After the blocking Save All leg wrote `path`: what is on disk is now
    /// the baseline.
    pub(crate) fn refresh_baseline_from_disk(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => self.record_disk_baseline(path, ContentHash::of(&bytes)),
            Err(e) => {
                crate::diagnostics::log_warning(format!(
                    "[save] {} could not be read back after saving ({e}); its next save is \
                     not compared with the file",
                    file_label(path)
                ));
                self.document_state.document_saves.forget_baseline(path);
            }
        }
    }

    /// A parent sheet was re-pointed on disk outside the save ledger (#1849).
    /// The open document keeps its unsaved edits, so its next save must compare
    /// with the patched bytes now on disk, not with what it was opened from.
    pub(crate) fn record_written_parent_baselines(
        &mut self,
        disk: &crate::app::parent_reference_disk::ParentDiskOutcomes,
    ) {
        use crate::app::parent_reference_disk::ParentDisk;
        for (parent, outcome) in disk {
            let ParentDisk::Written(bytes, read) = outcome else {
                continue;
            };
            let saves = &self.document_state.document_saves;
            if saves.baseline(parent).is_none() {
                continue;
            }
            // The rewrite patched whatever was on disk. If that was not the file
            // the document knew, another program changed it first: keep the old
            // baseline, so the next save shows the card instead of overwriting.
            if saves.knows(parent, *read) {
                self.record_disk_baseline(parent, ContentHash::of(bytes));
            } else {
                crate::diagnostics::log_warning(format!(
                    "[save] {} was changed on disk by another program before its sheet \
                     reference was updated; saving it will ask first",
                    file_label(parent)
                ));
            }
        }
    }

    /// A save refused to write because the file drifted. Nothing was written,
    /// so the sheet stays modified; the user is asked once per card.
    pub(crate) fn handle_save_found_disk_drift(&mut self, path: PathBuf, drift: DiskDrift) {
        let what = drift.describe();
        let name = file_label(&path);
        crate::diagnostics::log_warning(format!("[save] {name} {what}; not saved, still modified"));
        self.document_state
            .document_saves
            .record_failure(path.clone(), format!("{name} {what}"));
        self.document_state.sync_history_dirty(&path);
        // One card at a time: a second drifted sheet of a Save All is in
        // Messages and asks again on its next save.
        if self.document_state.error_notice.is_none() {
            self.document_state.error_notice = Some(ErrorNotice::disk_changed(path, drift));
        }
    }

    fn take_disk_conflict(&mut self) -> Option<PathBuf> {
        let notice = self.document_state.error_notice.as_ref()?;
        let Some(NoticeAction::DiskChanged { path, .. }) = notice.action.clone() else {
            return None;
        };
        self.document_state.error_notice = None;
        Some(path)
    }

    /// The user chose Overwrite (or Re-create): save again without comparing.
    pub(crate) fn handle_disk_conflict_overwrite(&mut self) -> Task<Message> {
        let Some(path) = self.take_disk_conflict() else {
            return Task::none();
        };
        crate::diagnostics::log_info(format!(
            "[save] Overwriting {} as the user chose",
            file_label(&path)
        ));
        match self.begin_schematic_save_unchecked(path) {
            Ok(task) => task,
            Err(error) => {
                self.report_save_failure("Failed to save active document", &error);
                Task::none()
            }
        }
    }

    /// The user chose Reload from disk: read the file and replace the document.
    /// The unsaved edits are dropped only when the read succeeds.
    pub(crate) fn handle_disk_conflict_reload(&mut self) -> Task<Message> {
        let Some(path) = self.take_disk_conflict() else {
            return Task::none();
        };
        if self.document_state.document_saves.is_saving(&path) {
            crate::diagnostics::log_warning(format!(
                "[reload] {} is still saving; reload it once the write finishes",
                file_label(&path)
            ));
            return Task::none();
        }
        let read_path = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_for_reload(&read_path))
                    .await
                    .unwrap_or_else(|e| Err(format!("the reload worker failed: {e}")))
            },
            move |result| Message::File(FileMsg::DiskReloadFinished { path, result }),
        )
    }

    pub(crate) fn handle_disk_reload_finished(
        &mut self,
        path: PathBuf,
        result: Result<Box<ReloadedSheet>, String>,
    ) -> Task<Message> {
        let name = file_label(&path);
        let reloaded = match result {
            Ok(reloaded) => reloaded,
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "[reload] {name} {why}; the document keeps its edits"
                ));
                return Task::none();
            }
        };
        let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.path == path && matches!(tab.kind, TabKind::Schematic))
        else {
            crate::diagnostics::log_warning(format!(
                "[reload] {name} is no longer open; nothing was reloaded"
            ));
            return Task::none();
        };
        let ReloadedSheet { sheet, hash } = *reloaded;
        let task = self.apply_reloaded(idx, Ok(RestoreJobDocument::Schematic(sheet)), None);
        self.document_state.mark_document_clean(&path);
        self.document_state.document_saves.clear_failure(&path);
        self.record_disk_baseline(&path, hash);
        crate::diagnostics::log_info(format!("[reload] Reloaded {name} from disk"));
        self.refresh_panel_ctx();
        Task::batch([task, self.autosave_after_disk_write(&path)])
    }
}
