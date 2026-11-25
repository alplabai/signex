//! A text note (mockup row 47): the wrapped text at its justification inside
//! a dashed frame, the anchor dot, and the Text Size and Position callouts
//! with the values the fields hold. The Position leader leaves the frame from
//! its bottom edge under the anchor, so it never cuts through the text; the
//! justification is named under it.

use iced::{Point, Rectangle};
use signex_i18n::{I18n, Translatable, UserText, ids, t};
use signex_types::schematic::{HAlign, VAlign};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel, TextRole,
    Tint,
};

use super::{HEIGHT_EM, fields};

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 120.0;
/// The dashed frame, view units. The column left of it holds the Text Size
/// callout; the band under it the Position callout and the justification.
const FRAME: Rectangle = Rectangle {
    x: 84.0,
    y: 10.0,
    width: 172.0,
    height: 66.0,
};
/// Radius of the anchor dot, and of the panel-ground ring that keeps it
/// clear of the frame's dashes.
const DOT_R: f32 = 3.0;
const DOT_HALO_R: f32 = 5.0;
/// The range the Position label's centre keeps to, so the label stays
/// whole whichever corner the anchor is at.
const POSITION_LABEL_X: (f32, f32) = (84.0, 180.0);

/// The values the callouts repeat, as the fields show them; `None` while
/// the panel has none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteValues {
    /// X and Y.
    pub position: Option<(String, String)>,
    /// The size and its unit symbol.
    pub size: Option<(String, String)>,
}
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

/// The Justify H / V segment names of `h` / `v`.
fn justify_names(h: HAlign, v: VAlign) -> (Translatable, Translatable) {
    let across = match h {
        HAlign::Left => ids::PANELS_MULTI_JUSTIFY_LEFT,
        HAlign::Center => ids::PANELS_MULTI_JUSTIFY_CENTER,
        HAlign::Right => ids::PANELS_MULTI_JUSTIFY_RIGHT,
    };
    let down = match v {
        VAlign::Top => ids::PROPERTIES_JUSTIFY_TOP,
        VAlign::Center => ids::PROPERTIES_JUSTIFY_MIDDLE,
        VAlign::Bottom => ids::PROPERTIES_JUSTIFY_BOTTOM,
    };
    (Translatable::new(across), Translatable::new(down))
}

/// The anchor dot at `at`, on a ring of panel ground that cuts the frame's
/// dashes away round it.
fn anchor_dot(at: Point) -> [Part; 2] {
    let disc = |radius: f32, paint: Ink| {
        Part::new(
            Primitive::Circle { center: at, radius },
            Style::filled(Paint::Ink(paint), Tint::Solid),
        )
        .linked(fields::POSITION)
    };
    [disc(DOT_HALO_R, Ink::Panel), disc(DOT_R, Ink::Secondary)]
}

/// The Position callout: a leader from the frame's bottom edge under the
/// anchor `dot` to the position, and the justification under it.
fn position_parts(dot: Point, h: HAlign, v: VAlign, values: &NoteValues, i18n: &I18n) -> [Part; 2] {
    let label_x = dot.x.clamp(POSITION_LABEL_X.0, POSITION_LABEL_X.1);
    let label_y = FRAME.y + FRAME.height + LINE_H + PAD / 2.0;
    let position = match &values.position {
        Some((x, y)) => t!(
            i18n,
            ids::PROPERTIES_PART_ILLUSTRATION_POSITION,
            "x" => x.as_str(),
            "y" => y.as_str()
        ),
        None => t!(i18n, ids::PROPERTIES_POSITION),
    };
    let leader = Leader {
        at: Point::new(dot.x, FRAME.y + FRAME.height),
        label_at: Point::new(label_x, label_y),
        label: position.into(),
        anchor: Anchor::Middle,
    };
    let (across, down) = justify_names(h, v);
    let justify = TextLabel {
        at: Point::new(label_x, label_y + LINE_H + PAD / 4.0),
        text: t!(i18n, ids::PROPERTIES_TEXT_NOTE_ILLUSTRATION_JUSTIFY, "h" => across, "v" => down)
            .into(),
        anchor: Anchor::Middle,
        role: TextRole::Caption,
    };
    [
        Part::leader(leader).linked(fields::POSITION),
        Part::label(justify, Ink::Faint).linked(fields::POSITION),
    ]
}

/// The Text Size callout one line high at `top`, left of the frame, with
/// the size under its name.
fn text_size_parts(top: f32, values: &NoteValues, i18n: &I18n) -> Vec<Part> {
    let size_x = FRAME.x - PAD;
    let label_x = size_x - PAD / 2.0;
    let callout = Callout::new(Point::new(size_x, top), Point::new(size_x, top + LINE_H))
        .label(t!(i18n, ids::PROPERTIES_TEXT_SIZE))
        .label_at(Point::new(label_x, top + LINE_H / 2.0), Anchor::End);
    let mut parts = vec![Part::callout(callout).linked(fields::TEXT_SIZE)];
    if let Some((size, unit)) = &values.size {
        let value = TextLabel {
            at: Point::new(label_x, top + LINE_H * 1.5),
            text: t!(
                i18n,
                ids::PROPERTIES_TEXT_NOTE_ILLUSTRATION_SIZE,
                "size" => size.as_str(),
                "unit" => unit.as_str()
            )
            .into(),
            anchor: Anchor::End,
            role: TextRole::Caption,
        };
        parts.push(Part::label(value, Ink::Faint).linked(fields::TEXT_SIZE));
    }
    parts
}

/// The note holding `text`, justified `h` / `v`, its callouts carrying
/// `values`.
pub fn text_note(
    text: &str,
    h: HAlign,
    v: VAlign,
    values: &NoteValues,
    i18n: &I18n,
) -> Illustration {
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
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .part(frame)
        .parts(text_parts)
        .parts(anchor_dot(dot))
        .parts(text_size_parts(top, values, i18n))
        .parts(position_parts(dot, h, v, values, i18n))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every annotation text of `drawing`, isolation marks dropped.
    fn annotations(drawing: &Illustration) -> Vec<String> {
        drawing
            .visible_parts()
            .filter_map(|p| match &p.primitive {
                Primitive::Callout(c) => c.label.as_ref().map(|l| l.as_str().to_owned()),
                Primitive::Leader(leader) => Some(leader.label.as_str().to_owned()),
                Primitive::Label(label) if label.role == TextRole::Caption => {
                    Some(label.text.as_str().to_owned())
                }
                _ => None,
            })
            .map(|t| t.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// Mockup row 47: the Position callout carries the position and the
    /// justification, and the Text Size callout the size, as the fields
    /// show them.
    #[test]
    fn the_callouts_carry_the_field_values() {
        let en = I18n::en_us();
        let values = NoteValues {
            position: Some(("63.500".into(), "152.400".into())),
            size: Some(("1.270".into(), "mm".into())),
        };
        let drawing = text_note("Place C14", HAlign::Left, VAlign::Top, &values, &en);
        let shown = annotations(&drawing);
        for want in [
            "Position 63.500, 152.400",
            "Justify Left / Top",
            "Text Size",
            "1.270 mm",
        ] {
            assert!(shown.iter().any(|t| t == want), "{want:?} not in {shown:?}");
        }
        let drawing = text_note("x", HAlign::Right, VAlign::Bottom, &values, &en);
        let shown = annotations(&drawing);
        let want = "Justify Right / Bottom";
        assert!(shown.iter().any(|t| t == want), "{shown:?}");
    }

    /// Mockup row 47: the anchor dot sits on a ring of panel ground, so the
    /// frame's dashes never hide it.
    #[test]
    fn the_anchor_dot_is_cut_clear_of_the_frame() {
        let en = I18n::en_us();
        let none = NoteValues::default();
        let drawing = text_note("x", HAlign::Left, VAlign::Center, &none, &en);
        let discs: Vec<(f32, Option<Paint>)> = drawing
            .visible_parts()
            .filter_map(|p| match p.primitive {
                Primitive::Circle { center, radius } => {
                    assert_eq!(center, anchor_point(HAlign::Left, VAlign::Center));
                    Some((radius, p.style.fill.map(|f| f.paint)))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            discs,
            vec![
                (DOT_HALO_R, Some(Paint::Ink(Ink::Panel))),
                (DOT_R, Some(Paint::Ink(Ink::Secondary))),
            ]
        );
    }
}
