//! Shared pieces of the per-kind Properties builders: the colours every
//! builder is handed and the selection-info lookups.

use super::super::*;

/// The colours a Properties builder draws with.
#[derive(Clone, Copy)]
pub(super) struct PanelStyle {
    pub muted: Color,
    pub primary: Color,
    pub border_c: Color,
    pub palette: PanelPalette,
}

/// Rotation, in degrees, read from the panel's display text (`90°`). A
/// missing, malformed or non-finite value reads as `0`. Pure: this runs
/// while the panel is built, so no refusal is logged here.
pub(super) fn rotation_degrees(text: &str) -> f64 {
    signex_types::numeric::parse_finite_f64(text.trim_end_matches('°')).unwrap_or(0.0)
}

/// Rotation, in degrees, from a rotation pick-list label (`90 Degrees`).
pub(super) fn pick_list_degrees(label: &str) -> f64 {
    rotation_degrees(label.split_whitespace().next().unwrap_or(""))
}

/// The row `key` of a selection's info, empty when the selection has none.
pub(super) fn info_row(info: &[(String, String)], key: &str) -> String {
    info.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// The horizontal justification row of a selection (#2024): the one lookup
/// every label, text note and symbol field panel shows it from.
pub(super) fn justify_h_row(info: &[(String, String)]) -> String {
    info_row(info, signex_engine::info_keys::JUSTIFY_H)
}

/// The vertical justification row of a selection (#2024).
pub(super) fn justify_v_row(info: &[(String, String)]) -> String {
    info_row(info, signex_engine::info_keys::JUSTIFY_V)
}

/// The justification a Properties row reports (`info_keys::JUSTIFY_H`).
pub(super) fn h_align_from_row(row: &str) -> signex_types::schematic::HAlign {
    use signex_types::schematic::HAlign;
    match row {
        "Left" => HAlign::Left,
        "Right" => HAlign::Right,
        _ => HAlign::Center,
    }
}
