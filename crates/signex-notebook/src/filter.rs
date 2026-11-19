//! Which annotations a list shows, and in what order.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::annotation::{Annotation, AnnotationKind, Status};
use crate::date::IsoDate;

/// How recent an annotation must be.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DateWindow {
    /// Dated or not.
    #[default]
    Any,
    /// Dated today or in the six days before.
    Last7Days,
    /// Dated today or in the 29 days before.
    Last30Days,
}

impl DateWindow {
    pub const ALL: [DateWindow; 3] = [
        DateWindow::Any,
        DateWindow::Last7Days,
        DateWindow::Last30Days,
    ];

    /// Days the window spans, today included; `None` for any date.
    pub fn days(self) -> Option<i64> {
        match self {
            DateWindow::Any => None,
            DateWindow::Last7Days => Some(7),
            DateWindow::Last30Days => Some(30),
        }
    }
}

/// The filters of an annotation list. The default shows everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Filter {
    /// Text the title, the snippet or a reference must contain, any case.
    pub query: String,
    /// Kinds left out.
    pub hidden_kinds: BTreeSet<AnnotationKind>,
    /// A part the annotation must name (a component or a pin's part).
    pub part: Option<String>,
    pub status: Option<Status>,
    pub date: DateWindow,
}

impl Filter {
    /// Whether `annotation` passes every filter, `today` being the date the
    /// window counts back from. An undated annotation passes only
    /// [`DateWindow::Any`]; a date after today passes every window.
    pub fn shows(&self, annotation: &Annotation, today: IsoDate) -> bool {
        !self.hidden_kinds.contains(&annotation.kind)
            && self.matches_query(annotation)
            && self.part.as_deref().is_none_or(|part| {
                annotation
                    .references
                    .iter()
                    .any(|r| r.component() == Some(part))
            })
            && self
                .status
                .is_none_or(|status| annotation.status == Some(status))
            && match self.date.days() {
                None => true,
                Some(days) => annotation
                    .date
                    .is_some_and(|date| date.days_since_epoch() > today.days_since_epoch() - days),
            }
    }

    fn matches_query(&self, annotation: &Annotation) -> bool {
        let needle = self.query.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        let has = |text: &str| text.to_lowercase().contains(&needle);
        has(&annotation.title)
            || has(&annotation.snippet)
            || annotation.references.iter().any(|r| has(&r.name()))
    }
}

/// Newest first by date, undated last; the same date keeps source order.
pub fn newest_first(a: &Annotation, b: &Annotation) -> Ordering {
    match (a.date, b.date) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then(a.line.cmp(&b.line))
}
