//! Sizes of the Layers panel, as the Graphite mockup draws them at the 12 px
//! base (panels.html, section A), scaled with the base through `chrome_px`.

use crate::styles::{ChromeMetrics, chrome_px};

/// `at_base` px of the mockup, at the current base size.
const fn px(at_base: f32) -> f32 {
    chrome_px(at_base * 11.0 / 12.0)
}

/// A layer row, a pair row and a System Colors row.
pub(super) const ROW: f32 = ChromeMetrics::DEFAULT.row;
/// The search field, the more button beside it.
pub(super) const FIELD: f32 = ChromeMetrics::DEFAULT.field;
/// The All Layers row.
pub(super) const ALL_LAYERS_ROW: f32 = px(22.0);
/// A section header.
pub(super) const SECTION_HEAD: f32 = px(26.0);
/// The Active | Set card.
pub(super) const ACTIVE_CARD: f32 = px(28.0);
/// The divider between the active layer and the set in the Active card.
pub(super) const DIVIDER: f32 = px(16.0);
/// The square of an eye button, and of a small icon button.
pub(super) const EYE: f32 = px(20.0);
/// An icon inside a button.
pub(super) const ICON: f32 = px(14.0);
/// A colour dot.
pub(super) const DOT: f32 = px(12.0);
/// The ring of an open or hovered dot, and the gap inside it.
pub(super) const DOT_RING: f32 = px(2.0);
pub(super) const DOT_GAP: f32 = px(1.0);
/// The copper number column.
pub(super) const NUMBER: f32 = px(12.0);
/// The gap between the parts of a row.
pub(super) const ROW_GAP: f32 = px(6.0);
/// The gap between an eye and a dot in a pair side.
pub(super) const SIDE_GAP: f32 = px(4.0);
/// The accent bar left of the active row, and its inset from the row's
/// top and bottom.
pub(super) const ACCENT_BAR: f32 = px(2.0);
pub(super) const ACCENT_BAR_INSET: f32 = px(4.0);
/// The Top and the Bottom column of the pairs table.
pub(super) const PAIR_SIDE: f32 = px(44.0);
/// A count pill and the Active pill.
pub(super) const PILL: f32 = px(16.0);
pub(super) const PILL_PAD: f32 = px(6.0);
/// The more menu card.
pub(super) const MENU: f32 = px(224.0);
/// The colour card: its width, its indent to the name column, its padding.
pub(super) const CARD: f32 = px(212.0);
pub(super) const CARD_INDENT: f32 = px(52.0);
pub(super) const CARD_PAD: f32 = px(10.0);
/// The palette grid: columns, gap, and the side of a swatch that fills the
/// card's width.
pub(super) const SWATCH_COLUMNS: usize = 8;
pub(super) const SWATCH_GAP: f32 = px(5.0);
pub(super) const SWATCH: f32 =
    (CARD - 2.0 * CARD_PAD - (SWATCH_COLUMNS as f32 - 1.0) * SWATCH_GAP) / SWATCH_COLUMNS as f32;
pub(super) const SWATCH_RADIUS: f32 = px(5.0);
/// The hex field and its preview swatch.
pub(super) const HEX_FIELD: f32 = px(24.0);
/// Padding of the "No layers match" line.
pub(super) const EMPTY_PAD_Y: f32 = px(14.0);
pub(super) const EMPTY_PAD_X: f32 = px(12.0);
/// Alpha of a hidden layer's dot.
pub(super) const HIDDEN_DOT_ALPHA: f32 = 0.45;
