//! A text note: the wrapped text at its justification inside a dashed frame,
//! the anchor dot, and the Text Size and Position callouts. The Position
//! leader leaves the frame from its bottom edge under the anchor, so it never
//! cuts through the text.

use iced::{Point, Rectangle};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::{HAlign, VAlign};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel, TextRole,
    Tint,
};

use super::{HEIGHT_EM, fields};

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 120.0;
/// The dashed frame, view units.
const FRAME: Rectangle = Rectangle {
    x: 72.0,
    y: 14.0,
    width: 176.0,
    height: 76.0,
};
/// Inner margin between the frame and the text.
const PAD: f32 = 8.0;
/// Height of one drawn line of text.
const LINE_H: f32 = 14.0;
/// Characters per drawn line, and lines drawn at most.
const LINE_CHARS: usize = 24;
const MAX_LINES: usize = 4;

/// `text` wrapped at word boundaries into lines of at most `width`
/// characters (a longer word is cut), keeping its own line breaks; at most
/// `max_lines`, the last ending in an ellipsis when text was left out.
pub(super) fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut word: String = word.to_string();
            loop {
                let room = if line.is_empty() {
                    width
                } else {
                    width.saturating_sub(line.chars().count() + 1)
                };
                let len = word.chars().count();
                if len <= room {
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(&word);
                    break;
                }
                if line.is_empty() {
                    let head: String = word.chars().take(width).collect();
                    word = word.chars().skip(width).collect();
                    lines.push(head);
                    if word.is_empty() {
                        break;
                    }
                } else {
                    lines.push(std::mem::take(&mut line));
                }
            }
        }
        lines.push(line);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            last.push('\u{2026}');
        }
    }
    lines
}

/// Where the anchor sits on the frame for a justification.
pub(super) fn anchor_point(h: HAlign, v: VAlign) -> Point {
    let x = match h {
        HAlign::Left => FRAME.x,
        HAlign::Center => FRAME.x + FRAME.width / 2.0,
        HAlign::Right => FRAME.x + FRAME.width,
    };
    let y = match v {
        VAlign::Top => FRAME.y,
        VAlign::Center => FRAME.y + FRAME.height / 2.0,
        VAlign::Bottom => FRAME.y + FRAME.height,
    };
    Point::new(x, y)
}

/// The note holding `text`, justified `h` / `v`.
pub fn text_note(text: &str, h: HAlign, v: VAlign, i18n: &I18n) -> Illustration {
    let lines = wrap(text, LINE_CHARS, MAX_LINES);
    let block = LINE_H * lines.len().max(1) as f32;
    let (x, anchor) = match h {
        HAlign::Left => (FRAME.x + PAD, Anchor::Start),
        HAlign::Center => (FRAME.x + FRAME.width / 2.0, Anchor::Middle),
        HAlign::Right => (FRAME.x + FRAME.width - PAD, Anchor::End),
    };
    let top = match v {
        VAlign::Top => FRAME.y + PAD,
        VAlign::Center => FRAME.y + (FRAME.height - block) / 2.0,
        VAlign::Bottom => FRAME.y + FRAME.height - PAD - block,
    };
    let frame = Part::new(Primitive::Rect { rect: FRAME }, Style::dashed());
    let text_parts = lines.into_iter().enumerate().map(|(i, line)| {
        Part::label(
            TextLabel {
                at: Point::new(x, top + LINE_H * (i as f32 + 0.5)),
                text: UserText::new(line).into(),
                anchor,
                role: TextRole::Caption,
            },
            Ink::Secondary,
        )
        .linked(fields::TEXT)
    });
    let dot = anchor_point(h, v);
    let anchor_dot = Part::new(
        Primitive::Circle {
            center: dot,
            radius: 2.5,
        },
        Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
    )
    .linked(fields::POSITION);
    let size_x = FRAME.x - PAD;
    let text_size = Callout::new(Point::new(size_x, top), Point::new(size_x, top + LINE_H))
        .label(t!(i18n, ids::PROPERTIES_TEXT_SIZE))
        .label_at(Point::new(size_x - 6.0, top + LINE_H / 2.0), Anchor::End);
    let bottom_edge = Point::new(dot.x, FRAME.y + FRAME.height);
    let position = Leader {
        at: bottom_edge,
        label_at: Point::new(dot.x, VIEW_H - 8.0),
        label: t!(i18n, ids::PROPERTIES_POSITION).into(),
        anchor: Anchor::Middle,
    };
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .part(frame)
        .parts(text_parts)
        .part(anchor_dot)
        .part(Part::callout(text_size).linked(fields::TEXT_SIZE))
        .part(Part::leader(position).linked(fields::POSITION))
}
