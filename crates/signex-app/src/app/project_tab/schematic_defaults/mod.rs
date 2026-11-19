//! Schematic defaults section of the Project tab (#2026): the widths and
//! sizes unset schematic items draw, export and are placed at, saved in the
//! `.snxprj` `schematic_defaults` key. An empty field is the built-in value.
//! The look (two cards, unit suffixes, field states, live preview) is the
//! Graphite mockup m36; see docs/PROJECT_TAB.md.

mod field;
mod preview;
mod view;

#[cfg(test)]
pub(super) use preview::{LabelLook, value_labels};
pub(super) use view::view;

use signex_types::project::{SchematicDefaultField, SchematicDefaultOverrides};

/// What a field's typed text does to its stored default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum DefaultInput {
    /// Empty text: clear the override, the built-in value applies.
    Clear,
    /// A number above zero, clamped into the field's range.
    Set(f64),
    /// Mid-edit, not a number, or not above zero: keep the draft text and
    /// change nothing (the house rule in `numeric_field`).
    Keep,
}

/// Read a field's typed text.
pub(super) fn parse_override(field: SchematicDefaultField, text: &str) -> DefaultInput {
    if text.trim().is_empty() {
        return DefaultInput::Clear;
    }
    match crate::numeric_field::parse_field_f64(text) {
        Some(v) if v > 0.0 => DefaultInput::Set(field.clamped(v)),
        _ => DefaultInput::Keep,
    }
}

/// A typed number that was clamped: the field's range and the value stored
/// instead. The view words it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ClampNote {
    pub min: f64,
    pub max: f64,
    pub used: f64,
}

/// The note under a field whose typed number was clamped. `None` when the
/// typed text is in range, empty or not a usable number.
pub(super) fn clamp_note(
    field: SchematicDefaultField,
    draft: Option<&String>,
) -> Option<ClampNote> {
    let typed = crate::numeric_field::parse_field_f64(draft?)?;
    let DefaultInput::Set(used) = parse_override(field, draft?) else {
        return None;
    };
    let range = field.range();
    (typed != used).then_some(ClampNote {
        min: *range.start(),
        max: *range.end(),
        used,
    })
}

/// How a numeric field box looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldState {
    /// No override: the built-in value shows as the grey placeholder.
    BuiltIn,
    /// A stored override: accent border and a reset button.
    Edited,
    /// The typed text is not a usable number, or it was clamped: err
    /// border, err-soft fill and a warning icon, plus the reset button.
    Invalid,
}

/// The state of `field`'s box from its draft text and stored override.
pub(super) fn field_state(
    field: SchematicDefaultField,
    draft: Option<&String>,
    stored: Option<f64>,
) -> FieldState {
    let typed = draft.filter(|text| !text.trim().is_empty());
    if let Some(text) = typed {
        let unusable = parse_override(field, text) == DefaultInput::Keep;
        if unusable || clamp_note(field, Some(text)).is_some() {
            return FieldState::Invalid;
        }
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
