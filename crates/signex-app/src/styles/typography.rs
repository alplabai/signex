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
//! replacing those two calls with `TypeScale::from_base(pref)`.
//!
//! # Mapping from the pre-token literals
//!
//! Measured over `crates/signex-app/src` (`.size(N)` call counts):
//!
//! | literal | count | step      | px at default base |
//! |---------|-------|-----------|--------------------|
//! | 9       | 36    | `caption` (folded, +1 px, applied when swept) | 10 |
//! | 10      | 316   | `caption` | 10 |
//! | 11      | 438   | `body`    | 11 |
//! | 12      | 96    | `label`   | 12 |
//! | 13      | 25    | `title`   | 13 |
//! | 14      | 34    | `heading` | 14 |
//!
//! Steps are ratios of the base (`n / 11`), so at the default base every
//! step reproduces its literal exactly. Spacing steps are `2 / 4 / 8 / 12 /
//! 16` px at the default base; an off-grid padding snaps to the nearest
//! step, ties toward the smaller one.

/// Base size, in px, that the scales are defined against. It is the `body`
/// size: the most common literal (11 px) in the pre-token code.
pub const DEFAULT_BASE_SIZE: f32 = 11.0;

/// Text sizes, smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeScale {
    /// Hints, secondary status lines, small badges (10 px at default).
    pub caption: f32,
    /// Default running text and controls (11 px).
    pub body: f32,
    /// Emphasised control labels and inputs (12 px).
    pub label: f32,
    /// Card / section titles (13 px).
    pub title: f32,
    /// Dialog headings (14 px).
    pub heading: f32,
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
    /// Hairline gaps between tightly packed items (2 px at default).
    pub xs: f32,
    /// Control padding, list-row gaps (4 px).
    pub s: f32,
    /// Standard gap between controls (8 px).
    pub m: f32,
    /// Section and card padding (12 px).
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
