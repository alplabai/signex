//! Effective draw style of a symbol's reference/value field.
//!
//! A field's [`TextProp`] rotation and justification are stored relative to
//! the symbol. Every renderer (canvas, SVG export, properties) must resolve
//! them through this one function so they cannot disagree.

use super::{HAlign, Symbol, TextProp, VAlign};

/// Resolve `prop` against its owning `sym`: returns `(draw_rotation_deg,
/// justify_h, justify_v)`. Total rotations of 180/270 fold to a readable
/// 0/90 and flip the justification; symbol mirroring also flips it.
pub fn field_effective_style(prop: &TextProp, sym: &Symbol) -> (f64, HAlign, VAlign) {
    let total = (sym.rotation + prop.rotation).rem_euclid(360.0);
    let (draw_rot, fold_h, fold_v) = match total.round() as i32 {
        0 => (0.0, false, false),
        90 => (90.0, false, false),
        180 => (0.0, true, false),
        270 => (90.0, false, true),
        _ => (total, false, false),
    };

    let flip_h = fold_h ^ sym.mirror_y;
    let flip_v = fold_v ^ sym.mirror_x;

    let h = if flip_h {
        match prop.justify_h {
            HAlign::Left => HAlign::Right,
            HAlign::Center => HAlign::Center,
            HAlign::Right => HAlign::Left,
        }
    } else {
        prop.justify_h
    };

    let v = if flip_v {
        match prop.justify_v {
            VAlign::Top => VAlign::Bottom,
            VAlign::Center => VAlign::Center,
            VAlign::Bottom => VAlign::Top,
        }
    } else {
        prop.justify_v
    };

    (draw_rot, h, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::Point;

    fn sym(rotation: f64, mirror_x: bool, mirror_y: bool) -> Symbol {
        Symbol {
            unknown_columns: Default::default(),
            unknown: Default::default(),
            uuid: uuid::Uuid::nil(),
            lib_id: String::new(),
            reference: String::new(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            position: Point::new(0.0, 0.0),
            rotation,
            mirror_x,
            mirror_y,
            unit: 1,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            fields_user_placed: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: Default::default(),
            custom_properties: Vec::new(),
            pin_uuids: Default::default(),
            instances: Vec::new(),
            library_id: None,
            row_id: None,
            library_version: String::new(),
        }
    }

    fn prop(rotation: f64) -> TextProp {
        TextProp {
            position: Point::new(0.0, 0.0),
            rotation,
            font_size: 1.27,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            hidden: false,
        }
    }

    #[test]
    fn symbol_90_prop_0_draws_vertical() {
        assert_eq!(
            field_effective_style(&prop(0.0), &sym(90.0, false, false)),
            (90.0, HAlign::Left, VAlign::Top)
        );
    }

    #[test]
    fn autoplaced_prop_90_on_symbol_90_folds_to_horizontal() {
        // 90 + 90 = 180: folds to a readable horizontal glyph, justify flipped.
        assert_eq!(
            field_effective_style(&prop(90.0), &sym(90.0, false, false)),
            (0.0, HAlign::Right, VAlign::Top)
        );
    }

    #[test]
    fn symbol_180_folds_to_horizontal_and_flips_h() {
        assert_eq!(
            field_effective_style(&prop(0.0), &sym(180.0, false, false)),
            (0.0, HAlign::Right, VAlign::Top)
        );
    }

    #[test]
    fn mirror_flips_justification() {
        assert_eq!(
            field_effective_style(&prop(0.0), &sym(0.0, true, true)),
            (0.0, HAlign::Right, VAlign::Bottom)
        );
    }
}
