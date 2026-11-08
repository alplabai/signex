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
            Self::Angle => Some("deg"),
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
    ("designator", OperandKind::Text),
    ("value", OperandKind::Text),
    ("net", OperandKind::Text),
    ("ids", OperandKind::Text),
    ("kinds", OperandKind::Text),
    ("region", OperandKind::Text),
    ("fields", OperandKind::Text),
    ("offset", OperandKind::Text),
    ("limit", OperandKind::Text),
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

/// Place one object at a point.
pub const AT: &[Operand] = &[required("at", OperandKind::Point)];
/// Draw from one point to another.
pub const SEGMENT: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
];
/// A label: its text at a point.
pub const LABEL: &[Operand] = &[
    required("at", OperandKind::Point),
    required("text", OperandKind::Text),
    optional("rotation", OperandKind::Angle),
];
/// A text note at a point.
pub const TEXT: &[Operand] = &[
    required("at", OperandKind::Point),
    required("text", OperandKind::Text),
];
/// A power port: the net it names and its library symbol.
pub const POWER: &[Operand] = &[
    required("at", OperandKind::Point),
    required("net_name", OperandKind::Text),
    required("lib_id", OperandKind::Text),
];
/// An open dialog's answer.
pub const OPEN_PATH: &[Operand] = &[required("path", OperandKind::Path)];
/// A save dialog's answer; `overwrite` lets it replace an existing file. A new
/// project accepts it and ignores it: an existing project is always refused.
pub const SAVE_PATH: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("overwrite", OperandKind::Bool),
];

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
/// Put a net, by its name in the project netlist, in a class of the project
/// the named sheet belongs to; no `class` takes it out of its listed class.
pub const ASSIGN_NET_CLASS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("net_name", OperandKind::Text),
    optional("class", OperandKind::Text),
];

/// The golden column: `name:kind` with `/unit`, `?` when optional, comma
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
