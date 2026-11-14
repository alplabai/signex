//! The operands a command takes besides its id (#1934): what a CLI or an agent
//! can pass in place of a pointer click or a dialog answer. One naming
//! convention, kept by [`CONVENTION`] and a test over every catalog row.

/// The shape of one operand's value, as written in text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    /// `x,y` in mm.
    Point,
    /// A multiple of 90 degrees within ±360, stored as 0, 90, 180 or 270:
    /// the pre-placement form's choices.
    Angle,
    /// Any angle within ±360 degrees, clockwise positive: a turn of the
    /// view, not the pre-placement form's right angles (#2316).
    Turn,
    /// Free text (a label text, a net name, a library id).
    Text,
    /// An absolute path on the host running the command (a GUI file dialog's
    /// answer).
    Path,
    /// `true` or `false`.
    Bool,
    /// An opaque colour, `#rrggbb` (the leading `#` is optional).
    Color,
    /// A document: its id (a hyphenated uuid) or an absolute path (#2172).
    Document,
}

impl OperandKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Angle => "angle",
            Self::Turn => "turn",
            Self::Text => "text",
            Self::Path => "path",
            Self::Bool => "bool",
            Self::Color => "color",
            Self::Document => "document",
        }
    }

    /// The unit the value is written in; `None` for a kind without one.
    pub const fn unit(self) -> Option<&'static str> {
        match self {
            Self::Point => Some("mm"),
            Self::Angle | Self::Turn => Some("deg"),
            Self::Text | Self::Path | Self::Bool | Self::Color | Self::Document => None,
        }
    }
}

/// One named operand of a command. A command's operands are the inputs it
/// needs; everything else (line width, fill, wire mode, label shape, font and
/// justification) comes from the pre-placement form and the project defaults,
/// as for a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operand {
    /// snake_case, one of the [`CONVENTION`] names.
    pub name: &'static str,
    pub kind: OperandKind,
    pub required: bool,
}

/// The allowed operand names and the one kind each has, on every command.
#[cfg(test)]
pub const CONVENTION: &[(&str, OperandKind)] = &[
    ("at", OperandKind::Point),
    ("start", OperandKind::Point),
    ("end", OperandKind::Point),
    ("rotation", OperandKind::Angle),
    ("degrees", OperandKind::Turn),
    ("text", OperandKind::Text),
    ("net_name", OperandKind::Text),
    ("class", OperandKind::Text),
    ("lib_id", OperandKind::Text),
    ("path", OperandKind::Path),
    ("overwrite", OperandKind::Bool),
    ("color", OperandKind::Color),
    ("document", OperandKind::Document),
    ("id", OperandKind::Text),
    ("kind", OperandKind::Text),
    ("project", OperandKind::Text),
    ("name", OperandKind::Text),
    ("dirty", OperandKind::Bool),
    ("allow_merge", OperandKind::Bool),
    ("allow_open", OperandKind::Bool),
    ("dry_run", OperandKind::Bool),
    ("designator", OperandKind::Text),
    ("value", OperandKind::Text),
    ("net", OperandKind::Text),
    ("ids", OperandKind::Text),
    ("kinds", OperandKind::Text),
    ("region", OperandKind::Text),
    ("fields", OperandKind::Text),
    ("offset", OperandKind::Text),
    ("limit", OperandKind::Text),
    ("expect_revision", OperandKind::Text),
    ("expect_revisions", OperandKind::Text),
    ("edits", OperandKind::Text),
    ("summary", OperandKind::Text),
    ("group", OperandKind::Text),
    ("documents", OperandKind::Text),
    ("location", OperandKind::Path),
    ("make_folder", OperandKind::Bool),
    ("profile", OperandKind::Text),
    ("overrides", OperandKind::Text),
    ("library", OperandKind::Text),
    ("part", OperandKind::Text),
    ("unit", OperandKind::Text),
    ("mirror", OperandKind::Text),
    ("sheet", OperandKind::Text),
    ("file", OperandKind::Text),
    ("direction", OperandKind::Text),
    ("apply", OperandKind::Bool),
    ("baseline", OperandKind::Text),
];

const fn required(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: true,
    }
}

const fn optional(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: false,
    }
}

// A placement takes an optional `document`: it then runs on that sheet and
// never on the active tab (#2199); without it, on the sheet the edit targets.

/// Place one object at a point.
pub const AT: &[Operand] = &[
    required("at", OperandKind::Point),
    optional("document", OperandKind::Document),
];
/// Draw from one point to another.
pub const SEGMENT: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("document", OperandKind::Document),
];
/// A wire run, checked against the sheet before it lands (#2199): `allow_merge`
/// lets it join two differently named nets, `allow_open` leaves an end
/// unconnected, `dry_run` reports without changing anything.
pub const WIRE: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("allow_merge", OperandKind::Bool),
    optional("allow_open", OperandKind::Bool),
    optional("dry_run", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A junction at a point: `allow_merge` lets it join two differently named
/// nets (#2199).
pub const JUNCTION: &[Operand] = &[
    required("at", OperandKind::Point),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A bus run: as [`WIRE`]; `allow_merge` lets it join two differently named
/// buses.
pub const BUS: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("allow_merge", OperandKind::Bool),
    optional("allow_open", OperandKind::Bool),
    optional("dry_run", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A label: its text at a point. `allow_merge` lets it name a net that already
/// has another name (#2199).
pub const LABEL: &[Operand] = &[
    required("at", OperandKind::Point),
    required("text", OperandKind::Text),
    optional("rotation", OperandKind::Angle),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A text note at a point.
pub const TEXT: &[Operand] = &[
    required("at", OperandKind::Point),
    required("text", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A power port: the net it names and its library symbol.
pub const POWER: &[Operand] = &[
    required("at", OperandKind::Point),
    required("net_name", OperandKind::Text),
    required("lib_id", OperandKind::Text),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A library part (#2199): the row of an open library, where, and which unit;
/// the designator, value, rotation and mirror a click's form would give it.
/// A connection point landing on anything is refused (no `allow_merge`).
pub const PART: &[Operand] = &[
    required("library", OperandKind::Text),
    required("part", OperandKind::Text),
    required("at", OperandKind::Point),
    optional("unit", OperandKind::Text),
    optional("designator", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("rotation", OperandKind::Angle),
    optional("mirror", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A sheet symbol between two corners, referring to its child `file` (relative
/// to the sheet, inside the project folder; never written). `allow_merge` lets
/// the nets the file brings in short or rename project nets (#2199).
pub const SHEET_SYMBOL: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    required("name", OperandKind::Text),
    required("file", OperandKind::Text),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// An entry on the border of the sheet symbol named `sheet`.
pub const SHEET_ENTRY: &[Operand] = &[
    required("sheet", OperandKind::Text),
    required("at", OperandKind::Point),
    required("name", OperandKind::Text),
    optional("direction", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// The entries of the sheet symbol named `sheet` matched to its child file.
pub const SYNC_SHEET: &[Operand] = &[
    required("sheet", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// An open dialog's answer.
pub const OPEN_PATH: &[Operand] = &[required("path", OperandKind::Path)];
/// A save dialog's answer; `overwrite` lets it replace an existing file.
pub const SAVE_PATH: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("overwrite", OperandKind::Bool),
];

/// A new project from the dialog's choices (#2232): where, which profile
/// and `key=value` overrides.
pub const NEW_PROJECT: &[Operand] = &[
    required("name", OperandKind::Text),
    required("location", OperandKind::Path),
    optional("make_folder", OperandKind::Bool),
    optional("profile", OperandKind::Text),
    optional("overrides", OperandKind::Text),
];

/// Update PCB without the dialog (#1312): `apply=true` executes the rows
/// ticked by default.
pub const APPLY: &[Operand] = &[required("apply", OperandKind::Bool)];

/// Turn the board view by `degrees` (90 when left out), clockwise positive
/// (#2316).
pub const TURN: &[Operand] = &[optional("degrees", OperandKind::Turn)];

/// Colour the net under a point of the named document's sheet.
pub const COLOR_NET: &[Operand] = &[
    required("at", OperandKind::Point),
    required("color", OperandKind::Color),
    required("document", OperandKind::Document),
];
/// The document a command acts on, by id or path. A headless or CLI run names
/// it every time and never falls back to the active one (#2172).
pub const DOCUMENT: &[Operand] = &[required("document", OperandKind::Document)];
/// A save dialog's answer for the named document.
pub const SAVE_DOCUMENT_AS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("overwrite", OperandKind::Bool),
];
/// `resolve`: the uuid to say what it is.
pub const RESOLVE: &[Operand] = &[required("id", OperandKind::Text)];
/// Which documents: the filters `list_documents` and `find` share.
pub const DOCUMENT_FILTERS: &[Operand] = &[
    optional("kind", OperandKind::Text),
    optional("project", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("dirty", OperandKind::Bool),
];
/// `find`: documents by the shared filters, or elements by what a person
/// reads on them, in one document or a whole project.
pub const FIND: &[Operand] = &[
    optional("document", OperandKind::Document),
    optional("kind", OperandKind::Text),
    optional("project", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("dirty", OperandKind::Bool),
    optional("designator", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("net", OperandKind::Text),
    optional("text", OperandKind::Text),
];
/// `read_document`: the named document's data, narrowed and paged.
pub const READ_DOCUMENT: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("ids", OperandKind::Text),
    optional("kinds", OperandKind::Text),
    optional("region", OperandKind::Text),
    optional("fields", OperandKind::Text),
    optional("offset", OperandKind::Text),
    optional("limit", OperandKind::Text),
];
/// `query_pins`: the pins, with their tip positions, of the part a designator
/// names on one sheet.
pub const QUERY_PINS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("designator", OperandKind::Text),
];
/// `check_erc` (#2199): ERC on the project of the named sheet, answered as a
/// check report; `baseline` is an earlier answer's `run`.
pub const CHECK_ERC: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("baseline", OperandKind::Text),
];
/// `check_drc` (#2199): DRC on the named board, answered as a check report;
/// `baseline` is an earlier answer's `run` on the same board.
pub const CHECK_DRC: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("baseline", OperandKind::Text),
];
/// Put a net, by its name in the project netlist, in a class of the project
/// the named sheet belongs to; no `class` takes it out of its listed class.
pub const ASSIGN_NET_CLASS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("net_name", OperandKind::Text),
    optional("class", OperandKind::Text),
];

/// `apply_edits` (#2178): a JSON list of catalog edit commands, applied to the
/// named document as one undo step if its revision is still `expect_revision`.
/// An edit that names another sheet of the project makes the call one group on
/// every sheet it touches; `expect_revisions` then holds each such sheet's
/// revision (a JSON object, document to revision).
pub const APPLY_EDITS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("expect_revision", OperandKind::Text),
    required("edits", OperandKind::Text),
    optional("summary", OperandKind::Text),
    optional("expect_revisions", OperandKind::Text),
];
/// `revert` (#2178): take back the agent step `group` of the named document,
/// while it is that document's newest step.
pub const REVERT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("group", OperandKind::Text),
    optional("documents", OperandKind::Text),
];

/// The golden column:`name:kind` with `/unit`, `?` when optional, comma
/// separated; `-` when the command takes none.
#[cfg(test)]
pub fn describe(operands: &[Operand]) -> String {
    if operands.is_empty() {
        return "-".into();
    }
    let parts: Vec<String> = operands
        .iter()
        .map(|op| {
            let unit = op.kind.unit().map_or(String::new(), |u| format!("/{u}"));
            let opt = if op.required { "" } else { "?" };
            format!("{}:{}{unit}{opt}", op.name, op.kind.name())
        })
        .collect();
    parts.join(",")
}
