//! The Design Notebook (#2795; mockup rows 195-202, row 60b), and its
//! annotations (#2679, Annotations panel: mockup row 165).
//!
//! A project's notebook is one `.snxnbk` file in its `notebook/` folder
//! that may `#include` chapter files. A `.snxnbk` file is Typst source
//! after a TOML header between two `+++` lines; the header holds the format
//! token and the stored id of each reference, and there is no sidecar file.
//! [`NotebookFile`] reads and writes one file byte for byte.
//!
//! The Typst names the design with `#component("R7")`, `#pin("U3", "3")`
//! (or `#pin("U3.3")`) and `#net("+3V3")` ([`Reference`]) and records
//! blocks with one function per kind, `#note`, `#calculation`,
//! `#measurement`, `#issue` and `#decision`, each with a typed id
//! ([`BlockId`]: `NOTE-012`, `CALC-2.1`, `MEAS-009`, `ISS-004`,
//! `DEC-002`). The earlier `#annotation(kind: "...", ...)` block is still
//! read: [`scan`] lists its blocks and every problem it met, by line, and
//! [`Filter`] picks the ones a list shows (`docs/NOTEBOOK_ANNOTATIONS.md`).
//!
//! [`compile`] turns [`NotebookSources`] into pages over Typst, with the
//! bundled `@signex/notebook` package and fonts only; a
//! [`CompiledNotebook`] draws a page for the preview and writes a PDF that
//! is the same file every time.
//!
//! [`resolve_references`] checks a file's references against a
//! [`DesignIndex`] the caller builds from the project and tells the stale
//! ones apart (renamed, deleted, missing pin), each with its fixes
//! ([`apply_fix`]); [`reference_card`] is a reference's hover card,
//! [`picker_entries`] the `@` picker and [`badges`] the schematic's
//! Notebook badges (mockup rows 196 and 197).
//!
//! UI-free and IO-free: the caller reads the files.

mod annotation;
mod badge;
mod card;
mod compile;
mod date;
mod design;
mod design_read;
mod file;
mod filter;
mod fix;
mod header;
mod id;
mod json;
mod lexer;
mod picker;
mod reference;
mod resolution;
mod scan;
mod sources;
mod syntax;
mod world;

pub use annotation::{Annotation, AnnotationKind, Status};
pub use badge::{Badge, BadgeTarget, BadgeTone, BlockMention, badge_of, badges, worst_tone};
pub use card::{
    CONTEXT_ACTIONS, CardAction, CardFacts, ReferenceCard, last_measurement, reference_card,
};
pub use compile::{
    Compilation, CompileOptions, CompiledNotebook, Diagnostic, DiagnosticSeverity, PDF_CREATOR,
    PageImage, PageInfo, compile,
};
pub use date::IsoDate;
pub use design::{DesignIndex, DesignNet, DesignPart, DesignPin, PinName, natural_order};
pub use design_read::{PlacedSheet, design_index};
pub use file::{HEADER_FENCE, NotebookFile, NotebookFileError};
pub use filter::{DateWindow, Filter, newest_first};
pub use fix::{
    Fix, FixError, FixOffer, Fixed, PlannedFix, apply_fix, apply_plan, markup_text, plan_update_all,
};
pub use header::{Header, HeaderError, NOTEBOOK_FORMAT, StoredReference};
pub use id::{BlockId, SERIAL_WIDTH};
pub use picker::{PICKER_LIMIT, PickerEntry, picker_entries};
pub use reference::{Reference, quote};
pub use resolution::{
    ReferenceReport, ReferenceState, ResolvedReference, resolve_one, resolve_references,
};
pub use scan::{ProblemKind, SNIPPET_MAX_CHARS, ScanProblem, TITLE_MAX_CHARS, scan};
pub use sources::{LoadProblem, LoadProblemKind, NotebookSources, resolve};
pub use syntax::{Include, ReferenceAt, distinct_references, includes, references, typst_source};
pub use world::{PACKAGE_IMPORT, PACKAGE_NAME, PACKAGE_NAMESPACE, PACKAGE_VERSION};

/// The project folder that holds the notebooks.
pub const NOTEBOOK_DIR: &str = "notebook";
/// The extension of a notebook file.
pub const NOTEBOOK_EXTENSION: &str = "snxnbk";
/// The notebook file a project's Design Notebook opens: `design.snxnbk`
/// in [`NOTEBOOK_DIR`].
pub const DEFAULT_NOTEBOOK_FILE: &str = "design.snxnbk";

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_badges;
#[cfg(test)]
mod tests_compile;
#[cfg(test)]
mod tests_design;
#[cfg(test)]
mod tests_file;
#[cfg(test)]
mod tests_model;
#[cfg(test)]
mod tests_references;
