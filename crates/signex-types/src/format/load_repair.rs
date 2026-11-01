//! What loading a sheet changed to make it usable (#1830).
//!
//! The parser records these on `SheetUnknown::load_repairs`; the document
//! that opens the sheet takes them once with
//! `SchematicSheet::take_load_repairs`, reports them in Messages and marks
//! the sheet modified (the file on disk still holds what was repaired).
//! Nothing here logs: previews, scans and history reads parse sheets too.

use uuid::Uuid;

/// The kind a repaired sheet symbol reports under; its re-minted id needs
/// the designators placed under it annotated again.
pub(in crate::format) const SHEET_SYMBOL: &str = "sheet symbol";

/// The object a repair names: its kind, its label or reference when it has
/// one, and the 1-based row it was read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairedObject {
    pub kind: &'static str,
    pub name: String,
    pub row: usize,
}

/// One change made while parsing a sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SheetLoadRepair {
    /// An exact copy of an earlier object of the same kind, carrying its
    /// uuid, was dropped.
    CopyDropped { object: RepairedObject, uuid: Uuid },
    /// A different object carrying an id already taken got a new one.
    UuidReminted {
        object: RepairedObject,
        old: Uuid,
        new: Uuid,
        /// The sheet symbol a sheet pin belongs to.
        owner: Option<String>,
    },
    /// Two pins of one symbol shared an id; the later pin got a new one.
    PinUuidReminted {
        symbol: String,
        pin: String,
        old: Uuid,
        new: Uuid,
    },
}

impl RepairedObject {
    fn label(&self) -> String {
        if self.name.is_empty() {
            format!("{} (row {})", self.kind, self.row)
        } else {
            format!("{} {} (row {})", self.kind, self.name, self.row)
        }
    }
}

impl SheetLoadRepair {
    /// The one-sentence wording for Messages. Short on purpose: the panel
    /// cuts long entries.
    pub fn describe(&self) -> String {
        match self {
            Self::CopyDropped { object, uuid } => format!(
                "{} repeated uuid {uuid} of an identical {}; dropped",
                object.label(),
                object.kind
            ),
            Self::UuidReminted {
                object,
                old,
                new,
                owner,
            } => {
                let mut text = format!("{} repeated uuid {old}; new uuid {new}", object.label());
                if let Some(owner) = owner {
                    text.push_str(&format!(" on sheet symbol {owner}"));
                }
                if object.kind == SHEET_SYMBOL {
                    text.push_str("; re-annotate its placed designators");
                }
                text
            }
            Self::PinUuidReminted {
                symbol,
                pin,
                old,
                new,
            } => format!("pin {pin} of symbol {symbol} repeated uuid {old}; new uuid {new}"),
        }
    }
}
