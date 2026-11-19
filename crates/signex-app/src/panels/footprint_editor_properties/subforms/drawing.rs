//! Drawing pieces the sub-form illustrations share: the view box, labels,
//! notes and the 45° hatch of a region (#126).

use iced::{Point, Rectangle};
use signex_widgets::property_illustration::{
    Anchor, Illustration, Ink, Paint, Part, Primitive, Style, Text, TextLabel, TextRole, Weight,
};

/// The view box every sub-form illustration is drawn on.
pub(super) const VIEW_W: f32 = 264.0;
pub(super) const VIEW_H: f32 = 140.0;

/// An empty illustration on the shared view box.
pub(super) fn canvas() -> Illustration {
    Illustration::new(VIEW_W, VIEW_H)
}

/// A label at `at`.
pub(super) fn label(at: Point, text: impl Into<Text>, anchor: Anchor, role: TextRole) -> Part {
    Part::label(
        TextLabel {
            at,
            text: text.into(),
            anchor,
            role,
        },
        Ink::Secondary,
    )
}

/// The small note in the top-left corner ("Top Layer").
pub(super) fn corner_note(text: impl Into<Text>) -> Part {
    label(Point::new(4.0, 12.0), text, Anchor::Start, TextRole::Note)
}

/// The note along the bottom edge.
pub(super) fn bottom_note(text: impl Into<Text>) -> Part {
    label(
        Point::new(4.0, VIEW_H - 6.0),
        text,
        Anchor::Start,
        TextRole::Note,
    )
}

/// A rectangle from its centre and size.
pub(super) fn centred(center: Point, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x: center.x - width / 2.0,
        y: center.y - height / 2.0,
        width,
        height,
    }
}

/// Lines at 45° filling `rect`, `gap` apart along each axis.
pub(super) fn hatch(rect: Rectangle, gap: f32, paint: Paint) -> Vec<Part> {
    let (x0, y0) = (rect.x, rect.y);
    let (x1, y1) = (rect.x + rect.width, rect.y + rect.height);
    let style = Style::outline(Ink::Tertiary, Weight::Hairline).with_stroke_paint(paint);
    let mut parts = Vec::new();
    let mut s = x0 + y0 + gap;
    while s < x1 + y1 {
        // The line x + y = s, clipped to the rectangle.
        let start_x = x0.max(s - y1);
        let end_x = x1.min(s - y0);
        if start_x < end_x {
            parts.push(Part::new(
                Primitive::Line {
                    from: Point::new(start_x, s - start_x),
                    to: Point::new(end_x, s - end_x),
                },
                style,
            ));
        }
        s += gap;
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hatch_stays_inside_its_rectangle() {
        let rect = Rectangle {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 30.0,
        };
        let parts = hatch(rect, 7.0, Paint::Ink(Ink::Tertiary));
        assert!(!parts.is_empty());
        for part in parts {
            let Primitive::Line { from, to } = part.primitive else {
                panic!("a hatch is made of lines");
            };
            for p in [from, to] {
                assert!(p.x >= rect.x - 1e-3 && p.x <= rect.x + rect.width + 1e-3);
                assert!(p.y >= rect.y - 1e-3 && p.y <= rect.y + rect.height + 1e-3);
            }
        }
    }
}
