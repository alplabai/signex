//! Schematic defaults section of the Project tab (#2026): the widths and
//! sizes unset schematic items draw, export and are placed at, saved in the
//! `.snxprj` `schematic_defaults` key. An empty field is the built-in value.

mod view;

pub(super) use view::view;

use signex_types::project::SchematicDefaultField;

/// The number typed in a field: empty or not a number above zero stores no
/// override, so the built-in value applies; a number out of the field's
/// range is stored clamped into it.
pub(super) fn parse_override(field: SchematicDefaultField, text: &str) -> Option<f64> {
    crate::numeric_field::parse_field_f64(text)
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| field.clamped(v))
}

/// The note under a field whose typed number was clamped: what was stored
/// instead, and the range. `None` when the typed text is in range.
pub(super) fn clamp_note(field: SchematicDefaultField, draft: Option<&String>) -> Option<String> {
    let typed = crate::numeric_field::parse_field_f64(draft?)?;
    let stored = parse_override(field, draft?)?;
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
