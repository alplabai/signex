//! The source a new block is inserted with: the Calculation, Measurement
//! and Block buttons of the notebook toolbar fill in the next free id of
//! the kind (mockup rows 195 and 198).

use crate::annotation::{Annotation, AnnotationKind, Status};
use crate::date::IsoDate;
use crate::id::BlockId;
use crate::reference::quote;

/// The next free id of `kind` among the blocks of `annotations`, in
/// `section` (empty for a flat id: `MEAS-010` after `MEAS-009`).
pub fn next_block_id(kind: AnnotationKind, section: &[u64], annotations: &[Annotation]) -> BlockId {
    BlockId::next_free(
        kind,
        section,
        annotations.iter().filter_map(|a| a.id.as_ref()),
    )
}

/// A new, empty block of `id`'s kind, dated `today`: the fields the kind
/// prints, left blank, and an empty body where the kind has one.
pub fn new_block_source(id: &BlockId, today: IsoDate) -> String {
    let kind = id.kind();
    let head = format!(
        "#{}(\n  id: {},\n  title: [],\n",
        kind.function(),
        quote(&id.to_string())
    );
    let date = format!("  date: {},\n", quote(&today.to_string()));
    let status = |status: Status| format!("  status: {},\n", quote(status.token()));
    match kind {
        AnnotationKind::Note => format!("{head}{date})[\n  \n]"),
        AnnotationKind::Calculation => format!("{head}{})[\n  \n]", status(Status::Current)),
        AnnotationKind::Measurement => {
            format!("{head}{date}{}  rows: (),\n)", status(Status::Planned))
        }
        AnnotationKind::Issue => format!("{head}{}{date})[\n  \n]", status(Status::Open)),
        AnnotationKind::Decision => format!(
            "{head}{}{date}  chosen: [],\n  alternatives: (),\n)[\n  \n]",
            status(Status::Proposed)
        ),
    }
}
