//! Effective field rotation + justify under a parent symbol transform.
//!
//! Spec: `docs/RENDERING_RULES.md::field-rotation-and-justify`.
//!
//! A schematic symbol's reference / value / user fields store their
//! own position and rotation independent of the parent symbol body.
//! When the parent rotates or mirrors, two adjustments must apply so
//! the field text stays readable and stays anchored outside the body:
//!
//! - **Rotation folding** — vertically rotated symbols (90° / 270°)
//!   prefer to display their reference / value horizontally.
//! - **Justify-flip on axis flip** — 180° rotation, mirror-X, mirror-Y
//!   each flip the corresponding justify axis to keep the text
//!   anchored outside the body.
//!
//! Filled in by Wave 3 sub-agent 12.

use signex_types::schematic::{HAlign, TextProp, VAlign};

use super::SymbolTransform;

/// The rendered (rotation, justify_h, justify_v) for a field given the
/// stored `prop` values and the parent symbol's `transform`.
pub fn field_effective_style(
    _prop: &TextProp,
    _transform: &SymbolTransform,
) -> (f64, HAlign, VAlign) {
    todo!(
        "Wave 3 sub-agent 12 fills this in against RENDERING_RULES.md::field-rotation-and-justify"
    )
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: smoke (identity transform passes through),
    // 180° rotation flips both justify axes, mirror-X flips H justify,
    // mirror-Y flips V justify, 90° symbol with stored 0° field still
    // renders horizontal.
}
