//! How blocks cite each other by typed id: an Issue raised `from:` a
//! Measurement or Calculation (which then "opens" it), a Decision that
//! `supersedes:` another, a Calculation reading `inputs:`. A failing
//! Measurement opens an Issue on its own (mockup row 198, decided as
//! drawn): [`open_failing_measurements`] writes that Issue after it.

use crate::annotation::{Annotation, AnnotationKind, Status};
use crate::date::IsoDate;
use crate::id::BlockId;
use crate::record::{Citation, Record, Verdict};
use crate::reference::{Reference, quote};

/// One block naming another.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Link {
    /// The block that names the other: the Issue of a `from:`.
    pub by: BlockId,
    /// The block named.
    pub to: BlockId,
    pub citation: Citation,
    /// The named block is in the list.
    pub resolved: bool,
}

/// Every citation among `annotations`, in source order of the citing
/// block.
pub fn links(annotations: &[Annotation]) -> Vec<Link> {
    let known = |id: &BlockId| annotations.iter().any(|a| a.id.as_ref() == Some(id));
    annotations
        .iter()
        .filter_map(|a| a.id.as_ref().map(|id| (id, a)))
        .flat_map(|(by, a)| {
            a.record
                .cites()
                .into_iter()
                .map(move |(citation, to)| Link {
                    by: by.clone(),
                    to: to.clone(),
                    citation,
                    resolved: false,
                })
        })
        .map(|link| Link {
            resolved: known(&link.to),
            ..link
        })
        .collect()
}

/// The blocks that name `id`, with how: the Issues a Measurement opens,
/// the Decision that supersedes a Decision, the Calculations reading it.
pub fn cited_by(id: &BlockId, annotations: &[Annotation]) -> Vec<(Citation, BlockId)> {
    links(annotations)
        .into_iter()
        .filter(|link| &link.to == id)
        .map(|link| (link.citation, link.by))
        .collect()
}

/// The Measurements of `annotations` that fail and open no Issue yet: each
/// has an id, a Fail verdict and no Issue `from:` it.
pub fn unopened_failures(annotations: &[Annotation]) -> Vec<&Annotation> {
    annotations
        .iter()
        .filter(|a| a.kind == AnnotationKind::Measurement)
        .filter(|a| a.record.verdict() == Some(Verdict::Fail))
        .filter(|a| {
            a.id.as_ref().is_some_and(|id| {
                !cited_by(id, annotations)
                    .iter()
                    .any(|(citation, _)| *citation == Citation::From)
            })
        })
        .collect()
}

/// The `#issue` a failing `measurement` opens: the next free `ISS-` id in
/// the measurement's own section (`MEAS-3.2` opens `ISS-3.1`), open,
/// dated `today`, `from:` the measurement, aimed at the failing rows'
/// targets, titled `title`. `None` when the measurement has no id.
pub fn issue_for_failure<'a>(
    measurement: &Annotation,
    taken: impl IntoIterator<Item = &'a BlockId>,
    today: IsoDate,
    title: &str,
) -> Option<(BlockId, String)> {
    let from = measurement.id.as_ref()?;
    let numbers = from.numbers();
    let section = &numbers[..numbers.len().saturating_sub(1)];
    let id = BlockId::next_free(AnnotationKind::Issue, section, taken);
    let targets = failing_targets(measurement);
    let target = match targets.as_slice() {
        [] => String::new(),
        list => {
            let items: Vec<String> = list.iter().map(Reference::to_code).collect();
            format!("  target: ({},),\n", items.join(", "))
        }
    };
    let source = format!(
        "#issue(\n  id: {},\n  title: {},\n  status: {},\n  date: {},\n  from: {},\n{target})",
        quote(&id.to_string()),
        quote(title),
        quote(Status::Open.token()),
        quote(&today.to_string()),
        quote(&from.to_string()),
    );
    Some((id, source))
}

/// `text` with an Issue written after each failing Measurement that opens
/// none yet, or `None` when there is none to open. `title` names each
/// Issue from its Measurement.
pub fn open_failing_measurements(
    text: &str,
    today: IsoDate,
    title: impl Fn(&Annotation) -> String,
) -> Option<String> {
    let (annotations, _) = crate::scan(text);
    let failures = unopened_failures(&annotations);
    if failures.is_empty() {
        return None;
    }
    let mut taken: Vec<BlockId> = annotations.iter().filter_map(|a| a.id.clone()).collect();
    let mut inserts = Vec::new();
    for measurement in failures {
        let Some((id, source)) = issue_for_failure(measurement, &taken, today, &title(measurement))
        else {
            continue;
        };
        taken.push(id);
        inserts.push((measurement.span.end, source));
    }
    let mut out = text.to_string();
    // From the end, so each offset still points where it did.
    for (at, source) in inserts.into_iter().rev() {
        out.insert_str(at, &format!("\n\n{source}"));
    }
    Some(out)
}

/// The targets of the rows that fail, else every reference of the block.
fn failing_targets(measurement: &Annotation) -> Vec<Reference> {
    let mut targets: Vec<Reference> = match &measurement.record {
        Record::Measurement(record) => record
            .rows
            .iter()
            .filter(|row| row.verdict == Some(Verdict::Fail))
            .filter_map(|row| row.target.clone())
            .collect(),
        _ => Vec::new(),
    };
    if targets.is_empty() {
        targets = measurement.references.clone();
    }
    targets.dedup();
    targets
}
