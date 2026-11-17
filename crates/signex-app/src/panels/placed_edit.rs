//! The editable Position, Rotation, Size and Parameters of one placed object
//! in Properties (#2418): its messages and the state the view reads. A box
//! keeps what is typed as a draft and commits on Enter, so one edit is one
//! undo step; the commit runs the same app method as the `set_object_*`
//! commands (`app/command/placed_operands.rs`).

use signex_types::schematic::SelectedItem;

/// One editable box of a placed object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlacedField {
    /// The position's X, in the panel's unit.
    X,
    /// The position's Y, in the panel's unit, counted from the page origin.
    Y,
    /// A sheet symbol's width.
    Width,
    /// A sheet symbol's height.
    Height,
    /// The value of a placed part's parameter, by name.
    Parameter(String),
}

/// What a placed object's Properties rows send.
#[derive(Debug, Clone, PartialEq)]
pub enum PlacedEditMsg {
    /// A keystroke in a box: kept as the draft until Enter.
    Typed {
        item: SelectedItem,
        field: PlacedField,
        text: String,
    },
    /// Enter in a box: the draft becomes one edit.
    Submitted {
        item: SelectedItem,
        field: PlacedField,
    },
    /// A pick in the Rotation row, in stored degrees.
    RotationPicked { item: SelectedItem, degrees: f64 },
}

/// The box being typed in and its text.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedDraft {
    pub item: SelectedItem,
    pub field: PlacedField,
    pub text: String,
}

/// What the Properties rows of the single selected object read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlacedPanel {
    /// The object the rows edit, when it has an editable position.
    pub item: Option<SelectedItem>,
    /// Its geometry in stored mm.
    pub geometry: Option<signex_engine::PlacedGeometry>,
    /// Why its fields are disabled: the lock that holds it.
    pub locked: Option<String>,
    /// The box being typed in.
    pub draft: Option<PlacedDraft>,
}

impl PlacedPanel {
    /// The text of `field` of `item` while it is being typed, else `shown`.
    pub fn text(&self, item: SelectedItem, field: &PlacedField, shown: String) -> String {
        match &self.draft {
            Some(d) if d.item == item && &d.field == field => d.text.clone(),
            _ => shown,
        }
    }
}
