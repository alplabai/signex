//! An annotation block, its kind and its status.

use std::ops::Range;

use crate::date::IsoDate;
use crate::id::BlockId;
use crate::record::Record;
use crate::reference::Reference;

/// What an annotation records. Written as the `kind:` argument of the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnnotationKind {
    Note,
    Calculation,
    Measurement,
    Issue,
    Decision,
}

impl AnnotationKind {
    /// Every kind, in the order the panel lists them.
    pub const ALL: [AnnotationKind; 5] = [
        AnnotationKind::Note,
        AnnotationKind::Calculation,
        AnnotationKind::Measurement,
        AnnotationKind::Issue,
        AnnotationKind::Decision,
    ];

    /// The token written in the source: `note`, `calculation`, ...
    pub fn token(self) -> &'static str {
        match self {
            AnnotationKind::Note => "note",
            AnnotationKind::Calculation => "calculation",
            AnnotationKind::Measurement => "measurement",
            AnnotationKind::Issue => "issue",
            AnnotationKind::Decision => "decision",
        }
    }

    /// The kind written as `token`; tokens are lower case and exact.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.token() == token)
    }

    /// The statuses a block of this kind may carry; a Note carries none.
    pub fn statuses(self) -> &'static [Status] {
        match self {
            AnnotationKind::Note => &[],
            AnnotationKind::Calculation => &[Status::Current, Status::Stale],
            AnnotationKind::Measurement => &[Status::Planned, Status::Done],
            AnnotationKind::Issue => &[Status::Open, Status::Resolved],
            AnnotationKind::Decision => &[Status::Proposed, Status::Accepted, Status::Superseded],
        }
    }
}

/// Where an annotation stands. Which ones a kind accepts is
/// [`AnnotationKind::statuses`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    /// Issue: not fixed yet.
    Open,
    /// Issue: fixed.
    Resolved,
    /// Decision: suggested, not taken.
    Proposed,
    /// Decision: taken.
    Accepted,
    /// Decision: replaced by a later one.
    Superseded,
    /// Measurement: still to do.
    Planned,
    /// Measurement: taken.
    Done,
    /// Calculation: matches the design ("Up to date").
    Current,
    /// Calculation: the design changed since ("Out of date").
    Stale,
}

impl Status {
    /// The token written in the source.
    pub fn token(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Resolved => "resolved",
            Status::Proposed => "proposed",
            Status::Accepted => "accepted",
            Status::Superseded => "superseded",
            Status::Planned => "planned",
            Status::Done => "done",
            Status::Current => "current",
            Status::Stale => "stale",
        }
    }

    /// The status written as `token` on a block of `kind`; `None` when the
    /// kind does not accept it.
    pub fn parse(kind: AnnotationKind, token: &str) -> Option<Self> {
        kind.statuses()
            .iter()
            .copied()
            .find(|status| status.token() == token)
    }
}

/// One annotation block of a notebook: `#note(...)`, `#calculation(...)`,
/// `#measurement(...)`, `#issue(...)`, `#decision(...)`, or the earlier
/// `#annotation(kind: ...)[...]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub kind: AnnotationKind,
    /// The typed id (`DEC-002`); the earlier form and a block written
    /// without one have none.
    pub id: Option<BlockId>,
    /// The `title:` argument, else the first line of the body (see
    /// `docs/NOTEBOOK_ANNOTATIONS.md`, "Title").
    pub title: String,
    pub status: Option<Status>,
    pub date: Option<IsoDate>,
    /// The whole block as written, from its `#` to its closing bracket.
    pub source: String,
    /// The body as plain text, references shown as their names.
    pub snippet: String,
    /// The parts, nets and pins the block names (its `target:` first, then
    /// its fields and body), first mention first, each once.
    pub references: Vec<Reference>,
    /// The review-record fields of its kind.
    pub record: Record,
    /// The 1-based line the block starts on.
    pub line: u32,
    /// The bytes of the file text the block spans.
    pub span: Range<usize>,
}

impl Annotation {
    /// The first part the block names (a component, else a pin's part).
    pub fn first_component(&self) -> Option<&str> {
        self.references.iter().find_map(Reference::component)
    }
}

impl serde::Serialize for AnnotationKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.token())
    }
}

impl serde::Serialize for Status {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.token())
    }
}

/// Written as `{"id": "DEC-002", "kind": "decision", "title": ...,
/// "status": "accepted", "date": "2026-11-18", "line": 214, "references":
/// [...], "snippet": ..., "record": {...}}` in a JSON result.
impl serde::Serialize for Annotation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("kind", &self.kind)?;
        map.serialize_entry("title", &self.title)?;
        map.serialize_entry("status", &self.status)?;
        map.serialize_entry("date", &self.date.map(|date| date.to_string()))?;
        map.serialize_entry("line", &self.line)?;
        map.serialize_entry("references", &self.references)?;
        map.serialize_entry("snippet", &self.snippet)?;
        map.serialize_entry("record", &self.record)?;
        map.end()
    }
}
