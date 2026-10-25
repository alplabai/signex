//! The blocking half of a History restore (#859).
//!
//! `handle_history_restore_clicked` used to open the project repo, wait for
//! running saves, rewrite the file from the chosen revision and re-read it
//! inside `update`. All of that is now one job on a worker thread; it never
//! touches `Signex` state and hands the re-read document back as plain data
//! for the completion handler to install.

use std::path::{Path, PathBuf};

use signex_types::pcb::PcbBoard;
use signex_types::schematic::SchematicSheet;

use crate::persist::SaveBarrier;

/// Fingerprint of a document's in-memory content when a restore started,
/// compared again when it finishes (see `restore_fingerprint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditStamp(pub u64);

/// What the tab being restored holds, which decides how the restored file
/// is read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreKind {
    Schematic,
    Pcb,
    Footprint,
    Symbol,
    /// Library tabs reload from their mounted adapter on the UI side.
    Library,
}

/// The restored file, read back and parsed.
#[derive(Debug, Clone)]
pub enum RestoredDocument {
    Schematic(Box<SchematicSheet>),
    Pcb(Box<PcbBoard>),
    Footprint(Box<signex_library::FootprintFile>),
    Symbol(Box<signex_library::SymbolFile>),
    Library,
}

/// How a restore failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreFailure {
    /// The revision was not written back; the text is the error card's.
    NotRestored(String),
    /// The file was restored but could not be loaded back into the tab.
    /// `unreadable` is true for an IO error, where reopening can help.
    NotReloaded { why: String, unreadable: bool },
}

/// Everything the worker needs, as owned data.
pub struct RestoreJob {
    pub project_root: PathBuf,
    pub rel_path: PathBuf,
    pub full_path: PathBuf,
    pub sha: String,
    pub kind: RestoreKind,
    pub saves: SaveBarrier,
}

impl RestoreJob {
    pub fn run(self) -> Result<Box<RestoredDocument>, RestoreFailure> {
        let adapter =
            signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
                self.project_root.clone(),
            )
            .map_err(|e| {
                RestoreFailure::NotRestored(format!(
                    "Could not open the version-control repository at {}: {e}",
                    self.project_root.display()
                ))
            })?;
        // A write still running for this file would land after the restore
        // and put the pre-restore content back on disk; let it finish first
        // (same bounded wait as Save All).
        self.saves.wait();
        adapter
            .restore_at_from_sha(&self.rel_path, &self.sha)
            .map_err(|e| {
                RestoreFailure::NotRestored(format!(
                    "Could not restore {} to {}: {e}",
                    self.rel_path.display(),
                    self.sha
                ))
            })?;
        read_back(self.kind, &self.full_path).map(Box::new)
    }
}

fn unreadable(e: impl std::fmt::Display) -> RestoreFailure {
    RestoreFailure::NotReloaded {
        why: format!("could not be read: {e}"),
        unreadable: true,
    }
}

fn unparsable(e: impl std::fmt::Display) -> RestoreFailure {
    RestoreFailure::NotReloaded {
        why: format!("could not be parsed: {e}"),
        unreadable: false,
    }
}

fn empty(what: &str) -> RestoreFailure {
    RestoreFailure::NotReloaded {
        why: format!("contains zero {what}"),
        unreadable: false,
    }
}

pub(crate) fn read_back(
    kind: RestoreKind,
    path: &Path,
) -> Result<RestoredDocument, RestoreFailure> {
    match kind {
        RestoreKind::Schematic => {
            let text = std::fs::read_to_string(path).map_err(unreadable)?;
            let parsed = signex_types::format::SnxSchematic::parse(&text).map_err(unparsable)?;
            Ok(RestoredDocument::Schematic(Box::new(parsed.sheet)))
        }
        RestoreKind::Pcb => {
            let text = std::fs::read_to_string(path).map_err(unreadable)?;
            let parsed = signex_types::format::SnxPcb::parse(&text).map_err(unparsable)?;
            Ok(RestoredDocument::Pcb(Box::new(parsed.board)))
        }
        RestoreKind::Footprint => {
            let text = std::fs::read_to_string(path).map_err(unreadable)?;
            let file = signex_library::FootprintFile::from_toml_str(&text).map_err(unparsable)?;
            if file.footprints.is_empty() {
                return Err(empty("footprints"));
            }
            Ok(RestoredDocument::Footprint(Box::new(file)))
        }
        RestoreKind::Symbol => {
            let bytes = std::fs::read(path).map_err(unreadable)?;
            let file = signex_library::SymbolFile::from_bytes(&bytes).map_err(unparsable)?;
            if file.symbols.is_empty() {
                return Err(empty("symbols"));
            }
            Ok(RestoredDocument::Symbol(Box::new(file)))
        }
        RestoreKind::Library => Ok(RestoredDocument::Library),
    }
}
