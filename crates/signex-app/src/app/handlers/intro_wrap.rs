//! How much taller a fixed-height dialog window gets when the text it shows
//! wraps onto more lines than the text its base height was measured with.
//! The text is user data (a library name, a part number, a table), so the
//! window follows its wrapped line count (docs/DETACHED_MODALS.md).

use crate::styles::{DIALOG_ICON_TILE, LINE_HEIGHT, Spacing, TypeScale};

/// Average width of one character, in em. Deliberately a little wide so the
/// count errs towards one line too many (a spare line of window) and never
/// one too few (a footer pushed out of the window).
const AVG_CHAR_EM: f32 = 0.58;

/// A text and the text its window height was measured with.
pub(super) struct Wrapped<'a> {
    pub text: &'a str,
    pub reference: &'a str,
}

/// Lines `text` takes at `size` in a column `width` px wide.
fn lines(text: &str, size: f32, width: f32) -> usize {
    let per_line = ((width / (size * AVG_CHAR_EM)).floor() as usize).max(1);
    text.lines()
        .map(|line| line.chars().count().div_ceil(per_line).max(1))
        .sum::<usize>()
        .max(1)
}

/// Height of the lines `t.text` takes beyond those of `t.reference`.
fn extra(t: &Wrapped<'_>, size: f32, width: f32) -> f32 {
    let more = lines(t.text, size, width).saturating_sub(lines(t.reference, size, width));
    more as f32 * size * LINE_HEIGHT
}

/// Extra window height for a dialog intro (a title and a secondary text) in a
/// card `card_w` wide whose body has `pad` px of padding on each side. The
/// icon tile narrows the column the title and the secondary text share.
pub(super) fn intro_growth(
    card_w: f32,
    pad: f32,
    title: Wrapped<'_>,
    secondary: Wrapped<'_>,
) -> f32 {
    let ts = TypeScale::DEFAULT;
    let width = card_w - 2.0 * pad - (DIALOG_ICON_TILE + Spacing::DEFAULT.m + 2.0);
    extra(&title, ts.title, width) + extra(&secondary, ts.body, width)
}

/// Extra window height for a one-line note that wraps (a caption line).
pub(super) fn note_growth(card_w: f32, pad: f32, text: &str) -> f32 {
    let size = TypeScale::DEFAULT.caption;
    extra(
        &Wrapped {
            text,
            reference: "",
        },
        size,
        card_w - 2.0 * pad,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_that_fits_one_line_adds_nothing_and_a_long_one_adds_lines() {
        let short = Wrapped {
            text: "short",
            reference: "short",
        };
        assert_eq!(extra(&short, 12.0, 400.0), 0.0);
        let long = Wrapped {
            text: &"word ".repeat(200),
            reference: "short",
        };
        assert!(extra(&long, 12.0, 400.0) >= 5.0 * 12.0 * LINE_HEIGHT);
    }

    #[test]
    fn a_narrower_column_wraps_more() {
        let t = "a library name that is quite long ".repeat(4);
        assert!(lines(&t, 12.0, 200.0) > lines(&t, 12.0, 400.0));
    }
}
