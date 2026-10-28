//! Library Browser — grid column model + derivation.
//!
//! The `GridColumn` / `ColumnKind` types, the numeric-aware cell
//! comparator, `derive_columns` (always-on + template-derived +
//! most-common parametric columns), the lifecycle dot colour and the
//! search-filter predicate. Extracted verbatim from the former
//! single-file `browser` module.

use super::sort::CellSortKey;
use super::*;
use crate::library::edit_row_modal::short_uuid;

#[derive(Debug, Clone)]
pub(super) struct GridColumn {
    pub(super) label: String,
    pub(super) kind: ColumnKind,
    pub(super) width: f32,
}

#[derive(Debug, Clone)]
pub(super) enum ColumnKind {
    InternalPn,
    Manufacturer,
    Mpn,
    /// Stage 14 — combined version + released indicator. Renders as
    /// `🔒 1.2.3` for released rows, `1.2.3` for unreleased. Sorts
    /// against the row's `version` cell so semver patterns sort
    /// numerically (1.0.2 vs 1.0.10 — `CellSortKey` handles the
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
    /// Read-only cached distributor price (qty-1 break, currency, age).
    /// Present only when some row of the table has a cached price.
    Price,
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
            ColumnKind::Price => "price".to_string(),
            ColumnKind::Parameter(key) => format!("parameters.{key}"),
        }
    }

    /// The row's sort key for this column. A parameter orders by its
    /// `ParamValue` (a measurement by base-unit quantity); every other
    /// column orders by its cell text.
    pub(super) fn sort_key_for(&self, r: &ComponentRow) -> CellSortKey {
        match self {
            ColumnKind::Parameter(key) => match r.parameters.get(key) {
                Some(value) => CellSortKey::from_param(value),
                None => CellSortKey::from_cell(""),
            },
            _ => CellSortKey::from_cell(&self.cell_value(r)),
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
                if r.is_symbol_unbound() {
                    "—".to_string()
                } else {
                    // Surface the short uuid prefix so the user has an
                    // at-a-glance signal without bloating the column
                    // width. Full path/name is in the Properties panel.
                    format!("• {}", short_uuid(&r.symbol_ref.uuid))
                }
            }
            ColumnKind::Footprint => match &r.footprint_ref {
                Some(fp) if !r.is_footprint_unbound() => {
                    format!("• {}", short_uuid(&fp.uuid))
                }
                _ => "—".to_string(),
            },
            ColumnKind::Tags => match r.parameters.get("tags") {
                Some(v) => v.display(),
                None => String::new(),
            },
            // The price lives in the pricing cache, not on the row; see
            // `PriceView`.
            ColumnKind::Price => String::new(),
            ColumnKind::Parameter(key) => match r.parameters.get(key) {
                Some(v) => v.display(),
                None => String::new(),
            },
        }
    }
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
    prices: &super::price::PriceView<'_>,
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
        width: 140.0,
    });
    columns.push(GridColumn {
        label: "Footprint".to_string(),
        kind: ColumnKind::Footprint,
        width: 140.0,
    });

    // Cached distributor price, once Refresh All Pricing has stored one.
    if prices.any(rows) {
        columns.push(GridColumn {
            label: "Price".to_string(),
            kind: ColumnKind::Price,
            width: 210.0,
        });
    }

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
    if classes.is_empty()
        && let Some(stem) = table_name.strip_suffix('s')
    {
        classes.insert(stem.to_string());
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
        format!("{}…", crate::text_trunc::prefix_within_bytes(key, 13))
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

#[cfg(test)]
mod short_uuid_tests {
    use super::short_uuid;

    #[test]
    fn short_uuid_is_eight_chars() {
        let id = uuid::Uuid::parse_str("12345678-9abc-def0-1234-56789abcdef0").unwrap();
        assert_eq!(short_uuid(&id), "12345678");
    }
}

#[cfg(test)]
mod sort_tests {
    use super::*;
    use signex_library::{
        ComponentClass, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart, ParamMap,
        ParamValue, PlmReserved, PrimitiveRef,
    };

    fn row_with(value: f64, unit: &str) -> ComponentRow {
        let now = chrono::Utc::now();
        let mut parameters = ParamMap::new();
        parameters.insert(
            "value".into(),
            ParamValue::Measurement {
                value,
                unit: unit.into(),
            },
        );
        ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: uuid::Uuid::new_v4(),
            internal_pn: InternalPn::new("R1"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(uuid::Uuid::nil(), uuid::Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "MPN"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters,
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: now,
            updated: now,
            content_hash: [0u8; 32],
            server_revision: None,
        }
    }

    fn param_row(value: ParamValue) -> ComponentRow {
        let mut r = row_with(0.0, "ohm");
        r.parameters.insert("value".into(), value);
        r
    }

    fn measured(value: f64, unit: &str) -> ParamValue {
        ParamValue::Measurement {
            value,
            unit: unit.into(),
        }
    }

    /// Sort `values` through the browser's own key path and return the
    /// display text of each, in order.
    fn sorted_display(values: Vec<ParamValue>) -> Vec<String> {
        let column = ColumnKind::Parameter("value".into());
        let mut rows: Vec<ComponentRow> = values.into_iter().map(param_row).collect();
        super::super::sort::sort_by_cell_key(&mut rows, |r| column.sort_key_for(r), false);
        rows.iter().map(|r| column.cell_value(r)).collect()
    }

    #[test]
    fn two_kilohm_sorts_before_ten_kilohm() {
        let out = sorted_display(vec![
            measured(10.0, "kΩ"),
            measured(900.0, "ohm"),
            measured(2000.0, "ohm"),
        ]);
        assert_eq!(out, ["900 Ω", "2 kΩ", "10 kΩ"]);
    }

    #[test]
    fn a_hundred_nanofarad_sorts_before_a_microfarad() {
        let out = sorted_display(vec![measured(1.0, "µF"), measured(100.0, "nF")]);
        assert_eq!(out, ["100 nF", "1 µF"]);
        let out = sorted_display(vec![measured(1.0, "µF"), measured(10.0, "nF")]);
        assert_eq!(out, ["10 nF", "1 µF"]);
    }

    #[test]
    fn legacy_and_canonical_measurements_sit_together() {
        let out = sorted_display(vec![
            measured(10_000.0, "ohm"),
            measured(4700.0, "ohm"),
            measured(4.7, "kΩ"),
            measured(900.0, "ohm"),
        ]);
        assert_eq!(out, ["900 Ω", "4.7 kΩ", "4.7 kΩ", "10 kΩ"]);
    }

    #[test]
    fn numbers_precede_measurements_and_text_comes_last() {
        let out = sorted_display(vec![
            ParamValue::Text("R10".into()),
            measured(5.0, "V"),
            ParamValue::Text("R2".into()),
            ParamValue::Number(100.0),
        ]);
        assert_eq!(out, ["100", "5 V", "R2", "R10"]);
    }

    #[test]
    fn mixed_unit_families_cluster() {
        let out = sorted_display(vec![
            measured(1.0, "µF"),
            measured(10.0, "kΩ"),
            measured(100.0, "nF"),
            measured(900.0, "ohm"),
        ]);
        assert_eq!(out, ["900 Ω", "10 kΩ", "100 nF", "1 µF"]);
    }
}
