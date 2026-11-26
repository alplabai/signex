//! Everything a notebook records on the bench, over every file it holds:
//! the readings, the attachments and what could not be read, by file and
//! line. The CLI answers, the timeline, the stale marks and the printed
//! figures all start here.

use crate::attachment::{Attachment, AttachmentProblemKind, attachments};
use crate::measurement::{MeasurementProblemKind, Reading, readings};
use crate::sources::NotebookSources;

/// A problem, where it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileProblem<K> {
    /// The notebook file, a path inside `notebook/`.
    pub file: String,
    pub line: u32,
    #[serde(flatten)]
    pub kind: K,
}

/// The readings and attachments of a notebook.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NotebookRecord {
    /// In file order (the notebook file first, then its chapters by path),
    /// each file in source order.
    pub readings: Vec<Reading>,
    pub attachments: Vec<Attachment>,
    pub measurement_problems: Vec<FileProblem<MeasurementProblemKind>>,
    pub attachment_problems: Vec<FileProblem<AttachmentProblemKind>>,
}

/// The record of every `.snxnbk` file `sources` holds. A file that is not
/// UTF-8 is read with its bad bytes replaced; the compile reports it.
pub fn record(sources: &NotebookSources) -> NotebookRecord {
    let mut files: Vec<&str> = sources
        .paths()
        .filter(|path| path.ends_with(&format!(".{}", crate::NOTEBOOK_EXTENSION)))
        .collect();
    // The notebook file first.
    files.sort_by_key(|path| (*path != sources.main(), *path));
    let mut found = NotebookRecord::default();
    for file in files {
        let Some(bytes) = sources.get(file) else {
            continue;
        };
        let text = String::from_utf8_lossy(bytes);
        let (mut file_readings, problems) = readings(&text);
        for reading in &mut file_readings {
            reading.file = file.to_string();
        }
        found.readings.extend(file_readings);
        found
            .measurement_problems
            .extend(problems.into_iter().map(|p| FileProblem {
                file: file.to_string(),
                line: p.line,
                kind: p.kind,
            }));
        let (mut file_attachments, problems) = attachments(&text);
        for attachment in &mut file_attachments {
            attachment.source_file = file.to_string();
        }
        found.attachments.extend(file_attachments);
        found
            .attachment_problems
            .extend(problems.into_iter().map(|p| FileProblem {
                file: file.to_string(),
                line: p.line,
                kind: p.kind,
            }));
    }
    found
}
