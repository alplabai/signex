//! The answers the measurement commands give (`docs/COMMANDS.md`): every
//! reading with its verdict and stale mark, every attachment with its
//! state, and the timeline of one target, each with the counts the
//! notebook's status strip shows ("9 measurements · 8 pass · 1 fail",
//! "1 stale attachment").

use crate::attachment::{
    Attachment, AttachmentKind, AttachmentProblemKind, ValueChange, changes, reading_changes,
};
use crate::bench::{FileProblem, NotebookRecord, record};
use crate::id::BlockId;
use crate::measurement::{
    MeasurementProblemKind, Reading, Timeline, Verdict, error_text, quantities_of, quantity_text,
    timeline,
};
use crate::reference::Reference;
use crate::scope::{ScopePlot, parse_csv, plot};
use crate::sources::NotebookSources;

/// One reading as the table prints it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ReadingRow {
    #[serde(flatten)]
    pub reading: Reading,
    /// The table's columns: Quantity, Target, Tol., Measured, Error, Result.
    pub quantity_label: String,
    pub target_text: String,
    pub tolerance_text: String,
    pub measured_text: String,
    pub error_text: String,
    pub error: Option<f64>,
    pub verdict: Option<Verdict>,
    /// The part values changed since it was taken; empty when current.
    pub changes: Vec<ValueChange>,
    /// The reading that corrects it.
    pub superseded_by: Option<BlockId>,
}

/// An attachment and what Signex found of it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AttachmentState {
    #[serde(flatten)]
    pub attachment: Attachment,
    /// The file is in `notebook/attachments/`.
    pub present: bool,
    pub changes: Vec<ValueChange>,
    /// A scope capture as drawn; `None` for other kinds, or when it could
    /// not be read (`error`).
    pub plot: Option<ScopePlot>,
    pub error: Option<String>,
    /// The readings taken with it.
    pub readings: Vec<BlockId>,
}

impl AttachmentState {
    pub fn is_stale(&self) -> bool {
        !self.changes.is_empty()
    }
}

/// The counts of the status strip.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Summary {
    pub readings: usize,
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
    /// Readings with no value or no limit yet.
    pub unchecked: usize,
    pub stale_readings: usize,
    pub attachments: usize,
    pub stale_attachments: usize,
    pub missing_attachments: usize,
}

/// `notebook_measurements`: every reading, the counts and the problems.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MeasurementsReport {
    pub readings: Vec<ReadingRow>,
    pub summary: Summary,
    pub problems: Vec<FileProblem<MeasurementProblemKind>>,
}

/// `notebook_attachments`: every attachment, the counts and the problems.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AttachmentsReport {
    pub attachments: Vec<AttachmentState>,
    pub summary: Summary,
    pub problems: Vec<FileProblem<AttachmentProblemKind>>,
}

/// `notebook_timeline`: the timeline, the quantities of the target and
/// the list as CSV.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TimelineReport {
    pub timeline: Timeline,
    pub quantities: Vec<String>,
    pub csv: String,
}

/// The state of every attachment of `found`, scope captures drawn.
pub fn attachment_states(
    sources: &NotebookSources,
    found: &NotebookRecord,
) -> Vec<AttachmentState> {
    let values = sources.design_values();
    found
        .attachments
        .iter()
        .map(|attachment| {
            let bytes = sources.get(&attachment.path_in_notebook());
            let (plot, error) = match (attachment.kind, bytes) {
                (AttachmentKind::ScopeCsv, Some(bytes)) => {
                    match parse_csv(&String::from_utf8_lossy(bytes)) {
                        Ok(capture) => (Some(plot(&capture, &attachment.plot)), None),
                        Err(error) => (None, Some(error.to_string())),
                    }
                }
                _ => (None, None),
            };
            let readings = found
                .readings
                .iter()
                .filter(|r| r.attach == attachment.file)
                .filter_map(|r| r.id.clone())
                .collect();
            AttachmentState {
                present: bytes.is_some(),
                changes: changes(&attachment.taken_with, values),
                plot,
                error,
                readings,
                attachment: attachment.clone(),
            }
        })
        .collect()
}

/// The rows of every reading of `found`.
fn rows(found: &NotebookRecord, sources: &NotebookSources) -> Vec<ReadingRow> {
    let values = sources.design_values();
    found
        .readings
        .iter()
        .map(|reading| {
            let check = reading.check();
            let unit = reading.unit.as_str();
            ReadingRow {
                quantity_label: reading.quantity_label(),
                target_text: reading
                    .limit
                    .map(|l| l.target_text(unit))
                    .unwrap_or_default(),
                tolerance_text: reading
                    .limit
                    .map(|l| l.tolerance_text(unit))
                    .unwrap_or_default(),
                measured_text: quantity_text(&reading.value_text, unit),
                error_text: check
                    .and_then(|c| c.error)
                    .map(error_text)
                    .unwrap_or_default(),
                error: check.and_then(|c| c.error),
                verdict: check.map(|c| c.verdict),
                changes: reading_changes(reading, &found.attachments, values),
                superseded_by: found
                    .readings
                    .iter()
                    .find(|o| o.supersedes.is_some() && o.supersedes == reading.id)
                    .and_then(|o| o.id.clone()),
                reading: reading.clone(),
            }
        })
        .collect()
}

fn summary(rows: &[ReadingRow], states: &[AttachmentState]) -> Summary {
    let count = |verdict| rows.iter().filter(|r| r.verdict == Some(verdict)).count();
    Summary {
        readings: rows.len(),
        pass: count(Verdict::Pass),
        warn: count(Verdict::Warn),
        fail: count(Verdict::Fail),
        unchecked: rows.iter().filter(|r| r.verdict.is_none()).count(),
        stale_readings: rows.iter().filter(|r| !r.changes.is_empty()).count(),
        attachments: states.len(),
        stale_attachments: states.iter().filter(|s| s.is_stale()).count(),
        missing_attachments: states.iter().filter(|s| !s.present).count(),
    }
}

/// The answer of `notebook_measurements` for `sources`.
pub fn measurements_report(sources: &NotebookSources) -> MeasurementsReport {
    let found = record(sources);
    let rows = rows(&found, sources);
    let states = attachment_states(sources, &found);
    MeasurementsReport {
        summary: summary(&rows, &states),
        readings: rows,
        problems: found.measurement_problems,
    }
}

/// The answer of `notebook_attachments` for `sources`.
pub fn attachments_report(sources: &NotebookSources) -> AttachmentsReport {
    let found = record(sources);
    let rows = rows(&found, sources);
    let states = attachment_states(sources, &found);
    AttachmentsReport {
        summary: summary(&rows, &states),
        attachments: states,
        problems: found.attachment_problems,
    }
}

/// The answer of `notebook_timeline` for `reference` and `quantity`.
pub fn timeline_report(
    sources: &NotebookSources,
    reference: &Reference,
    quantity: Option<&str>,
) -> TimelineReport {
    let found = record(sources);
    let timeline = timeline(
        &found.readings,
        reference,
        quantity,
        &found.attachments,
        sources.design_values(),
    );
    TimelineReport {
        quantities: quantities_of(&found.readings, reference),
        csv: timeline.to_csv(),
        timeline,
    }
}
