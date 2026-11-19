//! Size-scale tokens: text sizes and spacing steps, derived from one base.
//!
//! The colour tokens in [`super::tokens`] stop a view from hard-coding a
//! colour; these stop it from hard-coding `.size(11)` / `.padding([4, 14])`.
//! Both scales are pure functions of a single base size, so a future
//! base-font-size preference rescales the whole chrome from one number. The
//! values live in `signex_widgets::theme_ext` so widgets read the same scale.
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
//! `glyph` (24), `banner` (the 64 px pause icon) and `micro` (4, separator rows).

pub use signex_widgets::theme_ext::{
    DEFAULT_BASE_SIZE, HAIRLINE, LINE_HEIGHT, Spacing, TypeScale, chrome_px,
};
