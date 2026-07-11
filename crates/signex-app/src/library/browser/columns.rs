//! Library Browser — grid column model + derivation.
//!
//! The `GridColumn` / `ColumnKind` types, the numeric-aware cell
//! comparator, `derive_columns` (always-on + template-derived +
//! most-common parametric columns), the lifecycle dot colour and the
//! search-filter predicate. Extracted verbatim from the former
//! single-file `browser` module.

use super::*;

pub(super) struct GridColumn {
    pub(super) label: String,
    pub(super) kind: ColumnKind,
    pub(super) width: f32,
}

pub(super) enum ColumnKind {
    InternalPn,
    Manufacturer,
    Mpn,
    /// Stage 14 — combined version + released indicator. Renders as
    /// `🔒 1.2.3` for released rows, `1.2.3` for unreleased. Sorts
    /// against the row's `version` cell so semver patterns sort
    /// numerically (1.0.2 vs 1.0.10 — `compare_cells` handles the
    /// pure-number case; mixed `1.0.2` strings fall back to lexical
    /// which still works for short major.minor.patch strings).
    Rev,
    /// Read-only column showing the row's bound symbol primitive.
    /// Empty cell (`—`) when the row's `symbol_ref` is `Uuid::nil()`
    /// (the sentinel for an unbound row). The actual binding edit
    /// surface is the Properties panel's "Pick Symbol…" button — the
    /// column gives an at-a-glance status across the whole table.
    /// F16 of the 2026-05-03 library polish ("the relevant columns
    /// must be there by default").
    Symbol,
    /// Read-only column showing the row's bound footprint primitive.
    /// Empty cell (`—`) when `footprint_ref` is `None` or its UUID is
    /// `Uuid::nil()`. Edited via Properties panel "Pick Footprint…".
    Footprint,
    /// Stage 18 — read-only column reading from `parameters["tags"]`.
    /// Inline-editable through the leftmost cell-edit buffer pattern
    /// is deferred to a polish pass; for now the canonical edit point
    /// is the Edit Component Details modal.
    Tags,
    Parameter(String),
}

impl ColumnKind {
    /// Stable sort key matching `LibraryMessage::BrowserSortColumn`'s
    /// `column_key` field. Ties columns to their cell-edit buffers
    /// and to the [`super::super::state::BrowserSort`] state.
    pub(super) fn sort_key(&self) -> String {
        match self {
            ColumnKind::InternalPn => "internal_pn".to_string(),
            ColumnKind::Manufacturer => "manufacturer".to_string(),
            ColumnKind::Mpn => "mpn".to_string(),
            ColumnKind::Rev => "version".to_string(),
            ColumnKind::Symbol => "symbol_ref".to_string(),
            ColumnKind::Footprint => "footprint_ref".to_string(),
            ColumnKind::Tags => "parameters.tags".to_string(),
            ColumnKind::Parameter(key) => format!("parameters.{key}"),
        }
    }

    /// Extract the row's cell value for this column. Empty string when
    /// the row has no value for the underlying field. For
    /// [`ColumnKind::Rev`], the value is the bare semver string (the
    /// 🔒/🔓 badge is added at render time, not in the sort key, so
    /// released and unreleased rows still sort by version order).
    pub(super) fn cell_value(&self, r: &ComponentRow) -> String {
        match self {
            ColumnKind::InternalPn => r.internal_pn.as_str().to_string(),
            ColumnKind::Manufacturer => r.primary_mpn.manufacturer.clone(),
            ColumnKind::Mpn => r.primary_mpn.mpn.clone(),
            ColumnKind::Rev => r.version.clone(),
            ColumnKind::Symbol => {
                if r.symbol_ref.uuid == uuid::Uuid::nil() {
                    "—".to_string()
                } else {
                    // Surface the short uuid prefix so the user has an
                    // at-a-glance signal without bloating the column
                    // width. Full path/name is in the Properties panel.
                    format!("• {:.8}", r.symbol_ref.uuid)
                }
            }
            ColumnKind::Footprint => match &r.footprint_ref {
                Some(fp) if fp.uuid != uuid::Uuid::nil() => {
                    format!("• {:.8}", fp.uuid)
                }
                _ => "—".to_string(),
            },
            ColumnKind::Tags => match r.parameters.get("tags") {
                Some(v) => v.display(),
                None => String::new(),
            },
            ColumnKind::Parameter(key) => match r.parameters.get(key) {
                Some(v) => v.display(),
                None => String::new(),
            },
        }
    }
}

/// Comparator for two cell strings with auto-detected numeric
/// fallback. If both values parse as `f64`, sort numerically;
/// otherwise sort case-insensitively. This is Stage 8's answer to
/// the Altium "lexical sort on numeric columns" pain — we don't
/// need a typed schema lookup at compare time, and untyped legacy
/// columns get the right behaviour automatically when their cells
/// happen to be numeric.
pub(super) fn compare_cells(a: &str, b: &str) -> std::cmp::Ordering {
    if let (Ok(na), Ok(nb)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        return na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal);
    }
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Resolve the column list. Always: Internal PN / Manufacturer / MPN /
/// Rev / Symbol / Footprint. Then template-derived columns from the
/// `TemplateRegistry`: every `required_param` slot from the templates
/// resolved for the table's classes (de-duplicated across classes).
/// Then a Tags column (Stage 18) when *any* row carries a non-empty
/// `parameters["tags"]`. Finally up to [`MAX_PARAM_COLUMNS`] of the
/// most-common other parametric keys across `rows` — `tags` and any
/// already-shown template params are excluded so columns don't render
/// twice.
///
/// Template resolution sources `class` from the rows when present;
/// for an empty table it strips a trailing "s" off `table_name` and
/// uses that as the implicit class (works for the default
/// pluralisation `resistor` → `resistors` etc.). F19 / F20 of the
/// 2026-05-03 library polish: the user wanted basic params per table
/// to appear by default, AND they want Tables to be the only
/// user-facing concept (Classes are now derived purely from the
/// table name, never edited directly).
pub(super) fn derive_columns(
    rows: &[ComponentRow],
    library_id: uuid::Uuid,
    registry: &signex_library::TemplateRegistry,
    table_name: &str,
) -> Vec<GridColumn> {
    let mut columns: Vec<GridColumn> = Vec::with_capacity(4 + MAX_PARAM_COLUMNS);
    columns.push(GridColumn {
        label: "Internal PN".to_string(),
        kind: ColumnKind::InternalPn,
        width: 130.0,
    });
    columns.push(GridColumn {
        label: "Manufacturer".to_string(),
        kind: ColumnKind::Manufacturer,
        width: 120.0,
    });
    columns.push(GridColumn {
        label: "MPN".to_string(),
        kind: ColumnKind::Mpn,
        width: 130.0,
    });

    // Stage 14: surface row revision + released-flag as a single
    // column. Rendered with a 🔒 prefix on released rows. Always
    // present so the user has a stable place to spot drift even
    // when no rows are flagged released yet.
    columns.push(GridColumn {
        label: "Rev".to_string(),
        kind: ColumnKind::Rev,
        width: 80.0,
    });

    // F16 (2026-05-03 library polish) — Symbol + Footprint binding
    // status is shown by default. `—` means unbound; short uuid
    // prefix means bound. Full primitive path/name + the Pick…
    // affordance live in the Properties panel for the selected row.
    columns.push(GridColumn {
        label: "Symbol".to_string(),
        kind: ColumnKind::Symbol,
        width: 120.0,
    });
    columns.push(GridColumn {
        label: "Footprint".to_string(),
        kind: ColumnKind::Footprint,
        width: 120.0,
    });

    // F19 — template-derived basic-parameter columns. Resolve unique
    // classes from the rows; for empty tables, strip a trailing "s"
    // off the table name to derive an implicit class (works for the
    // default pluralisation, falls through harmlessly otherwise).
    // Each template's `required_params` becomes a column with
    // "<name> (<unit>)" label so users see the canonical units up
    // front. Already-added param keys are skipped to dedupe across
    // classes.
    let mut classes: std::collections::BTreeSet<String> =
        rows.iter().map(|r| r.class.as_str().to_string()).collect();
    if classes.is_empty() {
        if let Some(stem) = table_name.strip_suffix('s') {
            classes.insert(stem.to_string());
        }
    }
    for class in &classes {
        if let Some(tmpl) = registry.resolve(library_id, class) {
            for slot in &tmpl.required_params {
                let already = columns
                    .iter()
                    .any(|c| matches!(&c.kind, ColumnKind::Parameter(k) if k == &slot.name));
                if already {
                    continue;
                }
                // Label is the slot name only — no `(unit)` suffix.
                // Units vary per row (a "value" column holds 10kΩ in
                // resistors, 4.7µF in capacitors), so the column
                // header must be unit-agnostic; the cell renders the
                // unit inline via `ParamValue::Measurement.display()`.
                // Capitalises the first letter so "value" → "Value" /
                // "tolerance" → "Tolerance" without bringing a
                // heavy-weight casing crate in.
                let label = {
                    let mut chars = slot.name.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                };
                columns.push(GridColumn {
                    label,
                    kind: ColumnKind::Parameter(slot.name.clone()),
                    width: 110.0,
                });
            }
        }
    }

    // Surface tags as a first-class column whenever the table has at
    // least one tagged row. Saves the user from having to scroll
    // sideways through the auto-derived parameter columns to find them.
    let any_tagged = rows
        .iter()
        .any(|r| matches!(r.parameters.get("tags"), Some(v) if !v.display().is_empty()));
    if any_tagged {
        columns.push(GridColumn {
            label: "Tags".to_string(),
            kind: ColumnKind::Tags,
            width: 160.0,
        });
    }

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        for k in row.parameters.keys() {
            if k == "tags" {
                // Already surfaced via the dedicated Tags column.
                continue;
            }
            *counts.entry(k.clone()).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (key, _) in sorted.into_iter().take(MAX_PARAM_COLUMNS) {
        columns.push(GridColumn {
            label: shorten_label(&key),
            kind: ColumnKind::Parameter(key),
            width: 90.0,
        });
    }
    columns
}

/// Per-lifecycle indicator dot colour. Matches plan §6:
///
/// * Released → green;
/// * Draft / InReview → neutral grey ("active, but not preferred");
/// * Deprecated → amber/yellow;
/// * Obsolete → muted dark grey.
///
/// Centralised here so both the dot and any future lifecycle badge
/// in the side preview pane can pull the same colour.
pub(super) fn lifecycle_dot_color(state: LifecycleState) -> iced::Color {
    match state {
        LifecycleState::Released => iced::Color::from_rgb(0.30, 0.78, 0.40),
        LifecycleState::InReview => iced::Color::from_rgb(0.50, 0.65, 0.95),
        LifecycleState::Draft => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.45),
        LifecycleState::Deprecated => iced::Color::from_rgb(0.96, 0.78, 0.10),
        LifecycleState::Obsolete => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.20),
        // `LifecycleState` is `#[non_exhaustive]` — fall back to the
        // muted "Draft" colour for any future state we haven't styled yet.
        _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.30),
    }
}

fn shorten_label(key: &str) -> String {
    if key.len() <= 14 {
        key.to_string()
    } else {
        format!("{}…", &key[..13])
    }
}

pub(super) fn row_matches_filter(r: &ComponentRow, needle: &str) -> bool {
    if r.internal_pn.as_str().to_lowercase().contains(needle) {
        return true;
    }
    if r.primary_mpn.manufacturer.to_lowercase().contains(needle) {
        return true;
    }
    if r.primary_mpn.mpn.to_lowercase().contains(needle) {
        return true;
    }
    false
}
