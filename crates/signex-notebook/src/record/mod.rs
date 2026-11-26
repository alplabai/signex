//! The review record of an annotation block (mockup row 198): the fields
//! each kind prints under its title, read by one function per kind. A Note
//! has an author; a Calculation a target band, a result and its inputs; a
//! Measurement its rows with their verdict, the instrument, conditions and
//! operator; an Issue an owner, severity, due revision and the block it
//! came from; a Decision an owner, review, the chosen option, the
//! alternatives with why each was dropped, what it supersedes and who
//! approved it. Field names are in `docs/NOTEBOOK_ANNOTATIONS.md`,
//! "Review records".

mod kinds;
mod measurement;
pub(crate) mod value;

use crate::annotation::AnnotationKind;
use crate::id::BlockId;
use crate::reference::Reference;
use value::FieldValue;

pub use measurement::{Limits, MeasurementRow, VERDICT_WARN_MARGIN, Verdict, verdict_of};

/// The record fields of one block, by its kind.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Record {
    Note(NoteRecord),
    Calculation(CalculationRecord),
    Measurement(MeasurementRecord),
    Issue(IssueRecord),
    Decision(DecisionRecord),
}

/// A Note: free text with references; no status.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct NoteRecord {
    pub author: Option<String>,
}

/// A Calculation: what it must meet and what it gave.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct CalculationRecord {
    /// The band the result must fall in: `target: (3.30, 2%)` or
    /// `limit: (max: 125)`.
    pub limits: Option<Limits>,
    /// The nominal value of a `target:` band.
    pub nominal: Option<f64>,
    pub result: Option<f64>,
    pub unit: Option<String>,
    pub requirements: Vec<String>,
    /// The blocks whose values it reads (`inputs: ("MEAS-011",)`).
    pub inputs: Vec<BlockId>,
    /// Pass, Warn or Fail of the result against the band.
    pub verdict: Option<Verdict>,
}

/// A Measurement: one row per quantity measured.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct MeasurementRecord {
    pub rows: Vec<MeasurementRow>,
    pub unit: Option<String>,
    pub conditions: Option<String>,
    pub instrument: Option<String>,
    pub operator: Option<String>,
    pub requirements: Vec<String>,
    /// The worst verdict of the rows; `None` without rows.
    pub verdict: Option<Verdict>,
    /// The board revision a reading was taken on (`rev: "B"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    /// The board a reading was taken on (`board: "SN 0007"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
}

impl MeasurementRecord {
    /// How many rows pass, warn and fail: the "2 pass · 1 warn" of the
    /// block's header.
    pub fn counts(&self) -> [(Verdict, usize); 3] {
        Verdict::ALL.map(|v| {
            let count = self
                .rows
                .iter()
                .filter(|row| row.verdict == Some(v))
                .count();
            (v, count)
        })
    }
}

/// How serious an Issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Minor,
    Major,
    Critical,
}

impl Severity {
    pub const ALL: [Severity; 3] = [Severity::Minor, Severity::Major, Severity::Critical];

    /// The token written in the source: `minor`, `major`, `critical`.
    pub fn token(self) -> &'static str {
        match self {
            Severity::Minor => "minor",
            Severity::Major => "major",
            Severity::Critical => "critical",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.token() == token)
    }
}

/// An Issue: who owns it, how serious it is and where it came from.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct IssueRecord {
    pub owner: Option<String>,
    pub severity: Option<Severity>,
    /// The revision it is due in: `rev C`.
    pub due: Option<String>,
    /// The block that raised it: `from: "MEAS-010"`.
    pub from: Option<BlockId>,
    pub requirements: Vec<String>,
    /// What closes it, the footer line.
    pub closes_when: Option<String>,
}

/// One option a Decision did not take, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Alternative {
    pub option: String,
    pub reason: Option<String>,
}

/// A Decision: what was chosen over what, by whom, in which review.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct DecisionRecord {
    pub owner: Option<String>,
    /// The review it was taken in: `R-03`.
    pub review: Option<String>,
    pub requirements: Vec<String>,
    pub chosen: Option<String>,
    pub alternatives: Vec<Alternative>,
    pub supersedes: Option<BlockId>,
    pub approver: Option<String>,
}

impl Record {
    /// A record of `kind` with no field set: the earlier `#annotation`
    /// form carries none.
    pub fn empty(kind: AnnotationKind) -> Self {
        match kind {
            AnnotationKind::Note => Record::Note(NoteRecord::default()),
            AnnotationKind::Calculation => Record::Calculation(CalculationRecord::default()),
            AnnotationKind::Measurement => Record::Measurement(MeasurementRecord::default()),
            AnnotationKind::Issue => Record::Issue(IssueRecord::default()),
            AnnotationKind::Decision => Record::Decision(DecisionRecord::default()),
        }
    }

    /// The verdict a Calculation or Measurement printed: Pass, Warn or
    /// Fail.
    pub fn verdict(&self) -> Option<Verdict> {
        match self {
            Record::Calculation(record) => record.verdict,
            Record::Measurement(record) => record.verdict,
            _ => None,
        }
    }

    /// The blocks this record names: an Issue's `from`, a Decision's
    /// `supersedes`, a Calculation's `inputs`.
    pub fn cites(&self) -> Vec<(Citation, &BlockId)> {
        match self {
            Record::Issue(record) => record.from.iter().map(|id| (Citation::From, id)).collect(),
            Record::Decision(record) => record
                .supersedes
                .iter()
                .map(|id| (Citation::Supersedes, id))
                .collect(),
            Record::Calculation(record) => record
                .inputs
                .iter()
                .map(|id| (Citation::Input, id))
                .collect(),
            Record::Note(_) | Record::Measurement(_) => Vec::new(),
        }
    }
}

/// How one block names another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Citation {
    /// An Issue raised by the block (`from:`); read the other way, the
    /// block "opens" the Issue.
    From,
    /// A Decision that replaces the block.
    Supersedes,
    /// A Calculation that reads the block's value.
    Input,
}

/// What reading a field met: the argument and why it was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FieldProblem {
    /// The kind has no such field.
    Unknown { argument: String },
    /// The field's value is not one it takes.
    Bad { argument: String, value: String },
}

/// The fields of a block other than `id`, `title`, `status` and `date`,
/// in source order, with their raw text.
pub(crate) struct Fields {
    entries: Vec<(String, FieldValue, String)>,
}

impl Fields {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, name: String, raw: &str) {
        self.entries
            .push((name, FieldValue::read(raw), raw.trim().to_string()));
    }

    /// Every reference the fields make: `target:` first, then the others
    /// in source order.
    pub(crate) fn references(&self) -> Vec<Reference> {
        let (target, rest): (Vec<_>, Vec<_>) = self
            .entries
            .iter()
            .partition(|(name, _, _)| name == "target");
        target
            .into_iter()
            .chain(rest)
            .flat_map(|(_, value, _)| value.references())
            .collect()
    }
}

/// The record of a block of `kind` from its `fields`, one reader per kind;
/// what was not taken goes to `problems`.
pub(crate) fn read_record(
    kind: AnnotationKind,
    fields: &Fields,
    problems: &mut Vec<FieldProblem>,
) -> Record {
    let mut reader = kinds::Reader::new(fields, problems);
    let record = match kind {
        AnnotationKind::Note => Record::Note(kinds::read_note(&mut reader)),
        AnnotationKind::Calculation => Record::Calculation(kinds::read_calculation(&mut reader)),
        AnnotationKind::Measurement => Record::Measurement(kinds::read_measurement(&mut reader)),
        AnnotationKind::Issue => Record::Issue(kinds::read_issue(&mut reader)),
        AnnotationKind::Decision => Record::Decision(kinds::read_decision(&mut reader)),
    };
    reader.finish();
    record
}
