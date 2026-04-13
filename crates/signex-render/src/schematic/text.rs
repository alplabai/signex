//! Text rendering -- TextNote and TextProp (reference / value fields).

use iced::Color;
use iced::widget::canvas;

use signex_types::schematic::{HAlign, TextNote, TextProp, VAlign};

use super::ScreenTransform;

/// Draw a text note on the schematic.
pub fn draw_text_note(
    frame: &mut canvas::Frame,
    note: &TextNote,
    transform: &ScreenTransform,
    color: Color,
) {
    let font_size_mm = if note.font_size > 0.0 {
        note.font_size
    } else {
        1.27
    };
    let screen_font = transform.world_len(font_size_mm).max(6.0);

    let sp = transform.to_screen_point(note.position.x, note.position.y);

    let h_align = match note.justify_h {
        HAlign::Left => iced::alignment::Horizontal::Left,
        HAlign::Center => iced::alignment::Horizontal::Center,
        HAlign::Right => iced::alignment::Horizontal::Right,
    };

    let v_align = match note.justify_v {
        VAlign::Top => iced::alignment::Vertical::Top,
        VAlign::Center => iced::alignment::Vertical::Center,
        VAlign::Bottom => iced::alignment::Vertical::Bottom,
    };

    let rad = -(note.rotation.to_radians() as f32);

    frame.with_save(|f| {
        f.translate(iced::Vector::new(sp.x, sp.y));
        if rad.abs() > 0.001 {
            f.rotate(rad);
        }
        let text = canvas::Text {
            content: note.text.clone(),
            position: iced::Point::ORIGIN,
            color,
            size: iced::Pixels(screen_font),
            font: crate::IOSEVKA,
            align_x: h_align.into(),
            align_y: v_align,
            ..canvas::Text::default()
        };
        f.fill_text(text);
    });
}

/// Draw a property text (reference, value, or other field) at the
/// property's own position. These positions are absolute in world space,
/// not relative to the symbol -- KiCad stores them that way.
///
/// `mirror_x`: true when the parent symbol has `mirror x` (flips Y axis),
/// which causes KiCad to flip the horizontal justification of the field text
/// (SCH_FIELD::GetEffectiveJustify). Pass `sym.mirror_x`.
///
/// Rotation: KiCad field angles are CCW-positive in their Y-down screen
/// space. Iced `frame.rotate()` is CW-positive, so we negate the angle.
pub fn draw_text_prop(
    frame: &mut canvas::Frame,
    content: &str,
    prop: &TextProp,
    mirror_x: bool,
    transform: &ScreenTransform,
    color: Color,
) {
    if content.is_empty() {
        return;
    }

    let font_size_mm = if prop.font_size > 0.0 { prop.font_size } else { 1.27 };
    let screen_font = transform.world_len(font_size_mm).max(6.0);

    let sp = transform.to_screen_point(prop.position.x, prop.position.y);

    // KiCad SCH_FIELD::GetEffectiveJustify(): when symbol transform.x1 < 0
    // (i.e. mirror_x), flip Left ↔ Right.
    let effective_h = if mirror_x {
        match prop.justify_h {
            HAlign::Left => HAlign::Right,
            HAlign::Right => HAlign::Left,
            HAlign::Center => HAlign::Center,
        }
    } else {
        prop.justify_h
    };

    let h_align = match effective_h {
        HAlign::Left => iced::alignment::Horizontal::Left,
        HAlign::Center => iced::alignment::Horizontal::Center,
        HAlign::Right => iced::alignment::Horizontal::Right,
    };

    let v_align = match prop.justify_v {
        VAlign::Top => iced::alignment::Vertical::Top,
        VAlign::Center => iced::alignment::Vertical::Center,
        VAlign::Bottom => iced::alignment::Vertical::Bottom,
    };

    // KiCad angle: 0 = horizontal, 90 = vertical (reads bottom-to-top = CCW).
    // Iced CW-positive, Y-down → negate to get CCW.
    let rad = -(prop.rotation.to_radians() as f32);

    frame.with_save(|f| {
        f.translate(iced::Vector::new(sp.x, sp.y));
        if rad.abs() > 0.001 {
            f.rotate(rad);
        }
        let text = canvas::Text {
            content: content.to_string(),
            position: iced::Point::ORIGIN,
            color,
            size: iced::Pixels(screen_font),
            font: crate::IOSEVKA,
            align_x: h_align.into(),
            align_y: v_align,
            ..canvas::Text::default()
        };
        f.fill_text(text);
    });
}
