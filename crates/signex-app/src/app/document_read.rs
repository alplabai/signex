//! Off-thread document reads (#859).
//!
//! Opening a schematic, PCB or primitive from the project tree used to read
//! and parse the file inside `update`. The read is now a `Task`; this module
//! holds the plain data that crosses the thread boundary and the blocking
//! jobs themselves, none of which touch `Signex` state.
//!
//! A request is identified by its path plus a generation number from
//! [`InFlightReads`]. The completion handler applies a result only while the
//! tracker still holds that exact pair; closing the project, re-requesting the
//! path or finishing the read already makes any other completion stale.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use signex_types::pcb::PcbBoard;
use signex_types::schematic::SchematicSheet;

/// Paths with a read in flight, each with the generation that owns it.
#[derive(Debug, Default, Clone)]
pub struct InFlightReads {
    next_generation: u64,
    tokens: HashMap<PathBuf, u64>,
}

impl InFlightReads {
    pub fn contains(&self, path: &Path) -> bool {
        self.tokens.contains_key(path)
    }

    /// Start tracking `path`. `None` when a read for it is already in flight,
    /// so the caller must not spawn a second one.
    pub fn begin(&mut self, path: PathBuf) -> Option<u64> {
        if self.tokens.contains_key(&path) {
            return None;
        }
        self.next_generation += 1;
        self.tokens.insert(path, self.next_generation);
        Some(self.next_generation)
    }

    /// [`Self::begin`] for callers that do not need the generation.
    pub fn insert(&mut self, path: PathBuf) -> bool {
        self.begin(path).is_some()
    }

    pub fn remove(&mut self, path: &Path) {
        self.tokens.remove(path);
    }

    /// Consume the claim of a finished read. `true` only when `generation` is
    /// still the one tracked for `path`; anything else is a stale completion
    /// that must not be applied.
    pub fn finish(&mut self, path: &Path, generation: u64) -> bool {
        if self.tokens.get(path) != Some(&generation) {
            return false;
        }
        self.tokens.remove(path);
        true
    }

    /// Forget every document read under `dir`, so their completions turn
    /// stale. A `.snxprj` claim is left alone: project opens share this
    /// tracker and own their completion.
    pub fn cancel_under(&mut self, dir: &Path) {
        self.tokens
            .retain(|path, _| !path.starts_with(dir) || is_project_file(path));
    }

    /// [`Self::cancel_under`] for every directory.
    pub fn cancel_all(&mut self) {
        self.tokens.retain(|path, _| is_project_file(path));
    }
}

fn is_project_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "snxprj")
}

/// What a read completion is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadRequest {
    pub path: PathBuf,
    pub generation: u64,
}

/// A document read and parsed on a worker thread.
#[derive(Debug, Clone)]
pub enum LoadedDocument {
    Schematic {
        title: String,
        sheet: Box<SchematicSheet>,
    },
    Pcb {
        title: String,
        board: Box<PcbBoard>,
    },
    Symbol(Box<signex_library::SymbolFile>),
    Footprint(Box<signex_library::FootprintFile>),
}

/// Why a read failed, and how the UI reports it (the same split the
/// synchronous paths had).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenFailure {
    /// Logged to the Messages panel only: what a project-tree open of a
    /// schematic or PCB always did.
    LogOnly(String),
    /// Logged and raised as the error card: what opening a primitive did.
    Card(String),
}

/// The blocking part of an open.
#[derive(Debug, Clone)]
pub enum ReadJob {
    TreeSchematic {
        title: String,
    },
    TreePcb {
        title: String,
    },
    /// `from_tree` also checks the file exists first, with the project
    /// tree's own message.
    Symbol {
        from_tree: bool,
    },
    Footprint {
        from_tree: bool,
    },
}

impl ReadJob {
    /// Run on a blocking thread; never touches app state.
    pub fn run(self, path: &Path) -> Result<Box<LoadedDocument>, OpenFailure> {
        match self {
            Self::TreeSchematic { title } => {
                tree_checked(path).and_then(|()| read_schematic(path, title))
            }
            Self::TreePcb { title } => tree_checked(path).and_then(|()| read_pcb(path, title)),
            Self::Symbol { from_tree } => {
                if from_tree {
                    tree_checked(path)?;
                }
                read_symbol(path)
            }
            Self::Footprint { from_tree } => {
                if from_tree {
                    tree_checked(path)?;
                }
                read_footprint(path)
            }
        }
        .map(Box::new)
    }
}

fn tree_checked(path: &Path) -> Result<(), OpenFailure> {
    if path.exists() {
        return Ok(());
    }
    Err(OpenFailure::LogOnly(format!(
        "project tree file does not exist: {}",
        path.display()
    )))
}

fn read_schematic(path: &Path, title: String) -> Result<LoadedDocument, OpenFailure> {
    let sheet = std::fs::read_to_string(path)
        .with_context(|| format!("read schematic {}", path.display()))
        .and_then(|text| {
            signex_types::format::SnxSchematic::parse(&text)
                .with_context(|| format!("parse schematic {}", path.display()))
        })
        .map_err(|e| OpenFailure::LogOnly(format!("{e:#}")))?
        .sheet;
    Ok(LoadedDocument::Schematic {
        title,
        sheet: Box::new(sheet),
    })
}

fn read_pcb(path: &Path, title: String) -> Result<LoadedDocument, OpenFailure> {
    let board = std::fs::read_to_string(path)
        .with_context(|| format!("read pcb {}", path.display()))
        .and_then(|text| {
            signex_types::format::SnxPcb::parse(&text)
                .with_context(|| format!("parse pcb {}", path.display()))
        })
        .map_err(|e| OpenFailure::LogOnly(format!("{e:#}")))?
        .board;
    Ok(LoadedDocument::Pcb {
        title,
        board: Box::new(board),
    })
}

fn read_symbol(path: &Path) -> Result<LoadedDocument, OpenFailure> {
    let card = |why: String| OpenFailure::Card(why);
    let bytes =
        std::fs::read(path).map_err(|e| card(format!("could not read {}: {e}", path.display())))?;
    // v0.18.4 — auto-detect TOML vs legacy JSON.
    let file = signex_library::SymbolFile::from_bytes(&bytes)
        .map_err(|e| card(format!("could not parse {}: {e}", path.display())))?;
    // Reported, not ignored: a container that decodes but holds nothing is a
    // different problem from a corrupt one (#532).
    if file.symbols.is_empty() {
        return Err(card(format!("{} contains no symbols", path.display())));
    }
    Ok(LoadedDocument::Symbol(Box::new(file)))
}

fn read_footprint(path: &Path) -> Result<LoadedDocument, OpenFailure> {
    let card = |why: String| OpenFailure::Card(why);
    let text = std::fs::read_to_string(path)
        .map_err(|e| card(format!("could not read {}: {e}", path.display())))?;
    let file = signex_library::FootprintFile::from_toml_str(&text)
        .map_err(|e| card(format!("could not parse {}: {e}", path.display())))?;
    if file.footprints.is_empty() {
        return Err(card(format!("{} contains no footprints", path.display())));
    }
    Ok(LoadedDocument::Footprint(Box::new(file)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_begin_for_the_same_path_is_refused_until_it_finishes() {
        let mut reads = InFlightReads::default();
        let path = PathBuf::from("/p/a.snxsch");
        let first = reads.begin(path.clone()).expect("first");
        assert!(reads.begin(path.clone()).is_none());
        assert!(reads.finish(&path, first));
        assert!(reads.begin(path).is_some());
    }

    #[test]
    fn a_cancelled_read_cannot_finish_even_after_the_path_is_requested_again() {
        let mut reads = InFlightReads::default();
        let path = PathBuf::from("/p/a.snxsch");
        let old = reads.begin(path.clone()).expect("old");
        reads.cancel_under(Path::new("/p"));
        assert!(!reads.finish(&path, old), "cancelled");
        let old = reads.begin(path.clone()).expect("again");
        reads.cancel_all();
        let new = reads.begin(path.clone()).expect("new");
        assert!(!reads.finish(&path, old), "superseded");
        assert!(reads.finish(&path, new));
    }

    #[test]
    fn cancelling_leaves_project_open_claims_alone() {
        let mut reads = InFlightReads::default();
        reads.begin(PathBuf::from("/a/p.snxprj"));
        reads.begin(PathBuf::from("/a/x.snxsch"));
        reads.cancel_all();
        assert!(reads.contains(Path::new("/a/p.snxprj")));
        assert!(!reads.contains(Path::new("/a/x.snxsch")));
    }

    #[test]
    fn cancel_under_keeps_reads_outside_the_directory() {
        let mut reads = InFlightReads::default();
        reads.begin(PathBuf::from("/a/x.snxsch"));
        reads.begin(PathBuf::from("/b/y.snxsch"));
        reads.cancel_under(Path::new("/a"));
        assert!(!reads.contains(Path::new("/a/x.snxsch")));
        assert!(reads.contains(Path::new("/b/y.snxsch")));
    }
}
