//! The kind chips on one line (mockup row 119). At the panel's width the
//! chips give up side padding, down to a floor, before they would wrap; see
//! docs/DECISIONS.md, the 2026-11-26 Library panel kind chips row (#3007).

use iced::Font;
use iced::font::Weight;
use signex_widgets::chips::ChipMetrics;

use crate::dock::measure::text_width_in;

/// Width of a chip's content: its label, the gap and its count, measured in
/// the fonts the chip draws them in (the label medium when the chip is on,
/// the count in mono caption). A hairline per chip absorbs shaping rounding.
pub(super) fn content_width(label: &str, count: &str, is_on: bool, metrics: ChipMetrics) -> f32 {
    let weight = if is_on {
        Weight::Medium
    } else {
        Weight::Normal
    };
    let label_w = text_width_in(
        label,
        metrics.text_size,
        signex_widgets::ui_font::at(weight),
    );
    let count_w = text_width_in(count, metrics.caption_size, Font::MONOSPACE);
    label_w + metrics.gap + count_w + crate::styles::HAIRLINE
}

/// The side padding each chip gets so chips whose contents are `contents`
/// wide, `spacing` apart, fit `available` on one line: `full` when they fit
/// as drawn, else what the line leaves, never less than `least`.
pub(super) fn fitted_pad(
    contents: &[f32],
    spacing: f32,
    full: f32,
    least: f32,
    available: f32,
) -> f32 {
    if contents.is_empty() || !available.is_finite() {
        return full;
    }
    let n = contents.len() as f32;
    let fixed = contents.iter().sum::<f32>() + spacing * (n - 1.0);
    let room = (available - fixed) / (2.0 * n);
    room.clamp(least.min(full), full)
}

#[cfg(test)]
mod tests {
    use super::fitted_pad;

    /// Chips that fit keep the padding they are drawn with.
    #[test]
    fn chips_that_fit_keep_their_full_padding() {
        assert_eq!(fitted_pad(&[56.0, 68.0, 46.0], 4.0, 8.0, 4.0, 300.0), 8.0);
    }

    /// The default 240 px dock: the line is a few px short, so the padding
    /// shrinks just enough and the chips fill the line exactly.
    #[test]
    fn a_line_a_few_px_short_takes_it_from_the_padding() {
        let contents = [56.0, 68.0, 46.0];
        let pad = fitted_pad(&contents, 4.0, 8.0, 4.0, 222.0);
        assert!(pad < 8.0 && pad > 4.0, "{pad}");
        let line: f32 = contents.iter().map(|c| c + 2.0 * pad).sum::<f32>() + 2.0 * 4.0;
        assert!((line - 222.0).abs() < 0.01, "{line}");
    }

    /// Far too narrow: the padding stops at its floor.
    #[test]
    fn the_padding_never_goes_under_its_floor() {
        assert_eq!(fitted_pad(&[56.0, 68.0, 46.0], 4.0, 8.0, 4.0, 100.0), 4.0);
        assert_eq!(fitted_pad(&[], 4.0, 8.0, 4.0, 10.0), 8.0);
        assert_eq!(
            fitted_pad(&[56.0], 4.0, 8.0, 4.0, f32::INFINITY),
            8.0,
            "an unbounded width keeps the full padding"
        );
    }
}
