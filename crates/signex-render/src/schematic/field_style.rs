//! Effective field rotation + justify under a parent symbol transform.
//!
//! Spec: `docs/RENDERING_RULES.md::field-rotation-and-justify`.
//!
//! A schematic symbol's reference / value / user fields store their
//! own position and rotation independent of the parent symbol body.
//! When the parent rotates or mirrors, two adjustments apply so the
//! field text stays readable and stays anchored outside the body:
//!
//! - **Rotation folding** — fields with stored rotation `0°` render
//!   horizontally regardless of the parent's rotation; fields with
//!   stored `90°` fold the parent's rotation in. Whatever the user
//!   stored ends up either horizontal (`0°`) or vertical (`90°`)
//!   when rendered — never `180°` (upside-down) or `270°`
//!   (sideways-right).
//! - **Justify-flip on axis flip** — `mirror_y` flips H justify;
//!   `mirror_x` flips V justify; a parent rotation of `180°` flips
//!   both axes. Without this rule, a `Justify::Right` field stored to
//!   the *left* of a 180°-rotated body would anchor on its right edge
//!   and visibly grow back through the body.
//!
//! All semantics are derived from `signex_types::schematic`'s
//! `Symbol::rotation` / `mirror_x` / `mirror_y` fields and the
//! `TextProp` struct — no third-party reference.

use signex_types::schematic::{HAlign, TextProp, VAlign};

use super::SymbolTransform;

/// Effective `(rotation_deg, justify_h, justify_v)` for a field given
/// its stored values and the parent symbol's transform.
///
/// # Example
///
/// ```ignore
/// let parent = SymbolTransform::from_symbol(symbol);
/// let (rot, h, v) = field_effective_style(&symbol.ref_text.unwrap(), &parent);
/// // ... pass (rot, h, v) into text::draw_rotated_text ...
/// ```
#[must_use]
pub fn field_effective_style(
    prop: &TextProp,
    transform: &SymbolTransform,
) -> (f64, HAlign, VAlign) {
    let rotation = effective_rotation(prop.rotation, transform.rotation_deg);
    let (h, v) = effective_justify(
        prop.justify_h,
        prop.justify_v,
        transform.rotation_deg,
        transform.mirror_x,
        transform.mirror_y,
    );
    (rotation, h, v)
}

/// Fold the parent's rotation into the field's stored rotation,
/// reducing the result to either `0.0` (horizontal) or `90.0`
/// (vertical) so the rendered glyph never reads upside-down.
fn effective_rotation(stored_deg: f64, parent_deg: f64) -> f64 {
    let combined = (stored_deg + parent_deg).rem_euclid(360.0);
    // 90° and 270° both render as vertical; 180° renders as
    // horizontal (with justify flipped — see effective_justify).
    let bucket = combined.rem_euclid(180.0);
    if (bucket - 90.0).abs() < 45.0 {
        90.0
    } else {
        0.0
    }
}

/// Apply the justify-flip rules.
fn effective_justify(
    stored_h: HAlign,
    stored_v: VAlign,
    parent_rotation_deg: f64,
    parent_mirror_x: bool,
    parent_mirror_y: bool,
) -> (HAlign, VAlign) {
    let mut h = stored_h;
    let mut v = stored_v;

    let parent_normalised = parent_rotation_deg.rem_euclid(360.0);
    let parent_180 =
        (parent_normalised - 180.0).abs() < 45.0 || (parent_normalised - 180.0).abs() > 315.0;

    if parent_mirror_y || parent_180 {
        h = flip_h(h);
    }
    if parent_mirror_x || parent_180 {
        v = flip_v(v);
    }
    (h, v)
}

#[inline]
const fn flip_h(h: HAlign) -> HAlign {
    match h {
        HAlign::Left => HAlign::Right,
        HAlign::Right => HAlign::Left,
        HAlign::Center => HAlign::Center,
    }
}

#[inline]
const fn flip_v(v: VAlign) -> VAlign {
    match v {
        VAlign::Top => VAlign::Bottom,
        VAlign::Bottom => VAlign::Top,
        VAlign::Center => VAlign::Center,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Point;

    fn identity() -> SymbolTransform {
        SymbolTransform {
            origin: Point::ZERO,
            rotation_deg: 0.0,
            mirror_x: false,
            mirror_y: false,
        }
    }

    fn prop(rotation: f64, h: HAlign, v: VAlign) -> TextProp {
        TextProp {
            position: Point::ZERO,
            rotation,
            font_size: 1.27,
            justify_h: h,
            justify_v: v,
            hidden: false,
        }
    }

    #[test]
    fn identity_passes_through() {
        let (r, h, v) = field_effective_style(&prop(0.0, HAlign::Left, VAlign::Top), &identity());
        assert!((r - 0.0).abs() < 1e-9);
        assert_eq!(h, HAlign::Left);
        assert_eq!(v, VAlign::Top);
    }

    #[test]
    fn parent_180_flips_both_justify_axes() {
        let mut t = identity();
        t.rotation_deg = 180.0;
        let (_, h, v) = field_effective_style(&prop(0.0, HAlign::Left, VAlign::Top), &t);
        assert_eq!(h, HAlign::Right);
        assert_eq!(v, VAlign::Bottom);
    }

    #[test]
    fn mirror_y_flips_only_h() {
        let mut t = identity();
        t.mirror_y = true;
        let (_, h, v) = field_effective_style(&prop(0.0, HAlign::Left, VAlign::Top), &t);
        assert_eq!(h, HAlign::Right);
        assert_eq!(v, VAlign::Top);
    }

    #[test]
    fn mirror_x_flips_only_v() {
        let mut t = identity();
        t.mirror_x = true;
        let (_, h, v) = field_effective_style(&prop(0.0, HAlign::Left, VAlign::Top), &t);
        assert_eq!(h, HAlign::Left);
        assert_eq!(v, VAlign::Bottom);
    }

    #[test]
    fn ninety_symbol_with_stored_zero_field_renders_vertical() {
        // Edge case from the Wave 1 stub note: parent 90°, stored 0°.
        // The folded effective rotation is 90° (vertical).
        let mut t = identity();
        t.rotation_deg = 90.0;
        let (r, _, _) = field_effective_style(&prop(0.0, HAlign::Left, VAlign::Top), &t);
        assert!((r - 90.0).abs() < 1e-9);
    }

    #[test]
    fn folded_rotation_never_returns_upside_down_or_sideways_back() {
        // For any parent + stored combination the result should be
        // either 0° or 90° — never 180° or 270° — so glyphs never
        // render upside-down.
        for parent in [0.0, 45.0, 90.0, 180.0, 270.0, 315.0] {
            for stored in [0.0, 90.0, 180.0, 270.0] {
                let r = effective_rotation(stored, parent);
                assert!(
                    (r - 0.0).abs() < 1e-9 || (r - 90.0).abs() < 1e-9,
                    "parent={parent} stored={stored} -> {r}"
                );
            }
        }
    }
}
