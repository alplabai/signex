//! Attachments (mockup row 199): scope captures, thermal images and
//! simulation plots, printed as numbered figures with a source line. The
//! file lives in the project, in `notebook/attachments/`; a file dropped on
//! the page or picked from Figure is copied there on Save ([`copy`]). Each
//! attachment stores the values of the parts it names when it was taken
//! (`taken-with:`), and is stale once one of them changes ([`stale`]).
//!
//! ```text
//! #attachment("scope/ripple-2A-revB.csv",
//!   caption: [Output ripple on +3V3 at 2 A],
//!   target: (net("+3V3"), component("C15")),
//!   taken-with: ("L1": "4.7 µH", "C15": "22 µF"),
//!   taken: "2026-11-24T15:12",
//!   channels: ((name: "CH1", label: "+3V3", coupling: "AC", scale: "5 mV", bandwidth: "20 MHz"),),
//!   time-scale: "1 µs",
//!   cursors: (y1: "9 mV", y2: "-9 mV"),
//! )
//! ```

mod copy;
mod stale;

pub use copy::{AttachmentCopy, attachment_call, folder_for, plan_copy};
pub use stale::{DesignValues, ValueChange, changes, mark_still_valid, reading_changes};

use crate::literal::{FoundCall, Literal, calls_of};
use crate::measurement::read::{pairs, written};
use crate::reference::Reference;
use crate::scope::PlotSpec;
use crate::sources::resolve;
use crate::syntax::typst_source;
use crate::timestamp::Timestamp;

/// The folder inside `notebook/` that holds the attachments.
pub const ATTACHMENTS_DIR: &str = "attachments";
/// The function an attachment is written with.
pub(crate) const FUNCTION: &str = "attachment";

/// What an attachment shows, by its file's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    /// A scope's CSV export, drawn as a vector plot.
    ScopeCsv,
    /// A picture (a scope screenshot, a thermal image), shown as is.
    Image,
    /// A Signex simulation result (`.snxsim`).
    Simulation,
    /// Anything else: listed, printed as a source line only.
    Other,
}

impl AttachmentKind {
    /// The kind of the file `path`, by its extension, case-insensitive.
    pub fn of(path: &str) -> Self {
        let extension = path
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        match extension.as_str() {
            "csv" => AttachmentKind::ScopeCsv,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => AttachmentKind::Image,
            "snxsim" => AttachmentKind::Simulation,
            _ => AttachmentKind::Other,
        }
    }
}

/// One `#attachment(...)` call.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Attachment {
    /// The notebook file it is in; the reader of one source leaves it empty.
    pub source_file: String,
    /// The file, a path inside `notebook/attachments/`.
    pub file: String,
    pub kind: AttachmentKind,
    /// The caption's source text.
    pub caption: String,
    pub targets: Vec<Reference>,
    /// The part values it was taken with (designator, value).
    pub taken_with: Vec<(String, String)>,
    pub taken: Option<Timestamp>,
    pub rev: String,
    /// How a scope capture is drawn.
    #[serde(skip)]
    pub plot: PlotSpec,
    pub line: u32,
}

impl Attachment {
    /// The file as a path inside `notebook/`: `attachments/scope/a.csv`.
    pub fn path_in_notebook(&self) -> String {
        format!("{ATTACHMENTS_DIR}/{}", self.file)
    }
}

/// Something in an `#attachment` call that could not be used.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AttachmentProblem {
    pub line: u32,
    pub kind: AttachmentProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttachmentProblemKind {
    /// No file is named; the call is skipped.
    MissingFile,
    /// The file leaves `notebook/attachments/`; the call is skipped.
    OutsideAttachments { written: String },
    /// `taken:` is not a date with an optional time; it is left out.
    BadTimestamp { written: String },
}

/// The attachments of the notebook file text `source`, in source order,
/// and what could not be read.
pub fn attachments(source: &str) -> (Vec<Attachment>, Vec<AttachmentProblem>) {
    let text = typst_source(source);
    let mut problems = Vec::new();
    let found = calls_of(&text, FUNCTION)
        .iter()
        .filter_map(|call| read(call, &mut problems))
        .collect();
    (found, problems)
}

fn read(call: &FoundCall, problems: &mut Vec<AttachmentProblem>) -> Option<Attachment> {
    let line = call.line;
    let named = call
        .positional
        .iter()
        .find_map(|value| match value {
            Literal::Str(text) => Some(text.as_str()),
            _ => None,
        })
        .or_else(|| call.arg("file").and_then(Literal::text));
    let Some(named) = named.map(str::trim).filter(|f| !f.is_empty()) else {
        problems.push(AttachmentProblem {
            line,
            kind: AttachmentProblemKind::MissingFile,
        });
        return None;
    };
    let Some(file) = inside_attachments(named) else {
        problems.push(AttachmentProblem {
            line,
            kind: AttachmentProblemKind::OutsideAttachments {
                written: named.to_string(),
            },
        });
        return None;
    };
    let taken = call.arg("taken").and_then(|literal| {
        let parsed = literal.text().and_then(Timestamp::parse);
        if parsed.is_none() {
            problems.push(AttachmentProblem {
                line,
                kind: AttachmentProblemKind::BadTimestamp {
                    written: written(literal),
                },
            });
        }
        parsed
    });
    let text = |name: &str| {
        call.arg(name)
            .and_then(Literal::text)
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    };
    Some(Attachment {
        source_file: String::new(),
        kind: AttachmentKind::of(&file),
        file,
        caption: text("caption"),
        targets: call
            .arg("target")
            .map(Literal::references)
            .unwrap_or_default(),
        taken_with: pairs(call.arg("taken-with")),
        taken,
        rev: text("rev"),
        plot: PlotSpec::from_call(call),
        line,
    })
}

/// `written` as a path inside `notebook/attachments/`; `None` when it
/// climbs out of that folder.
pub fn inside_attachments(written: &str) -> Option<String> {
    let base = format!("{ATTACHMENTS_DIR}/_");
    let resolved = resolve(&base, written.trim_start_matches('/'))?;
    resolved
        .strip_prefix(&format!("{ATTACHMENTS_DIR}/"))
        .filter(|rest| !rest.is_empty())
        .map(str::to_string)
}
