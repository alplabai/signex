//! Text rendering -- TextNote and TextProp (reference / value fields).

use iced::Color;
use iced::widget::canvas;

use signex_types::schematic::{HAlign, Symbol, TextNote, TextProp, VAlign};

use super::{ScreenTransform, field_effective_style};

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
    let screen_font = (transform.world_len(font_size_mm) * crate::canvas_font_size_scale()).abs();
    if screen_font < 1.0 {
        return;
    }

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
            font: crate::canvas_font(),
            align_x: h_align.into(),
            align_y: v_align,
            ..canvas::Text::default()
        };
        f.fill_text(text);
    });
}

/// Draw a property text (reference, value, or other field).
///
/// `display_pos`: resolved display position for the field text, computed by
/// caller via [`field_display_pos`].
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
    sym: &Symbol,
    display_pos: (f64, f64),
    transform: &ScreenTransform,
    color: Color,
) {
    if content.is_empty() {
        return;
    }

    let font_size_mm = if prop.font_size > 0.0 { prop.font_size } else { 1.27 };
    let screen_font = (transform.world_len(font_size_mm) * crate::canvas_font_size_scale()).abs();
    if screen_font < 1.0 {
        return;
    }

    let sp = transform.to_screen_point(display_pos.0, display_pos.1);

    let (draw_rotation, effective_h, effective_v) = field_effective_style(prop, sym);

    let h_align = match effective_h {
        HAlign::Left => iced::alignment::Horizontal::Left,
        HAlign::Center => iced::alignment::Horizontal::Center,
        HAlign::Right => iced::alignment::Horizontal::Right,
    };

    let v_align = match effective_v {
        VAlign::Top => iced::alignment::Vertical::Top,
        VAlign::Center => iced::alignment::Vertical::Center,
        VAlign::Bottom => iced::alignment::Vertical::Bottom,
    };

    // Iced CW-positive, Y-down; KiCad field angles are CCW.
    let rad = -(draw_rotation.to_radians() as f32);

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
            font: crate::canvas_font(),
            align_x: h_align.into(),
            align_y: v_align,
            ..canvas::Text::default()
        };
        f.fill_text(text);
    });
}
