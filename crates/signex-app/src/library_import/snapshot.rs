//! An imported library as a committed snapshot (#3114, contract §5–§6):
//! refuse an update over uncommitted work, run the converter, then set up
//! git and commit the result as one import commit. Blocking; call from
//! `spawn_blocking`.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use signex_library::import_snapshot::{
    IMPORT_TRAILER, commit_snapshot, prepare_snapshot_repo, read_snapshot_state,
};

use super::error::ImportError;
use super::events::ProgressEvent;
use super::run::{
    ConverterCommand, LibraryRunRequest, RunFailure, RunLimits, RunMode, RunOutcome,
    run_library_claimed, run_library_in,
};
use super::staging::{StagedLibrary, TargetBefore, staging_dir_for};

/// Where the library is mounted after the import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MountScope {
    /// Recorded as a global library, so every project sees it.
    #[default]
    AllProjects,
    /// Added to the active project's libraries.
    ThisProject,
}

/// What an update would run over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UpdateCheck {
    /// Commits that are not imports; the dialog warns before updating.
    pub local_commits: usize,
}

/// Before `--update`: refuse a folder with no repository, with no import
/// commit in its history or with uncommitted changes, and count the
/// commits since the last import.
pub fn check_update(library_dir: &Path) -> Result<UpdateCheck, ImportError> {
    let state = read_snapshot_state(library_dir).map_err(|e| ImportError::Git {
        detail: e.to_string(),
    })?;
    if !state.has_repo {
        return Err(ImportError::NotAnImportedLibrary {
            dir: library_dir.to_path_buf(),
        });
    }
    // Caner (2026-11-27): only a library Signex imported is offered an
    // update; a repository without an import commit is someone's own.
    if !state.has_import {
        return Err(ImportError::NoImportHistory {
            dir: library_dir.to_path_buf(),
        });
    }
    if !state.uncommitted.is_empty() {
        return Err(ImportError::UncommittedChanges {
            paths: state.uncommitted,
        });
    }
    Ok(UpdateCheck {
        local_commits: state.local_commits,
    })
}

/// One import: the converter and what to ask it.
#[derive(Debug, Clone)]
pub struct ImportJob {
    pub command: ConverterCommand,
    /// Name and version from `describe`, for the commit.
    pub converter: String,
    pub converter_version: String,
    pub request: LibraryRunRequest,
    pub limits: RunLimits,
    /// The converter's process group while it runs, for a kill at quit.
    pub group: super::process::ProcessGroup,
    /// How far [`Self::run`] has got, for the quit wait.
    pub stage: super::run_state::JobStage,
    /// A staged preview (#3171) to move into `request.output` and commit
    /// in place of a converter run; only for [`RunMode::Import`].
    pub adopt: Option<Box<StagedLibrary>>,
}

/// A finished import (or preview).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportJobOutcome {
    pub run: RunOutcome,
    /// The `.snxlib` to mount; `None` for a preview (a staged one too).
    pub library_file: Option<PathBuf>,
    /// The new commit; `None` for a preview or an unchanged re-import.
    pub commit: Option<String>,
    /// Non-import commits found before an update.
    pub local_commits: usize,
    /// The library's id, read from its `.snxlib`.
    pub library_id: Option<uuid::Uuid>,
    /// The library ids its manifest `requires`, to mount with it.
    pub requires: Vec<uuid::Uuid>,
    /// A staged preview's library (#3171), now the caller's to adopt or
    /// discard; `None` for every other run.
    pub staged: Option<Box<StagedLibrary>>,
}

impl ImportJob {
    /// Run the job. An update is refused before the converter starts unless
    /// the library is a clean repository; it holds the library's write lock
    /// until its commit, and a failed update puts the tree back to its last
    /// import. A [`RunMode::Stage`] job writes a staged preview beside the
    /// target; a job with [`Self::adopt`] moves one into place instead of
    /// running the converter (#3171). The stage is marked done once
    /// everything, the write lock included, is released.
    pub fn run(
        &self,
        cancel: &AtomicBool,
        on_event: &mut dyn FnMut(&ProgressEvent),
    ) -> Result<ImportJobOutcome, RunFailure> {
        // Declared first, so dropped last.
        let _done = self.stage.done_on_drop();
        if self.request.mode == RunMode::Stage {
            return self.run_staged(cancel, on_event);
        }
        if let Some(staged) = &self.adopt {
            return self.adopt_staged(staged, cancel);
        }
        let (local_commits, _write_lock) = match self.request.mode {
            RunMode::Update => {
                let output = &self.request.output;
                // Lock first: an edit landing after the clean check would
                // otherwise be wiped by a failed update's reset.
                let file = find_library_file(output).map_err(RunFailure::refused)?;
                let lock = signex_library::hold_library_write_lock(&file).map_err(|e| {
                    RunFailure::refused(ImportError::Git {
                        detail: e.to_string(),
                    })
                })?;
                let check = check_update(output).map_err(RunFailure::refused)?;
                (check.local_commits, Some(lock))
            }
            RunMode::Import | RunMode::Preview | RunMode::Stage => (0, None),
        };
        let run = run_library_in(
            &self.command,
            &self.request,
            self.limits,
            cancel,
            &self.group,
            &self.stage,
            on_event,
        )
        .map_err(|failure| self.restore_after_failed_update(failure))?;
        let Some(library_dir) = run.library.clone() else {
            return Ok(ImportJobOutcome {
                run,
                library_file: None,
                commit: None,
                local_commits,
                library_id: None,
                requires: Vec::new(),
                staged: None,
            });
        };
        self.commit_written(run, &library_dir, local_commits)
    }

    /// Find the library file in `library_dir`, commit it and read its
    /// identity. A failure here keeps the library: it was written.
    fn commit_written(
        &self,
        run: RunOutcome,
        library_dir: &Path,
        local_commits: usize,
    ) -> Result<ImportJobOutcome, RunFailure> {
        let library_file = find_library_file(library_dir).map_err(|e| keep_run(e, &run))?;
        let message = self.commit_message(&library_file);
        self.stage.settling();
        let commit = record_snapshot(library_dir, &message).map_err(|e| keep_run(e, &run))?;
        let (library_id, requires) = read_library_identity(&library_file);
        Ok(ImportJobOutcome {
            run,
            library_file: Some(library_file),
            commit,
            local_commits,
            library_id,
            requires,
            staged: None,
        })
    }

    /// The drawn preview (#3171): the converter writes the library into a
    /// new staging folder beside `request.output`, which is left as it is.
    /// Nothing is committed or mounted. A stop requested while the run
    /// ended (quitting) discards the staging folder, so none is left.
    fn run_staged(
        &self,
        cancel: &AtomicBool,
        on_event: &mut dyn FnMut(&ProgressEvent),
    ) -> Result<ImportJobOutcome, RunFailure> {
        let target = &self.request.output;
        let target_before = TargetBefore::read(target).map_err(RunFailure::refused)?;
        let staging = staging_dir_for(target).ok_or_else(|| {
            RunFailure::refused(ImportError::OutputUnusable {
                path: target.clone(),
                detail: "no parent folder".to_string(),
            })
        })?;
        let inner = LibraryRunRequest {
            output: staging,
            ..self.request.clone()
        };
        let (run, claimed) = run_library_claimed(
            &self.command,
            &inner,
            self.limits,
            cancel,
            &self.group,
            on_event,
        )?;
        let discard = |error: ImportError, run: &RunOutcome| {
            let mut failure = keep_run(error, run);
            failure.removed_output = claimed.discard();
            failure
        };
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(discard(ImportError::Cancelled, &run));
        }
        let library_dir = run.library.clone().unwrap_or_else(|| inner.output.clone());
        let library_file = find_library_file(&library_dir).map_err(|e| discard(e, &run))?;
        let staged = StagedLibrary {
            claimed: claimed.clone(),
            target: target.clone(),
            target_before,
            request: self.request.clone(),
            run: run.clone(),
            library_file,
        };
        // No library file: a staged preview is never mounted.
        Ok(ImportJobOutcome {
            run,
            library_file: None,
            commit: None,
            local_commits: 0,
            library_id: None,
            requires: Vec::new(),
            staged: Some(Box::new(staged)),
        })
    }

    /// Import of a staged preview: the converter does not run again. The
    /// staging folder is moved to `request.output` (refused, the preview
    /// kept, when that folder was created or changed since the preview),
    /// then committed as a run's library is.
    fn adopt_staged(
        &self,
        staged: &StagedLibrary,
        cancel: &AtomicBool,
    ) -> Result<ImportJobOutcome, RunFailure> {
        let kept = |error: ImportError| RunFailure {
            preview_kept: true,
            ..RunFailure::refused(error)
        };
        if self.request.mode != RunMode::Import || staged.target != self.request.output {
            return Err(kept(ImportError::TargetChanged {
                path: self.request.output.clone(),
            }));
        }
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(kept(ImportError::Cancelled));
        }
        // From here the library is being put in place; quitting waits.
        self.stage.settling();
        staged.move_into_place().map_err(|failure| {
            let mut run_failure = keep_run(failure.error, &staged.run);
            run_failure.refused = failure.preview_kept;
            run_failure.preview_kept = failure.preview_kept;
            run_failure
        })?;
        let run = staged.run_in_target();
        self.commit_written(run, &staged.target, 0)
    }

    /// An update failed after `check_update` found the tree clean: whatever
    /// the converter wrote is undone. Only reached for an update; a refusal
    /// before the converter started is left as it is.
    fn restore_after_failed_update(&self, mut failure: RunFailure) -> RunFailure {
        // A refused run wrote nothing, so there is nothing to undo.
        if self.request.mode == RunMode::Update && !failure.refused {
            self.stage.settling();
            let restored = signex_library::import_snapshot::restore_to_head(&self.request.output)
                .map_err(|e| e.to_string());
            if let Err(why) = &restored {
                log::warn!(
                    "library import: {} not restored: {why}",
                    self.request.output.display()
                );
            }
            failure.restored = Some(restored);
        }
        failure
    }

    fn commit_message(&self, library_file: &Path) -> String {
        let name = self.request.name.clone().unwrap_or_else(|| {
            library_file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        commit_message(
            self.request.mode,
            &name,
            &self.request.inputs,
            &self.converter,
            &self.converter_version,
        )
    }
}

/// A failure after the converter wrote a library: the run's report and
/// error output are kept with it.
fn keep_run(error: ImportError, run: &RunOutcome) -> RunFailure {
    RunFailure {
        error: Box::new(error),
        report: run.report.clone().map(Box::new),
        stderr_tail: run.stderr_tail.clone(),
        removed_output: false,
        restored: None,
        refused: false,
        preview_kept: false,
    }
}

/// `git init` when needed, the footprint diff setup, and one commit.
pub fn record_snapshot(library_dir: &Path, message: &str) -> Result<Option<String>, ImportError> {
    let git = |e: signex_library::LibraryError| ImportError::Git {
        detail: e.to_string(),
    };
    prepare_snapshot_repo(library_dir).map_err(git)?;
    commit_snapshot(library_dir, message).map_err(git)
}

/// The import commit's message: the subject names the library and its
/// sources, the trailer marks it as an import.
pub fn commit_message(
    mode: RunMode,
    name: &str,
    sources: &[PathBuf],
    converter: &str,
    version: &str,
) -> String {
    let verb = match mode {
        RunMode::Update => "Update",
        RunMode::Import | RunMode::Preview | RunMode::Stage => "Import",
    };
    let file_names: Vec<String> = sources
        .iter()
        .map(|p| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let mut message = format!("{verb} library {name} from {}\n\n", file_names.join(", "));
    message.push_str(&format!("Converted with {converter} {version} from:\n"));
    for source in sources {
        message.push_str(&format!("- {}\n", source.display()));
    }
    message.push_str(&format!("\n{IMPORT_TRAILER}: {converter} {version}\n"));
    message
}

/// The `library_id` of the `.snxlib` at `file` and the ids its manifest
/// `requires`; nothing when it does not parse.
fn read_library_identity(file: &Path) -> (Option<uuid::Uuid>, Vec<uuid::Uuid>) {
    let parsed = std::fs::read_to_string(file)
        .ok()
        .and_then(|text| signex_library::library_file::LibraryFile::parse(&text).ok());
    match parsed {
        Some(library) => (
            Some(library.manifest.library_id),
            library.manifest.library.required_libraries(),
        ),
        None => (None, Vec::new()),
    }
}

/// The `.snxlib` directly inside `dir`; the first by name when several.
pub fn find_library_file(dir: &Path) -> Result<PathBuf, ImportError> {
    let missing = || ImportError::NoLibraryFile {
        dir: dir.to_path_buf(),
    };
    let entries = std::fs::read_dir(dir).map_err(|_| missing())?;
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("snxlib"))
        })
        .collect();
    found.sort();
    found.into_iter().next().ok_or_else(missing)
}

/// The folder imported libraries go in by default: `~/Signex/Libraries`.
pub fn default_libraries_root() -> Option<PathBuf> {
    dirs::home_dir().map(|home| libraries_root_in(&home))
}

fn libraries_root_in(home: &Path) -> PathBuf {
    home.join("Signex").join("Libraries")
}

/// `<root>/<name>`, or `None` when `name` is not a portable folder name.
pub fn default_library_dir_in(root: &Path, name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.contains(['/', '\\']) || name == "." || name == ".." {
        return None;
    }
    let probe = root.join(name).join(format!("{name}.snxlib"));
    crate::library::commands::validate_new_library_name(&probe).ok()?;
    Some(root.join(name))
}

/// `~/Signex/Libraries/<name>`, the dialog's suggested output folder.
pub fn default_library_dir(name: &str) -> Option<PathBuf> {
    default_library_dir_in(&default_libraries_root()?, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_names_sources_converter_and_trailer() {
        let m = commit_message(
            RunMode::Import,
            "Passives",
            &[
                PathBuf::from("/in/Passives.lib1"),
                PathBuf::from("/in/Caps.lib1"),
            ],
            "example-import",
            "0.16.0",
        );
        assert!(m.starts_with("Import library Passives from Passives.lib1, Caps.lib1\n\n"));
        assert!(m.contains("- /in/Caps.lib1\n"));
        assert!(m.ends_with("\nSignex-Import: example-import 0.16.0\n"));
        assert!(signex_library::import_snapshot::is_import_commit(&m));
        let update = commit_message(RunMode::Update, "P", &[], "c", "1");
        assert!(update.starts_with("Update library P"));
    }

    #[test]
    fn the_default_folder_is_under_signex_libraries() {
        let root = libraries_root_in(Path::new("/home/u"));
        assert_eq!(root, PathBuf::from("/home/u/Signex/Libraries"));
        assert_eq!(
            default_library_dir_in(&root, " Passives "),
            Some(PathBuf::from("/home/u/Signex/Libraries/Passives"))
        );
        assert_eq!(default_library_dir_in(&root, "a/b"), None);
        assert_eq!(default_library_dir_in(&root, ""), None);
        assert_eq!(default_library_dir_in(&root, ".."), None);
    }

    #[test]
    fn a_folder_without_a_library_file_is_named() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            find_library_file(dir.path()),
            Err(ImportError::NoLibraryFile { .. })
        ));
        std::fs::write(dir.path().join("B.snxlib"), "").unwrap();
        std::fs::write(dir.path().join("A.snxlib"), "").unwrap();
        assert_eq!(
            find_library_file(dir.path()).unwrap(),
            dir.path().join("A.snxlib")
        );
    }
}
