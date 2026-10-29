//! Opening a stock symbol or footprint edits a copy in the user's library
//! (#813).
//!
//! The stock library mounts read-only, so its primitives can never be
//! edited in place. Every way of opening one for editing (the Components
//! panel, the browser, Edit Symbol on a placed part, the project tree)
//! reaches [`crate::app::Signex::open_primitive`], which diverts a stock
//! primitive here: the user picks (once) which writable library receives
//! copies, the copy is written off the UI thread, and the editor opens on
//! the copy. The stock file is only ever read.
//!
//! * [`StockCopyState`]: the "Copy to Library" dialog, held in
//!   `LibraryState::stock_copy` while it is open.
//! * [`StockCopyMsg`]: its past-tense events.
//! * [`job`]: the blocking copy, run on a worker.
//! * [`view`]: the dialog, a pure read of the state.

use std::path::PathBuf;

pub mod job;
mod view;

#[cfg(test)]
mod tests;

pub use view::view;

/// Which primitive kind a stock file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockPrimitiveKind {
    Symbol,
    Footprint,
}

impl StockPrimitiveKind {
    /// The kind of the primitive file at `path`, from its extension.
    pub fn of_path(path: &std::path::Path) -> Option<Self> {
        match path
            .extension()
            .and_then(|e| e.to_str())?
            .to_ascii_lowercase()
            .as_str()
        {
            "snxsym" => Some(Self::Symbol),
            "snxfpt" => Some(Self::Footprint),
            _ => None,
        }
    }

    /// The library sub-directory holding files of this kind.
    pub fn directory(self) -> &'static str {
        match self {
            Self::Symbol => "symbols",
            Self::Footprint => "footprints",
        }
    }

    /// The file extension of this kind.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Symbol => "snxsym",
            Self::Footprint => "snxfpt",
        }
    }

    /// Lower-case noun for messages.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Footprint => "footprint",
        }
    }
}

/// The open "Copy to Library" dialog: which stock primitive the user asked
/// to edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockCopyState {
    /// The stock file that was asked for. Never written.
    pub source: PathBuf,
    pub kind: StockPrimitiveKind,
    /// What the primitive is called in the dialog text.
    pub label: String,
    /// Set when the library remembered from an earlier copy can no longer
    /// receive one (not mounted, or read-only), so the dialog says why it
    /// asks again.
    pub previous_unavailable: Option<PathBuf>,
    /// The last attempt's failure, shown in the dialog.
    pub error: Option<String>,
}

/// Where the copy goes.
#[derive(Debug, Clone)]
pub enum CopyTarget {
    /// A mounted writable library, by its `.snxlib` path.
    Existing(PathBuf),
    /// A library to create first, at this `.snxlib` path.
    New(PathBuf),
}

/// Events of the "Copy to Library" dialog and its copy.
#[derive(Debug, Clone)]
pub enum StockCopyMsg {
    /// The user chose a mounted writable library in the dialog.
    TargetPicked(PathBuf),
    /// The user asked to create a new library to copy into.
    NewLibraryRequested,
    /// The save dialog for the new library closed; `None` = dismissed.
    NewLibraryPathChosen(Option<PathBuf>),
    /// The user dismissed the dialog (button, Esc or window close).
    Cancelled,
    /// The copy finished on its worker.
    CopyFinished(job::StockCopyOutcome),
}
