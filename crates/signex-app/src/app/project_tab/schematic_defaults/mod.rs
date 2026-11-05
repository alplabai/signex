//! Schematic defaults section of the Project tab (#2026): the widths and
//! sizes unset schematic items draw, export and are placed at, saved in the
//! `.snxprj` `schematic_defaults` key. An empty field is the built-in value.

mod view;

pub(super) use view::view;

use signex_types::project::SchematicDefaultField;

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

/// The note under a field whose typed number was clamped: what was stored
/// instead, and the range. `None` when the typed text is in range.
pub(super) fn clamp_note(field: SchematicDefaultField, draft: Option<&String>) -> Option<String> {
    let typed = crate::numeric_field::parse_field_f64(draft?)?;
    let DefaultInput::Set(stored) = parse_override(field, draft?) else {
        return None;
    };
    (typed != stored).then(|| {
        let range = field.range();
        format!(
            "Out of range ({} to {}): {stored} is used.",
            range.start(),
            range.end()
        )
    })
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
