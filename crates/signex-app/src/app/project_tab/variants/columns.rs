//! The widths of the Variant Manager matrix's columns (#3037, mockup row
//! 32). A variant's column is never narrower than its heading (the name and
//! the active pill) or its widest cell (Not fitted, Fitted, or "mixed" on a
//! repeated sheet), measured in the UI font, so a pill never wraps or is
//! cut. Value · Footprint gives up room first, down to a floor; past that
//! the matrix scrolls sideways.

use std::collections::HashMap;

use iced::Length;
use signex_i18n::{MessageId, ids};
use signex_output::VariantMatrix;
use signex_types::pcb::SymbolLink;

use super::super::table::{SP, TS};
use super::{Manager, PILL_PAD_RATIO, medium};
use crate::dock::measure::text_width_in;

/// Width of the Part column, in `body`-size characters.
const PART_EMS: f32 = 58.0 / 12.0;
/// Widest the Value · Footprint column grows to, in `body`-size characters.
const VALUE_EMS: f32 = 150.0 / 12.0;
/// Narrowest the Value · Footprint column gives way to, in `body`-size
/// characters.
const VALUE_MIN_EMS: f32 = 72.0 / 12.0;
/// Room left over a measured width so a shaping rounding never cuts it, px.
const SLACK: f32 = 2.0;

/// The matrix's column widths.
pub(super) struct Columns {
    /// The least width of each variant's column, Base first, in px.
    variants: Vec<f32>,
}

impl Columns {
    /// The widths `m`'s matrix needs for `matrix`.
    pub(super) fn of(m: &Manager<'_>, matrix: &VariantMatrix) -> Self {
        let cell = cell_width(m, matrix);
        let variants = m
            .variants()
            .map(|variant| heading_width(m, variant).max(cell))
            .collect();
        Self { variants }
    }

    /// The Part column.
    pub(super) fn part() -> Length {
        Length::Fixed(TS.body * PART_EMS)
    }

    /// The Value · Footprint column: it shares the room left over, up to
    /// its widest, and is the first to give way.
    pub(super) fn value() -> Length {
        Length::Fill
            .min(TS.body * VALUE_MIN_EMS)
            .max(TS.body * VALUE_EMS)
    }

    /// The column of the `index`th variant (Base is 0).
    pub(super) fn variant(&self, index: usize) -> Length {
        Length::Fill.min(self.variants.get(index).copied().unwrap_or(0.0))
    }

    /// The least width of the whole matrix: wider than its room, it scrolls.
    pub(super) fn least(&self) -> f32 {
        TS.body * (PART_EMS + VALUE_MIN_EMS) + self.variants.iter().sum::<f32>()
    }
}

/// `id`'s words at `size` in the medium face, in px.
fn words_width(m: &Manager<'_>, id: MessageId, size: f32) -> f32 {
    text_width_in(&m.i18n.message_plain(id, &[]), size, medium())
}

/// A pill of `words` px of text: its side padding added.
fn pill_width(words: f32) -> f32 {
    words + 2.0 * SP.m * PILL_PAD_RATIO
}

/// A heading cell: its padding, the name, and the active pill after it.
fn heading_width(m: &Manager<'_>, variant: Option<&str>) -> f32 {
    let name = m.i18n.upper_for_display(&m.name(variant));
    let mut width = text_width_in(&name, TS.section, medium());
    if m.is_active(variant) {
        width += SP.s + pill_width(words_width(m, ids::PROJECT_VARIANTS_ACTIVE, TS.caption));
    }
    2.0 * SP.m + width + SLACK
}

/// The widest cell any column shows: its padding and the widest pill.
fn cell_width(m: &Manager<'_>, matrix: &VariantMatrix) -> f32 {
    let fit_words = [
        ids::PROJECT_VARIANTS_FITTED,
        ids::PROJECT_VARIANTS_NOT_FITTED,
    ]
    .into_iter()
    .map(|id| words_width(m, id, TS.caption))
    .fold(0.0, f32::max);
    // The glyph, the gap after it and the words.
    let mut pill = pill_width(TS.caption + SP.xs + fit_words);
    if let Some(copies) = most_channel_copies(matrix) {
        let mixed = m.i18n.message_plain(
            ids::PROJECT_VARIANTS_CHANNEL_MIXED,
            &[("count", copies.into()), ("total", copies.into())],
        );
        pill = pill.max(pill_width(text_width_in(&mixed, TS.caption, medium())));
    }
    2.0 * SP.m + pill + SLACK
}

/// The most copies a part on a repeated sheet has, `None` when no part is
/// on one.
fn most_channel_copies(matrix: &VariantMatrix) -> Option<usize> {
    let mut copies: HashMap<&SymbolLink, usize> = HashMap::new();
    for channel in matrix.rows.iter().filter_map(|row| row.channel.as_ref()) {
        *copies.entry(&channel.part).or_default() += 1;
    }
    copies.into_values().max()
}
