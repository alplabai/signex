//! The Design Notebook's operands (mockup rows 195 to 202): each names the
//! project by a `document` (the project file, or any document of it).

use super::{Operand, OperandKind, optional, required};

/// The project's Design Notebook as PDF (mockup row 202): which project (or
/// one of its documents), the notebook file (`design.snxnbk` when left
/// out) and the file (relative to the project folder; the default name in
/// `Project Outputs/notebook/` when left out, and an existing file only
/// with `overwrite`); the rest are the export dialog's settings.
pub const NOTEBOOK_PDF_EXPORT: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("path", OperandKind::Path),
    optional("overwrite", OperandKind::Bool),
    optional("notebook", OperandKind::Text),
    optional("pages", OperandKind::Text),
    optional("paper", OperandKind::Text),
    optional("variant", OperandKind::Text),
    optional("annotations", OperandKind::Text),
    optional("stale", OperandKind::Text),
    optional("title_block", OperandKind::Bool),
    optional("contents", OperandKind::Bool),
    optional("bookmarks", OperandKind::Bool),
    optional("python_source", OperandKind::Bool),
];
/// `list_annotations` (mockup row 198): the annotation blocks of the
/// project the named document is (or belongs to), narrowed to one typed
/// `id`, one `kind` or the blocks naming one `part`.
pub const LIST_ANNOTATIONS: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("id", OperandKind::Text),
    optional("kind", OperandKind::Text),
    optional("part", OperandKind::Text),
];
/// `notebook_timeline` (mockup row 199): the readings of one net, part or
/// pin (`U3.3`) of the named project's notebook, of one quantity or all.
pub const NOTEBOOK_TIMELINE: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("net", OperandKind::Text),
    optional("part", OperandKind::Text),
    optional("pin", OperandKind::Text),
    optional("quantity", OperandKind::Text),
];
/// `export_notebook_timeline`: that timeline's list written as CSV to
/// `path` (relative to the project folder; an existing file only with
/// `overwrite`).
pub const NOTEBOOK_TIMELINE_EXPORT: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("net", OperandKind::Text),
    optional("part", OperandKind::Text),
    optional("pin", OperandKind::Text),
    optional("quantity", OperandKind::Text),
    required("path", OperandKind::Path),
    optional("overwrite", OperandKind::Bool),
];
/// `mark_notebook_attachment_valid`: the attachment (`file`, inside
/// `notebook/attachments/`) or the reading (`id`, `MEAS-..`) to mark.
pub const NOTEBOOK_MARK_VALID: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("file", OperandKind::Text),
    optional("id", OperandKind::Text),
];
/// `attach_notebook_file`: the file, the attachments folder it goes to
/// (`kind`), its caption (`text`) and what it shows.
pub const NOTEBOOK_ATTACH: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("kind", OperandKind::Text),
    optional("text", OperandKind::Text),
    optional("net", OperandKind::Text),
    optional("part", OperandKind::Text),
    optional("pin", OperandKind::Text),
];
/// `generate_design_notebook` (#2945, mockup row 201): the design report of
/// the project the named document is (or belongs to). `sections` is a comma
/// list or `all`; `checks` is `run` or `last`; `fail_on` (`requirements`,
/// `checks`) makes the summary's status `fail` when one holds; `save` writes
/// `notebook/generated/design-report.snxnbk`, refused over an existing one
/// unless `update`.
pub const GENERATE_DESIGN_NOTEBOOK: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("sections", OperandKind::Text),
    optional("variant", OperandKind::Text),
    optional("template", OperandKind::Text),
    optional("document_number", OperandKind::Text),
    optional("revision", OperandKind::Text),
    optional("checks", OperandKind::Text),
    optional("fail_on", OperandKind::Text),
    optional("save", OperandKind::Bool),
    optional("update", OperandKind::Bool),
];
/// `notebook_insert` (#2795): what an insert button writes into the open
/// notebook. `kind` is `calculation`, `measurement`, `figure`, `note`,
/// `issue` or `decision`, or `component`, `pin` or `net` with the `name`
/// (`R7`, `U3.12`, `+3V3`); `line` (from 1, in the file) moves the cursor
/// there first.
pub const NOTEBOOK_INSERT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("kind", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("line", OperandKind::Text),
];
/// `notebook_view` (#2795): the tab's `view` (`source`, `split`,
/// `preview`), its side `panel` (`outline`, `pages`, `references`), and
/// `next_problem` to move the cursor to the next problem.
pub const NOTEBOOK_VIEW: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("view", OperandKind::Text),
    optional("panel", OperandKind::Text),
    optional("next_problem", OperandKind::Bool),
];
