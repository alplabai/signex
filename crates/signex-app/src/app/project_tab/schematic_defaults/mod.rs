//! Schematic defaults section of the Project tab (#2026): the widths and
//! sizes unset schematic items draw, export and are placed at, saved in the
//! `.snxprj` `schematic_defaults` key. An empty field is the built-in value.
//! The look (two cards, unit suffixes, field states, live preview) is the
//! mockup row 36; see docs/PROJECT_TAB.md.

mod field;
mod preview;
mod view;

#[cfg(test)]
pub(super) use preview::{LabelLook, value_labels};
pub(super) use view::view;

use signex_types::numeric::{FiniteParseError, parse_finite_f64};
use signex_types::project::{SchematicDefaultField, SchematicDefaultOverrides};

/// What a field's typed text does to its stored default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum DefaultInput {
    /// Empty text: clear the override, the built-in value applies.
    Clear,
    /// A number inside the field's range.
    Set(f64),
    /// Mid-edit, not a number, or outside the field's range: keep the draft
    /// text and change nothing (the house rule in `numeric_field`).
    Keep,
}

/// Read a field's typed text.
pub(super) fn parse_override(field: SchematicDefaultField, text: &str) -> DefaultInput {
    if text.trim().is_empty() {
        return DefaultInput::Clear;
    }
    match crate::numeric_field::parse_field_f64(text) {
        Some(v) if field.range().contains(&v) => DefaultInput::Set(v),
        _ => DefaultInput::Keep,
    }
}

/// The range a typed number was refused for. `None` when the text is in
/// range, empty or not a number. Silent: the view asks it every frame.
pub(super) fn refused_range(
    field: SchematicDefaultField,
    draft: Option<&String>,
) -> Option<(f64, f64)> {
    let range = field.range();
    let outside = match parse_finite_f64(draft?) {
        Ok(typed) => !range.contains(&typed),
        Err(FiniteParseError::NonFinite(_)) => true,
        Err(FiniteParseError::NotANumber(_)) => false,
    };
    outside.then_some((*range.start(), *range.end()))
}

/// How a numeric field box looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldState {
    /// No override: the built-in value shows as the grey placeholder.
    BuiltIn,
    /// A stored override: accent border and a reset button.
    Edited,
    /// The typed text is refused: err border, err-soft fill and a warning
    /// icon, plus the reset button.
    Invalid,
}

/// The state of `field`'s box from its draft text and stored override.
pub(super) fn field_state(
    field: SchematicDefaultField,
    draft: Option<&String>,
    stored: Option<f64>,
) -> FieldState {
    let typed = draft.filter(|text| !text.trim().is_empty());
    if let Some(text) = typed
        && parse_override(field, text) == DefaultInput::Keep
    {
        return FieldState::Invalid;
    }
    if stored.is_some() {
        FieldState::Edited
    } else {
        FieldState::BuiltIn
    }
}

/// The value `field` falls back to when its own box is empty: the built-in
/// table, or for symbol lines the wire width they follow.
pub(super) fn built_in_value(
    field: SchematicDefaultField,
    overrides: &SchematicDefaultOverrides,
) -> f64 {
    field.resolved(&overrides.with(field, None).resolve())
}

/// A default as the section shows it: at most three decimals, no trailing
/// zeros. Lengths keep `.` in every locale.
pub(super) fn shown_number(value: f64) -> String {
    format!("{}", (value * 1000.0).round() / 1000.0)
}

/// What a field box shows: the half-typed draft, else the stored override,
/// else nothing (the placeholder shows the value in effect).
fn shown_text(draft: Option<&String>, stored: Option<f64>) -> String {
    match (draft, stored) {
        (Some(draft), _) => draft.clone(),
        (None, Some(value)) => format!("{value}"),
        (None, None) => String::new(),
    }
}
