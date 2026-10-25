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

/// Paths with a read in flight. A *tracked* claim (from [`Self::begin`])
/// carries a generation its completion must present; an *untracked* one (from
/// [`Self::insert`]: File > Open and project opens) only dedupes and is
/// cleared by its own completion.
#[derive(Debug, Default, Clone)]
pub struct InFlightReads {
    next_generation: u64,
    claims: HashMap<PathBuf, Claim>,
}

#[derive(Debug, Clone, Copy)]
struct Claim {
    generation: u64,
    tracked: bool,
    /// Set by the `cancel_*_retaining` methods: the work is abandoned but
    /// its worker still runs, so the claim stays until the worker reports.
    cancelled: bool,
}

/// How a finished worker relates to the claim it started under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finished {
    /// Still the tracked request: apply the result.
    Current,
    /// Abandoned while it ran, claim held until now: drop the result.
    Cancelled,
    /// Not the tracked request (superseded or its claim already dropped).
    Stale,
}

impl InFlightReads {
    pub fn contains(&self, path: &Path) -> bool {
        self.claims.contains_key(path)
    }

    fn claim(&mut self, path: PathBuf, tracked: bool) -> Option<u64> {
        if self.claims.contains_key(&path) {
            return None;
        }
        self.next_generation += 1;
        let generation = self.next_generation;
        self.claims.insert(
            path,
            Claim {
                generation,
                tracked,
                cancelled: false,
            },
        );
        Some(generation)
    }

    /// Start a tracked claim on `path`. `None` when a read for it is already
    /// in flight, so the caller must not spawn a second one.
    pub fn begin(&mut self, path: PathBuf) -> Option<u64> {
        self.claim(path, true)
    }

    /// An untracked claim: dedupes only, never cancelled, cleared by
    /// [`Self::remove_untracked`].
    pub fn insert(&mut self, path: PathBuf) -> bool {
        self.claim(path, false).is_some()
    }

    /// Clear an untracked claim. A tracked claim for the same path belongs to
    /// a newer request and is left alone.
    pub fn remove_untracked(&mut self, path: &Path) {
        if self.claims.get(path).is_some_and(|c| !c.tracked) {
            self.claims.remove(path);
        }
    }

    /// Consume the claim of a finished read. `true` only when `generation` is
    /// still the one tracked for `path`; anything else is a stale completion
    /// that must not be applied.
    pub fn finish(&mut self, path: &Path, generation: u64) -> bool {
        self.finish_claim(path, generation) == Finished::Current
    }

    /// Consume the claim of a finished worker, telling a live request from
    /// one cancelled with a `cancel_*_retaining` method (whose claim kept
    /// refusing saves until now).
    pub fn finish_claim(&mut self, path: &Path, generation: u64) -> Finished {
        match self.claims.get(path) {
            Some(c) if c.tracked && c.generation == generation => {
                let cancelled = c.cancelled;
                self.claims.remove(path);
                if cancelled {
                    Finished::Cancelled
                } else {
                    Finished::Current
                }
            }
            _ => Finished::Stale,
        }
    }

    /// Abandon the tracked claim on `path` but keep it until its worker
    /// finishes: the worker may still write the file, so the path stays
    /// claimed (and `contains` stays true) meanwhile.
    pub fn cancel_retaining(&mut self, path: &Path) {
        if let Some(c) = self.claims.get_mut(path).filter(|c| c.tracked) {
            c.cancelled = true;
        }
    }

    /// [`Self::cancel_retaining`] for every tracked claim under `dir`.
    pub fn cancel_under_retaining(&mut self, dir: &Path) {
        for (path, c) in &mut self.claims {
            if c.tracked && path.starts_with(dir) {
                c.cancelled = true;
            }
        }
    }

    /// [`Self::cancel_retaining`] for every tracked claim.
    pub fn cancel_all_retaining(&mut self) {
        for c in self.claims.values_mut().filter(|c| c.tracked) {
            c.cancelled = true;
        }
    }

    /// Drop the tracked claim on `path`, so its completion turns stale.
    pub fn cancel(&mut self, path: &Path) {
        if self.claims.get(path).is_some_and(|c| c.tracked) {
            self.claims.remove(path);
        }
    }

    /// Cancel every tracked claim under `dir`.
    pub fn cancel_under(&mut self, dir: &Path) {
        self.claims
            .retain(|path, c| !c.tracked || !path.starts_with(dir));
    }

    /// Cancel every tracked claim.
    pub fn cancel_all(&mut self) {
        self.claims.retain(|_, c| !c.tracked);
    }
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
    fn cancelling_leaves_untracked_claims_alone_and_they_never_clear_tracked_ones() {
        let mut reads = InFlightReads::default();
        reads.insert(PathBuf::from("/a/p.snxprj"));
        reads.begin(PathBuf::from("/a/x.snxsch"));
        reads.cancel_all();
        assert!(reads.contains(Path::new("/a/p.snxprj")));
        assert!(!reads.contains(Path::new("/a/x.snxsch")));
        let generation = reads.begin(PathBuf::from("/a/y.snxsch")).expect("tracked");
        reads.remove_untracked(Path::new("/a/y.snxsch"));
        assert!(reads.finish(Path::new("/a/y.snxsch"), generation));
        reads.remove_untracked(Path::new("/a/p.snxprj"));
        assert!(!reads.contains(Path::new("/a/p.snxprj")));
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

    #[test]
    fn a_retained_cancel_keeps_the_claim_until_its_worker_finishes() {
        let mut reads = InFlightReads::default();
        let path = PathBuf::from("/a/x.snxsch");
        let generation = reads.begin(path.clone()).expect("tracked");
        reads.cancel_retaining(&path);
        assert!(reads.contains(&path), "still claimed");
        assert!(reads.begin(path.clone()).is_none(), "no second worker");
        assert_eq!(reads.finish_claim(&path, generation), Finished::Cancelled);
        assert!(!reads.contains(&path));
        assert_eq!(reads.finish_claim(&path, generation), Finished::Stale);
    }

    #[test]
    fn retained_cancels_by_directory_and_all_leave_the_rest_live() {
        let mut reads = InFlightReads::default();
        let a = reads.begin(PathBuf::from("/a/x")).expect("a");
        let b = reads.begin(PathBuf::from("/b/y")).expect("b");
        reads.cancel_under_retaining(Path::new("/a"));
        assert_eq!(
            reads.finish_claim(Path::new("/a/x"), a),
            Finished::Cancelled
        );
        let c = reads.begin(PathBuf::from("/c/z")).expect("c");
        reads.cancel_all_retaining();
        assert_eq!(
            reads.finish_claim(Path::new("/b/y"), b),
            Finished::Cancelled
        );
        assert_eq!(
            reads.finish_claim(Path::new("/c/z"), c),
            Finished::Cancelled
        );
    }
}
