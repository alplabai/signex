//! Reads fabrication data back (#2539): Gerber RS-274X with X2 attributes,
//! Excellon drill files and the Gerber job file, as the Gerber viewer and
//! its compare-with-board show them.
//!
//! A reader takes a file name and its bytes; it opens no file and reads no
//! clock, so the viewer, the command line and the tests read the same way.
//! An image is objects in integer nm in the file's own frame (x right, y
//! up, from the file origin), which for a file Signex wrote is the board
//! origin, the frame `signex_output::fab` writes in.
//!
//! Nothing is dropped: every command not understood and every object that
//! cannot be built is a [`ReadProblem`] with its file and line, and an
//! object without a shape is kept (empty `shapes`) so the viewer can mark
//! where it is. See docs/GERBER_VIEWER.md.

mod number;

pub mod compare;
pub mod excellon;
pub mod file_function;
pub mod gerber;
pub mod image;
pub mod job;
pub mod package;
pub mod problem;
pub mod source;

pub use compare::{
    BoardOverlay, CompareOptions, Comparison, Difference, DifferenceKind, LayerMatch,
    compare_with_board,
};
pub use excellon::read_excellon;
pub use file_function::{CopperSide, DrillKind, FileFunction, Side};
pub use gerber::read_gerber;
pub use image::{
    ApertureDef, FilePolarity, Image, Object, ObjectAttributes, ObjectKind, Polarity, Units,
};
pub use job::{JobEntry, JobFile, read_job};
pub use package::{NameSource, Package, PackageLayer, assemble};
pub use problem::{ProblemKind, ReadProblem, Severity};
pub use source::{FileKind, ReadFile, SourceLine, read_file, sniff_kind};
