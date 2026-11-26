//! What the report is built from, gathered by the caller: the project file,
//! the hand-written notebook's blocks, the check runs, the BOM and the
//! records of simulation, outputs and changes. Each source is a [`Fact`]:
//! read, with where it came from, or not read, with why. Nothing is guessed.

use signex_types::check::CheckReport;
use signex_types::project::RequirementEntry;

use crate::annotation::Annotation;
use crate::date::IsoDate;

/// One source of the report: read from somewhere, or not read and why.
#[derive(Debug, Clone, PartialEq)]
pub enum Fact<T> {
    Read {
        value: T,
        /// Where it came from, as the answer names it ("design.snxnbk, 21
        /// blocks", "check_erc run 3").
        from: String,
    },
    NotRead {
        why: String,
    },
}

impl<T> Fact<T> {
    pub fn read(value: T, from: impl Into<String>) -> Self {
        Fact::Read {
            value,
            from: from.into(),
        }
    }

    pub fn not_read(why: impl Into<String>) -> Self {
        Fact::NotRead { why: why.into() }
    }

    /// The value, when it was read.
    pub fn value(&self) -> Option<&T> {
        match self {
            Fact::Read { value, .. } => Some(value),
            Fact::NotRead { .. } => None,
        }
    }

    /// Where the value came from, when it was read.
    pub fn from(&self) -> Option<&str> {
        match self {
            Fact::Read { from, .. } => Some(from),
            Fact::NotRead { .. } => None,
        }
    }

    /// Why it was not read, when it was not.
    pub fn why_not(&self) -> Option<&str> {
        match self {
            Fact::Read { .. } => None,
            Fact::NotRead { why } => Some(why),
        }
    }
}

/// Everything a report is built from.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectFacts {
    /// The project file's name (`buck-3v3.snxprj`).
    pub project_file: String,
    /// The project's name (`buck-3v3`).
    pub name: String,
    /// The project's revision, when its parameters name one.
    pub revision: Option<String>,
    /// The variant the BOM was built for (`Base`).
    pub variant: String,
    /// The project's sheets, root first, each with its depth in the
    /// hierarchy.
    pub sheets: Vec<SheetFact>,
    /// The power nets of the design, by name, sorted.
    pub power_nets: Vec<String>,
    /// The project's requirements list, as stored.
    pub requirements: Vec<RequirementEntry>,
    /// The blocks of the hand-written notebook (`design.snxnbk` and its
    /// chapters).
    pub blocks: Fact<Vec<Annotation>>,
    pub requirement_check: Fact<CheckReport>,
    pub erc: Fact<CheckReport>,
    pub drc: Fact<CheckReport>,
    pub bom: Fact<BomFacts>,
    pub simulation: Fact<Vec<SimulationRun>>,
    pub outputs: Fact<Vec<OutputFile>>,
    pub changes: Fact<Vec<ChangeEntry>>,
    /// References of the hand-written notebook whose part, pin or net is
    /// gone or renamed.
    pub stale_references: usize,
}

impl ProjectFacts {
    /// The notebook's blocks, none when they were not read.
    pub fn block_list(&self) -> &[Annotation] {
        self.blocks.value().map_or(&[], Vec::as_slice)
    }
}

/// One sheet of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetFact {
    pub name: String,
    /// The file, as the project lists it.
    pub file: String,
    /// 0 for the root sheet, 1 for its children, ...
    pub depth: usize,
    /// The parts placed on it.
    pub parts: usize,
}

/// The BOM as it is bought.
#[derive(Debug, Clone, PartialEq)]
pub struct BomFacts {
    /// Grouped lines.
    pub lines: usize,
    /// Fitted parts.
    pub parts: usize,
    /// The cost of one board, when every line is priced.
    pub cost: Option<BomCost>,
    /// Lines with no price, by their first designator.
    pub unpriced: Vec<String>,
    /// Lines with no stock, by their first designator.
    pub no_stock: Vec<String>,
}

/// What one board costs at a stated build quantity.
#[derive(Debug, Clone, PartialEq)]
pub struct BomCost {
    pub per_board: f64,
    pub quantity: u32,
    pub currency: String,
}

/// One recorded simulation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationRun {
    pub name: String,
    /// The analysis (`transient`, `ac`, ...).
    pub kind: String,
    /// Named results, each as recorded (`("Vout ripple", "18.0 mV")`).
    pub measurements: Vec<(String, String)>,
}

/// One file the last output run wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    /// The path inside the project folder.
    pub path: String,
    /// The SHA-256 of its bytes, in lower-case hex.
    pub sha256: String,
}

/// One entry of the project's version history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeEntry {
    pub date: Option<IsoDate>,
    /// The revision or version it made.
    pub revision: String,
    pub summary: String,
}
