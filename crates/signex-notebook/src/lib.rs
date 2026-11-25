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
//! Each block is a review record ([`Record`], mockup row 198): the fields
//! its kind prints, a Measurement's rows with their [`Verdict`], and the
//! ids it cites ([`links`]); a failing Measurement opens an Issue
//! ([`open_failing_measurements`]). [`new_block_source`] is what an insert
//! button writes.
//!
//! [`compile`] turns [`NotebookSources`] into pages over Typst, with the
//! bundled `@signex/notebook` package and fonts only; a
//! [`CompiledNotebook`] draws a page for the preview and writes a PDF that
//! is the same file every time.
//!
//! The notebook tab reads the rest from here: the [`outline`], Typst
//! [`Highlights`], the insert toolbar's text ([`insertion_text`]), the
//! reference picker's order ([`matching`]), the preview's page colours
//! ([`PageColours`]), the empty state's [`Template`]s and the compile
//! status ([`NotebookStatus`]).
//!
//! [`resolve_references`] checks a file's references against a
//! [`DesignIndex`] the caller builds from the project and tells the stale
//! ones apart (renamed, deleted, missing pin), each with its fixes
//! ([`apply_fix`]); [`reference_card`] is a reference's hover card,
//! [`picker_entries`] the `@` picker and [`badges`] the schematic's
//! Notebook badges (mockup rows 196 and 197).
//!
//! Bench work (mockup row 199): [`measurement`] reads the `#measurement`
//! readings with their verdict and builds the timeline of a net, part or
//! pin; [`attachment`] reads the `#attachment` figures, their stale marks
//! and the copy into `notebook/attachments/`; [`scope`] draws a scope CSV
//! as a vector plot. [`bench`] gathers them over a notebook's files and
//! [`report`] gives the command answers (`docs/NOTEBOOK_MEASUREMENTS.md`).
//!
//! UI-free and IO-free: the caller reads the files.

pub mod attachment;
pub mod bench;
pub mod edit;
pub mod measurement;
pub mod report;
pub mod scope;
pub mod si;

mod derived;
mod literal;
mod timestamp;

pub use timestamp::Timestamp;

mod annotation;
mod badge;
mod block;
mod card;
mod compile;
mod date;
mod design;
mod design_picker;
mod design_read;
mod file;
mod filter;
mod fix;
mod header;
mod highlight;
mod id;
mod insert;
mod json;
mod lexer;
mod links;
mod location;
mod outline;
mod picker;
mod preview;
mod record;
mod reference;
mod resolution;
mod scan;
mod sources;
mod status;
mod syntax;
mod template;
mod templates;
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
pub use design_picker::{PICKER_LIMIT, PickerEntry, picker_entries};
pub use design_read::{PlacedSheet, design_index};
pub use file::{HEADER_FENCE, NotebookFile, NotebookFileError};
pub use filter::{DateWindow, Filter, newest_first};
pub use fix::{
    Fix, FixError, FixOffer, Fixed, PlannedFix, apply_fix, apply_plan, markup_text, plan_update_all,
};
pub use header::{Header, HeaderError, NOTEBOOK_FORMAT, StoredReference};
pub use highlight::{Highlights, Token};
pub use id::{BlockId, SERIAL_WIDTH};
pub use insert::{Inserted, Insertion, insertion_text, next_id, numbers_within_section};
pub use links::{
    Link, cited_by, issue_for_failure, links, open_failing_measurements, unopened_failures,
};
pub use location::{is_notebook_file, notebook_path};
pub use outline::{
    BlockEntry, Figure, FigureKind, Outline, Section, SourceAt, outline, outline_of_text,
};
pub use picker::{Candidate, MAX_ROWS, matching};
pub use preview::PageColours;
pub use record::{
    Alternative, CalculationRecord, Citation, DecisionRecord, IssueRecord, Limits,
    MeasurementRecord, MeasurementRow, NoteRecord, Record, Severity, VERDICT_WARN_MARGIN, Verdict,
    verdict_of,
};
pub use reference::{Reference, quote};
pub use resolution::{
    ReferenceReport, ReferenceState, ResolvedReference, resolve_one, resolve_references,
};
pub use scan::{ProblemKind, SNIPPET_MAX_CHARS, ScanProblem, TITLE_MAX_CHARS, scan};
pub use sources::{LoadProblem, LoadProblemKind, NotebookSources, resolve};
pub use status::{BlockCounts, NotebookStatus, header_line_count};
pub use syntax::{Include, ReferenceAt, distinct_references, includes, references, typst_source};
pub use template::{new_block_source, next_block_id};
pub use templates::Template;
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
mod tests_attachment;
#[cfg(test)]
mod tests_badges;
#[cfg(test)]
mod tests_compile;
#[cfg(test)]
mod tests_design;
#[cfg(test)]
mod tests_file;
#[cfg(test)]
mod tests_measure_print;
#[cfg(test)]
mod tests_measurement;
#[cfg(test)]
mod tests_model;
#[cfg(test)]
mod tests_records;
#[cfg(test)]
mod tests_references;
#[cfg(test)]
mod tests_scope;
