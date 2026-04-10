//! Text rendering -- TextNote and TextProp (reference / value fields).

use iced::widget::canvas;
use iced::Color;

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

    let text = canvas::Text {
        content: note.text.clone(),
        position: sp,
        color,
        size: iced::Pixels(screen_font),
        font: crate::IOSEVKA,
        ..canvas::Text::default()
    };

    frame.fill_text(text);
}

/// Draw a property text (reference, value, or other field) at the
/// property's own position. These positions are absolute in world space,
/// not relative to the symbol -- KiCad stores them that way.
pub fn draw_text_prop(
    frame: &mut canvas::Frame,
    content: &str,
    prop: &TextProp,
    transform: &ScreenTransform,
    color: Color,
) {
    if content.is_empty() {
        return;
    }

    let font_size_mm = if prop.font_size > 0.0 {
        prop.font_size
    } else {
        1.27
    };
    let screen_font = transform.world_len(font_size_mm).max(6.0);

    let sp = transform.to_screen_point(prop.position.x, prop.position.y);

    let h_align = match prop.justify_h {
        HAlign::Left => iced::alignment::Horizontal::Left,
        HAlign::Center => iced::alignment::Horizontal::Center,
        HAlign::Right => iced::alignment::Horizontal::Right,
    };

    let v_align = match prop.justify_v {
        VAlign::Top => iced::alignment::Vertical::Top,
        VAlign::Center => iced::alignment::Vertical::Center,
        VAlign::Bottom => iced::alignment::Vertical::Bottom,
    };

    let text = canvas::Text {
        content: content.to_string(),
        position: sp,
        color,
        size: iced::Pixels(screen_font),
        font: crate::IOSEVKA,
        align_x: h_align.into(),
        align_y: v_align.into(),
        ..canvas::Text::default()
    };

    frame.fill_text(text);
}
