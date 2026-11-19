//! Design Notebook annotations (#1306, Annotations panel #165).
//!
//! A notebook is a `.snxnbk` file in the project's `notebook/` folder:
//! Typst source with an optional TOML front matter between two `+++` lines.
//! An annotation is a block written
//! `#annotation(kind: "issue", status: "open", title: "..", date: "2026-11-17")[ body ]`;
//! its body may name parts, nets and pins with `#component("U3")`,
//! `#net("VBUS")` and `#pin("U1", "3")`. [`scan`] lists the blocks of one
//! source and every problem it met, by line; [`Filter`] picks the ones a
//! list shows. The grammar and the reading rules are in
//! `docs/NOTEBOOK_ANNOTATIONS.md`.
//!
//! UI-free and IO-free: the caller reads the files.

mod annotation;
mod date;
mod filter;
mod lexer;
mod reference;
mod scan;

pub use annotation::{Annotation, AnnotationKind, Status};
pub use date::IsoDate;
pub use filter::{DateWindow, Filter, newest_first};
pub use reference::Reference;
pub use scan::{ProblemKind, SNIPPET_MAX_CHARS, ScanProblem, TITLE_MAX_CHARS, scan};

/// The project folder that holds the notebooks.
pub const NOTEBOOK_DIR: &str = "notebook";
/// The extension of a notebook file.
pub const NOTEBOOK_EXTENSION: &str = "snxnbk";

#[cfg(test)]
mod tests;
