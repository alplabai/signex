//! Size-scale tokens: text sizes and spacing steps, derived from one base.
//!
//! The colour tokens in [`super::tokens`] stop a view from hard-coding a
//! colour; these stop it from hard-coding `.size(11)` / `.padding([4, 14])`.
//! Both scales are pure functions of a single base size, so a future
//! base-font-size preference rescales the whole chrome from one number.
//!
//! There is no base-size preference yet (only `ui_font` is persisted), so
//! views read [`TypeScale::default`] / [`Spacing::default`], which is the
//! scale at [`DEFAULT_BASE_SIZE`]. Threading a user preference means
//! replacing those two calls with `TypeScale::from_base(pref)`. Fixed chrome
//! lengths go through [`chrome_px`] so they move with the same base.
//!
//! # Mapping from the pre-token literals
//!
//! Measured over `crates/signex-app/src` (`.size(N)` call counts):
//!
//! | literal | count | step      | px at an 11 px base |
//! |---------|-------|-----------|--------------------|
//! | 9       | 36    | `caption` (folded, +1 px, applied when swept) | 10 |
//! | 10      | 316   | `caption` | 10 |
//! | 11      | 438   | `body`    | 11 |
//! | 12      | 96    | `label`   | 12 |
//! | 13      | 25    | `title`   | 13 |
//! | 14      | 34    | `heading` | 14 |
//!
//! Steps are ratios of the old 11 px base (`n / 11`), so at an 11 px base
//! every step reproduces its literal exactly; the default base is 12 px (#1956),
//! one step up. Spacing steps are `2 / 4 / 8 / 12 / 16` px at an 11 px base;
//! an off-grid padding snaps to the nearest step, ties toward the smaller one.
//! Literals outside the table fold onto `heading` (15, 16), `display` (20),
//! `glyph` (24 and the 64 px pause icon at 2.5x) and `micro` (4, separator rows).

/// Base size, in px, that the scales are defined against. It is the `body`
/// size.
pub const DEFAULT_BASE_SIZE: f32 = 12.0;

/// The base the ratios in this file were authored against: the pre-#1956
/// `body` literal. Lengths written in px at that base are rescaled by
/// [`chrome_px`].
const REFERENCE_BASE_SIZE: f32 = 11.0;

/// A fixed chrome length, authored in px at an 11 px base, scaled to the
/// default base so it keeps its proportion to the text it holds.
pub const fn chrome_px(px_at_reference: f32) -> f32 {
    px_at_reference * DEFAULT_BASE_SIZE / REFERENCE_BASE_SIZE
}

/// Thickness, in px, of a divider rule. A line weight, not a spacing gap, so
/// it stays 1 px whatever the base size.
pub const HAIRLINE: f32 = 1.0;

/// Text sizes, smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeScale {
    /// Hints, secondary status lines, small badges (10 px at an 11 px base).
    pub caption: f32,
    /// Default running text and controls (11 px at an 11 px base).
    pub body: f32,
    /// Emphasised control labels and inputs (12 px at an 11 px base).
    pub label: f32,
    /// Card / section titles (13 px at an 11 px base).
    pub title: f32,
    /// Dialog headings (14 px at an 11 px base).
    pub heading: f32,
    /// Product name in the About card (20 px).
    pub display: f32,
    /// Status icons drawn as text glyphs (24 px).
    pub glyph: f32,
    /// Rows of rule characters used as thin separators (4 px).
    pub micro: f32,
}

impl TypeScale {
    /// The scale at [`DEFAULT_BASE_SIZE`].
    pub const DEFAULT: Self = Self::from_base(DEFAULT_BASE_SIZE);

    /// Derive every step from `base` (the `body` size).
    pub const fn from_base(base: f32) -> Self {
        Self {
            caption: base * 10.0 / 11.0,
            body: base,
            label: base * 12.0 / 11.0,
            title: base * 13.0 / 11.0,
            heading: base * 14.0 / 11.0,
            display: base * 20.0 / 11.0,
            glyph: base * 24.0 / 11.0,
            micro: base * 4.0 / 11.0,
        }
    }
}

impl Default for TypeScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Padding / gap steps, smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Hairline gaps between tightly packed items (2 px at an 11 px base).
    pub xs: f32,
    /// Control padding, list-row gaps (4 px).
    pub s: f32,
    /// Standard gap between controls (8 px).
    pub m: f32,
    /// Section and card padding (12 px at an 11 px base).
    pub l: f32,
    /// Outer dialog padding (16 px).
    pub xl: f32,
}

impl Spacing {
    /// The scale at [`DEFAULT_BASE_SIZE`].
    pub const DEFAULT: Self = Self::from_base(DEFAULT_BASE_SIZE);

    /// Derive every step from `base` (the `body` size).
    pub const fn from_base(base: f32) -> Self {
        Self {
            xs: base * 2.0 / 11.0,
            s: base * 4.0 / 11.0,
            m: base * 8.0 / 11.0,
            l: base * 12.0 / 11.0,
            xl: base * 16.0 / 11.0,
        }
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Self::DEFAULT
    }
}
