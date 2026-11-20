//! Properties of a multi-selection (#1258, #1699): the fields every selected
//! object of a kind shares, editable at once, one section per kind under
//! kind chips that choose which sections show. The grouping and the bulk
//! edit live in `signex_engine::multi_edit`; this is the view, the text
//! parsing and the form's view state.

mod controls;
mod form_state;
mod view;

#[cfg(test)]
mod tests;

pub(in crate::panels) use controls::{
    Cell, PairField, control_row, counted_section, hover_linked, illustration_view, inset_caption,
    pair_row, segmented_cells,
};
pub use form_state::{MultiKind, SelectionForm, SelectionFormMsg, SelectionKinds, rule_matches};
pub use view::view_multi_selection;

use crate::numeric_field::{FieldNumber, parse_field_number};
use signex_engine::{MultiEdit, Shared};
use signex_types::coord::Unit;
use signex_types::schematic::SCHEMATIC_PT_TO_MM;

/// Shown in place of a value the selection disagrees on. The view reads
/// `placeholder-mixed` from the catalog; the tests check it says this.
#[cfg(test)]
pub const MIXED: &str = "<mixed>";

/// A text box of the multi-selection form, the key of its draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiField {
    SymbolValue,
    SymbolFootprint,
    SymbolParameter(String),
    LabelFontPt,
    /// The typed width of every selected wire, in the panel's unit.
    WireWidth,
}

impl MultiField {
    /// How the field is named in a refusal line.
    fn name(&self) -> String {
        match self {
            Self::SymbolValue => "Value".into(),
            Self::SymbolFootprint => "Footprint".into(),
            Self::SymbolParameter(key) => format!("Parameter {key}"),
            Self::LabelFontPt => "Label font size".into(),
            Self::WireWidth => "Wire width".into(),
        }
    }
}

/// Multi-selection form messages, namespaced under `PanelMsg::MultiEdit`.
#[derive(Debug, Clone)]
pub enum MultiEditMsg {
    /// A keystroke in a text box; applied on Enter, not per key, so one
    /// edit is one undo step.
    DraftChanged(MultiField, String),
    /// Enter in a text box.
    DraftSubmitted(MultiField),
    /// A checkbox or segmented button: applied at once.
    Picked(MultiEdit),
}

/// The edit the text `typed` in `field` asks for, a length read in mm, or
/// why it cannot apply.
#[cfg(test)]
pub fn edit_from_text(field: &MultiField, typed: &str) -> Result<MultiEdit, String> {
    edit_from_text_in(field, typed, Unit::Mm)
}

/// [`edit_from_text`] with a length read in `unit`, the panel's unit.
pub fn edit_from_text_in(field: &MultiField, typed: &str, unit: Unit) -> Result<MultiEdit, String> {
    let number = || match parse_field_number(typed) {
        FieldNumber::Number(v) => Ok(v),
        FieldNumber::NotANumber | FieldNumber::Refused => {
            Err(format!("{}: '{typed}' is not a number", field.name()))
        }
    };
    let non_negative = |what: &str| {
        number().and_then(|v| {
            (v >= 0.0)
                .then_some(v)
                .ok_or_else(|| format!("{what} cannot be negative, got '{typed}'"))
        })
    };
    Ok(match field {
        MultiField::SymbolValue => MultiEdit::SymbolValue(typed.to_owned()),
        MultiField::SymbolFootprint => MultiEdit::SymbolFootprint(typed.to_owned()),
        MultiField::SymbolParameter(key) => MultiEdit::SymbolParameter {
            key: key.clone(),
            value: typed.to_owned(),
        },
        MultiField::LabelFontPt => {
            let pt = non_negative("Label font size")?;
            if pt <= 0.0 {
                return Err(format!("Label font size must be above 0, got '{typed}'"));
            }
            MultiEdit::LabelFontSizeMm(pt * SCHEMATIC_PT_TO_MM)
        }
        MultiField::WireWidth => MultiEdit::WireWidthMm(
            non_negative("Wire width")? * signex_types::measure::mm_per(unit),
        ),
    })
}

/// A number without trailing zeros (`1.27`, `10`).
fn number_text(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn shared_text(shared: &Shared<String>) -> String {
    shared.same().cloned().unwrap_or_default()
}

fn shared_number(shared: &Shared<f64>, scale: f64) -> String {
    shared
        .same()
        .map(|v| number_text(v / scale))
        .unwrap_or_default()
}
