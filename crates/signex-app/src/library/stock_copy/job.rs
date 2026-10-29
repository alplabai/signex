//! The blocking half of a stock copy: create the target library if asked,
//! write the copy, bring its STEP models along. Runs on a worker, never in
//! `update`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use signex_library::{
    FootprintFile, LibraryAdapter as _, LibraryError, LocalGitAdapter, SymbolFile,
};
use uuid::Uuid;

use crate::panels::components_panel::global_prefs::{self, GlobalLibraryEntry};

use super::{CopyTarget, StockPrimitiveKind};
use crate::library::state::NewLibraryDefaults;

/// Where the stock file to copy comes from.
pub enum StockSource {
    /// The file the user opened.
    File(PathBuf),
    /// The stock file whose `file_uuid` a copy recorded; found by scanning
    /// the stock library's directory for its kind. Used by Copy Again,
    /// which starts from the copy.
    ByUuid { stock_dir: PathBuf, uuid: Uuid },
}

/// One copy of one stock file.
pub struct StockCopyJob {
    /// What the in-flight record and the outcome are keyed by.
    pub key: PathBuf,
    pub source: StockSource,
    pub kind: StockPrimitiveKind,
    /// The stock library's directory: where the source's STEP models live.
    pub source_library_dir: PathBuf,
    pub target: CopyTarget,
    /// Make a new copy even when the target already holds one of this
    /// stock primitive.
    pub copy_again: bool,
    /// Seeds the manifest of a library this copy creates.
    pub new_library_defaults: NewLibraryDefaults,
    pub now: chrono::DateTime<chrono::Utc>,
}

/// How a copy ended. Cloneable so it can ride a message.
#[derive(Debug, Clone)]
pub struct StockCopyOutcome {
    /// The job's `key`.
    pub source: PathBuf,
    /// The `.snxlib` the copy went to (or was to be created at).
    pub library: PathBuf,
    /// This job created `library` (whether or not the copy then worked).
    pub created_library: bool,
    /// An existing copy was found, so nothing was written.
    pub reused: bool,
    /// The copy's file, or why there is none.
    pub result: Result<PathBuf, String>,
    /// The global library list after registering a created library, so it
    /// survives a restart; `None` when no library was created.
    pub global_libraries: Option<Result<Vec<GlobalLibraryEntry>, String>>,
}

impl StockCopyJob {
    pub fn run(self) -> StockCopyOutcome {
        let library = match &self.target {
            CopyTarget::Existing(path) | CopyTarget::New(path) => path.clone(),
        };
        let mut outcome = StockCopyOutcome {
            source: self.key.clone(),
            library: library.clone(),
            created_library: false,
            reused: false,
            result: Err(String::new()),
            global_libraries: None,
        };
        if matches!(self.target, CopyTarget::New(_)) {
            if let Err(e) =
                crate::library::commands::create_library_file(&library, &self.new_library_defaults)
            {
                outcome.result = Err(e.to_string());
                return outcome;
            }
            outcome.created_library = true;
            outcome.global_libraries = Some(global_prefs::add_path(library.clone()));
        }
        outcome.result = match self.copy() {
            Ok((path, reused)) => {
                outcome.reused = reused;
                Ok(path)
            }
            Err(e) => Err(e.to_string()),
        };
        outcome
    }

    fn source_path(&self) -> Result<PathBuf, LibraryError> {
        match &self.source {
            StockSource::File(path) => Ok(path.clone()),
            StockSource::ByUuid { stock_dir, uuid } => {
                files_in(&stock_dir.join(self.kind.directory()), self.kind)
                    .into_iter()
                    .find(|path| {
                        std::fs::read_to_string(path)
                            .is_ok_and(|text| text.contains(&format!("file_uuid = \"{uuid}\"")))
                    })
                    .ok_or_else(|| {
                        LibraryError::NotFound(format!("the stock file {uuid} was not found"))
                    })
            }
        }
    }

    fn copy(self) -> Result<(PathBuf, bool), LibraryError> {
        let library_file = match &self.target {
            CopyTarget::Existing(path) | CopyTarget::New(path) => path.clone(),
        };
        let adapter = LocalGitAdapter::open(&library_file)?;
        let target_dir = adapter
            .root_dir()
            .ok_or_else(|| {
                LibraryError::Backend(format!(
                    "{} has no directory to copy into",
                    library_file.display()
                ))
            })?
            .to_path_buf();

        let bytes = std::fs::read(self.source_path()?)?;
        let source_uuid = match self.kind {
            StockPrimitiveKind::Symbol => {
                SymbolFile::from_bytes(&bytes).map_err(backend)?.file_uuid
            }
            StockPrimitiveKind::Footprint => {
                FootprintFile::from_bytes(&bytes)
                    .map_err(backend)?
                    .file_uuid
            }
        };
        if !self.copy_again
            && let Some(existing) = find_existing_copy(&target_dir, self.kind, source_uuid)
        {
            return Ok((existing, true));
        }

        // The names the library holds now, read from its files, so the
        // symbol name and the file name are judged against the same disk.
        let taken_names: HashSet<String> = match self.kind {
            StockPrimitiveKind::Symbol => adapter.list_symbols()?,
            StockPrimitiveKind::Footprint => adapter.list_footprints()?,
        }
        .into_iter()
        .map(|summary| summary.name)
        .collect();

        // A symbol file is found by scanning the directory, so it is named
        // after its symbol, as the stock files are. A footprint file is
        // found by the footprint's uuid, so it is named by that.
        let (text, name, file_stem, step_hashes) = match self.kind {
            StockPrimitiveKind::Symbol => {
                let file = SymbolFile::from_bytes(&bytes).map_err(backend)?;
                let copy = file.copy_for_library(&taken_names, self.now);
                let name = copy.symbols[0].name.clone();
                let stem = file_stem_for(&name);
                (
                    copy.to_toml_string().map_err(backend)?,
                    name,
                    stem,
                    Vec::new(),
                )
            }
            StockPrimitiveKind::Footprint => {
                let file = FootprintFile::from_bytes(&bytes).map_err(backend)?;
                let copy = file.copy_for_library(&taken_names, self.now);
                let name = copy.footprints[0].name.clone();
                let stem = copy.footprints[0].uuid.to_string();
                let hashes = copy.step_hashes();
                (copy.to_toml_string().map_err(backend)?, name, stem, hashes)
            }
        };

        let message = format!("Copy stock {} {name}", self.kind.noun());
        for hash in &step_hashes {
            copy_step_model(
                &self.source_library_dir,
                &target_dir,
                hash,
                &message,
                &adapter,
            );
        }

        let directory = target_dir.join(self.kind.directory());
        std::fs::create_dir_all(&directory)?;
        let destination = free_file(&directory, &file_stem, self.kind.extension());
        let written = adapter.rewrite_file_locked(&destination, &message, &mut |current| {
            if current.is_some() {
                return Err(LibraryError::Conflict(format!(
                    "{} appeared while copying; nothing was overwritten",
                    destination.display()
                )));
            }
            Ok(Some(text.clone().into_bytes()))
        });
        // The adapter is opened for this one copy: report what its commits
        // left out before it goes (#1729).
        let library = adapter.manifest().library.name.clone();
        crate::library::held_back::report_held_back(&library, &adapter);
        written?;
        Ok((destination, false))
    }
}

/// The primitive files of `kind` directly in `directory`, sorted.
fn files_in(directory: &Path, kind: StockPrimitiveKind) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| StockPrimitiveKind::of_path(path) == Some(kind))
        .collect();
    files.sort();
    files
}

/// The copy of the stock file `source_uuid` already in `target_dir`, if any.
fn find_existing_copy(
    target_dir: &Path,
    kind: StockPrimitiveKind,
    source_uuid: Uuid,
) -> Option<PathBuf> {
    let needle = format!("copied_from = \"{source_uuid}\"");
    files_in(&target_dir.join(kind.directory()), kind)
        .into_iter()
        .find(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(&needle)))
}

fn backend(error: impl std::fmt::Display) -> LibraryError {
    LibraryError::Backend(error.to_string())
}

/// Bring `step/<hash>.step` from the source library to the target. A model
/// the stock library does not hold stays a reference only; one the target
/// already holds is left as it is (addressed by content hash).
fn copy_step_model(
    source_dir: &Path,
    target_dir: &Path,
    hash: &str,
    message: &str,
    adapter: &LocalGitAdapter,
) {
    // The hash names a file; only a hex digest is accepted, so a hostile
    // footprint file cannot point the copy outside `step/`.
    if hash.is_empty() || hash.len() > 128 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        tracing::warn!(
            target: "signex::library",
            hash = %hash,
            "a copied footprint names a STEP model that is not a hex digest; the model was not copied"
        );
        return;
    }
    let name = format!("{hash}.step");
    let from = source_dir.join("step").join(&name);
    let to = target_dir.join("step").join(&name);
    if to.exists() {
        return;
    }
    let written = std::fs::read(&from)
        .and_then(|bytes| {
            std::fs::create_dir_all(target_dir.join("step"))?;
            signex_types::atomic_io::atomic_write(&to, &bytes)
        })
        .map_err(|e| e.to_string());
    match written {
        Ok(()) => {
            if let Err(e) = adapter.commit_external_change(&to, message) {
                tracing::warn!(
                    target: "signex::library",
                    path = %to.display(),
                    error = %e,
                    "copied STEP model not committed"
                );
            }
        }
        Err(e) => tracing::warn!(
            target: "signex::library",
            path = %from.display(),
            error = %e,
            "STEP model of a copied footprint could not be copied; the copy keeps the reference"
        ),
    }
}

/// `<directory>/<stem>.<ext>`, with ` (2)`, ` (3)`, ... when the file exists.
fn free_file(directory: &Path, stem: &str, extension: &str) -> PathBuf {
    let first = directory.join(format!("{stem}.{extension}"));
    if !first.exists() {
        return first;
    }
    (2u32..)
        .map(|n| directory.join(format!("{stem} ({n}).{extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

/// A primitive name as a portable file stem.
fn file_stem_for(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        "copy".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_portable_file_stem() {
        assert_eq!(file_stem_for("R"), "R");
        assert_eq!(file_stem_for("a/b:c"), "a_b_c");
        assert_eq!(file_stem_for("  "), "copy");
        assert_eq!(file_stem_for("name."), "name");
    }

    #[test]
    fn a_step_name_that_is_not_hex_is_not_copied() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("src");
        let target = dir.path().join("dst");
        std::fs::create_dir_all(source.join("step")).unwrap();
        std::fs::write(source.join("evil.step"), b"x").unwrap();
        std::fs::create_dir_all(&target).unwrap();
        let library = dir.path().join("T.snxlib");
        crate::library::commands::create_library_file(&library, &NewLibraryDefaults::default())
            .unwrap();
        let adapter = LocalGitAdapter::open(&library).unwrap();
        copy_step_model(&source, &target, "../evil", "m", &adapter);
        copy_step_model(&source, &target, "", "m", &adapter);
        assert!(!target.join("step").exists());
        assert!(!target.join("evil.step").exists());
    }

    #[test]
    fn a_taken_file_name_gets_a_suffix() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            free_file(dir.path(), "R", "snxsym"),
            dir.path().join("R.snxsym")
        );
        std::fs::write(dir.path().join("R.snxsym"), b"x").expect("write");
        assert_eq!(
            free_file(dir.path(), "R", "snxsym"),
            dir.path().join("R (2).snxsym")
        );
    }
}
