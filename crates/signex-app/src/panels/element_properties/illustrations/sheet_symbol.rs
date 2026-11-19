//! Line-art illustration of a sheet symbol (#46): the box at its size with
//! its fill and line width, the name above and the file below, the entries
//! on the edges with their direction shapes, the W / H dimensions and a
//! Line Width callout. Data only; `property_illustration` draws it.

use iced::{Color, Point, Rectangle};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::{ChildSheet, SheetPin};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel,
    TextRole, Tint, Weight,
};

/// Field ids the sheet symbol rows and the drawing share.
pub(in crate::panels) const SHEET_SIZE: FieldId = FieldId::new("sheet.size");
pub(in crate::panels) const SHEET_NAME: FieldId = FieldId::new("sheet.name");
pub(in crate::panels) const SHEET_FILE: FieldId = FieldId::new("sheet.file");
pub(in crate::panels) const SHEET_BORDER: FieldId = FieldId::new("sheet.border");
pub(in crate::panels) const SHEET_FILL: FieldId = FieldId::new("sheet.fill");
pub(in crate::panels) const SHEET_LINE_WIDTH: FieldId = FieldId::new("sheet.line_width");
pub(in crate::panels) const SHEET_ENTRIES: FieldId = FieldId::new("sheet.entries");

/// The view box, the size of the approved drawings.
const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 140.0;
/// The largest box the drawing gives the symbol.
const BOX_MAX_W: f32 = 150.0;
const BOX_MAX_H: f32 = 74.0;
/// Top of the box: room above for the W dimension and the name.
const BOX_TOP: f32 = 38.0;
/// Half the size of an entry's direction shape.
const ENTRY_HALF: f32 = 3.5;

/// The direction of a sheet entry, read from its stored word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::panels) enum EntryDirection {
    Input,
    Output,
    Bidirectional,
    TriState,
    Passive,
    Unspecified,
    /// A word the form has no name for; shown as stored.
    Other,
}

pub(in crate::panels) fn entry_direction(word: &str) -> EntryDirection {
    // Parses the stored file word, never shown: ASCII fold in every language.
    // ast-grep-ignore: no-case-in-view
    match word.trim().to_ascii_lowercase().as_str() {
        "input" | "in" => EntryDirection::Input,
        "output" | "out" => EntryDirection::Output,
        "bidirectional" | "bidir" | "bi_directional" => EntryDirection::Bidirectional,
        "tri_state" | "tristate" | "three_state" => EntryDirection::TriState,
        "passive" => EntryDirection::Passive,
        "" | "unspecified" => EntryDirection::Unspecified,
        _ => EntryDirection::Other,
    }
}

/// The edge of the symbol an entry sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::panels) enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// The edge nearest `pin` and how far along it the pin sits (0 to 1, from
/// the top or the left).
pub(in crate::panels) fn entry_place(sheet: &ChildSheet, pin: &SheetPin) -> (Edge, f32) {
    let (x0, y0) = (sheet.position.x, sheet.position.y);
    let (w, h) = (
        sheet.size.0.max(f64::EPSILON),
        sheet.size.1.max(f64::EPSILON),
    );
    let (px, py) = (pin.position.x, pin.position.y);
    let gaps = [
        (Edge::Left, (px - x0).abs()),
        (Edge::Right, (x0 + w - px).abs()),
        (Edge::Top, (py - y0).abs()),
        (Edge::Bottom, (y0 + h - py).abs()),
    ];
    let edge = gaps
        .iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(Edge::Left, |(edge, _)| *edge);
    let along = match edge {
        Edge::Left | Edge::Right => (py - y0) / h,
        Edge::Top | Edge::Bottom => (px - x0) / w,
    };
    (edge, along.clamp(0.0, 1.0) as f32)
}

/// The box the symbol is drawn as: its aspect kept, as large as the view
/// allows.
pub(in crate::panels) fn symbol_box(size: (f64, f64)) -> Rectangle {
    let (w, h) = (
        size.0.max(f64::EPSILON) as f32,
        size.1.max(f64::EPSILON) as f32,
    );
    let scale = (BOX_MAX_W / w).min(BOX_MAX_H / h);
    let (bw, bh) = (w * scale, h * scale);
    Rectangle {
        x: (VIEW_W - bw) / 2.0 - 8.0,
        y: BOX_TOP,
        width: bw,
        height: bh,
    }
}

fn caption(
    at: Point,
    text: impl Into<signex_widgets::property_illustration::Text>,
    anchor: Anchor,
    role: TextRole,
) -> TextLabel {
    TextLabel {
        at,
        text: text.into(),
        anchor,
        role,
    }
}

/// The closed outline of an entry's direction shape at edge point `at`,
/// `out` the unit vector leaving the box.
fn entry_shape(direction: EntryDirection, at: Point, out: (f32, f32)) -> Vec<Point> {
    let s = ENTRY_HALF;
    let along = (-out.1, out.0);
    let p = |n: f32, t: f32| {
        Point::new(
            at.x + out.0 * n + along.0 * t,
            at.y + out.1 * n + along.1 * t,
        )
    };
    match direction {
        // Points into the box.
        EntryDirection::Input => vec![p(0.0, 0.0), p(2.0 * s, s), p(2.0 * s, -s)],
        // Points out of the box.
        EntryDirection::Output => vec![p(0.0, s), p(2.0 * s, 0.0), p(0.0, -s)],
        EntryDirection::Bidirectional | EntryDirection::TriState => {
            vec![p(0.0, 0.0), p(s, s), p(2.0 * s, 0.0), p(s, -s)]
        }
        EntryDirection::Passive | EntryDirection::Unspecified | EntryDirection::Other => {
            vec![p(0.0, s), p(2.0 * s, s), p(2.0 * s, -s), p(0.0, -s)]
        }
    }
}

/// The entries on the edges, each with its name inside the box.
fn entry_parts(sheet: &ChildSheet, frame: Rectangle) -> Vec<Part> {
    let mut parts = Vec::new();
    for pin in &sheet.pins {
        let (edge, along) = entry_place(sheet, pin);
        let (at, out, label_at, anchor) = match edge {
            Edge::Left => {
                let y = frame.y + frame.height * along;
                (
                    Point::new(frame.x, y),
                    (-1.0, 0.0),
                    Point::new(frame.x + 4.0, y),
                    Anchor::Start,
                )
            }
            Edge::Right => {
                let (x, y) = (frame.x + frame.width, frame.y + frame.height * along);
                (
                    Point::new(x, y),
                    (1.0, 0.0),
                    Point::new(x - 4.0, y),
                    Anchor::End,
                )
            }
            Edge::Top => {
                let x = frame.x + frame.width * along;
                (
                    Point::new(x, frame.y),
                    (0.0, -1.0),
                    Point::new(x, frame.y + 8.0),
                    Anchor::Middle,
                )
            }
            Edge::Bottom => {
                let (x, y) = (frame.x + frame.width * along, frame.y + frame.height);
                (
                    Point::new(x, y),
                    (0.0, 1.0),
                    Point::new(x, y - 5.0),
                    Anchor::Middle,
                )
            }
        };
        let shape = Primitive::Polyline {
            points: entry_shape(entry_direction(&pin.direction), at, out),
            is_closed: true,
        };
        parts.push(
            Part::new(
                shape,
                Style::outline(Ink::Secondary, Weight::Thin)
                    .with_fill(Paint::Ink(Ink::Secondary), Tint::Body),
            )
            .linked(SHEET_ENTRIES),
        );
        parts.push(
            Part::label(
                caption(
                    label_at,
                    UserText::new(pin.name.clone()),
                    anchor,
                    TextRole::Note,
                ),
                Ink::Tertiary,
            )
            .linked(SHEET_ENTRIES),
        );
    }
    parts
}

/// The sheet symbol illustration.
pub(in crate::panels) fn sheet_symbol_illustration(
    sheet: &ChildSheet,
    i18n: &I18n,
) -> Illustration {
    let frame = symbol_box(sheet.size);
    let (left, top) = (frame.x, frame.y);
    let (right, bottom) = (frame.x + frame.width, frame.y + frame.height);
    let fill = match sheet.fill_color {
        Some(c) => Paint::Document(Color::from_rgba8(c.r, c.g, c.b, f32::from(c.a) / 255.0)),
        None => Paint::Ink(Ink::Secondary),
    };
    let mut edge = Style::outline(Ink::Secondary, Weight::Heavy).with_fill(fill, Tint::Light);
    if let Some(c) = sheet.stroke_color {
        edge = edge.with_stroke_paint(Paint::Document(Color::from_rgba8(
            c.r,
            c.g,
            c.b,
            f32::from(c.a) / 255.0,
        )));
    }
    let w_callout = Callout::new(Point::new(left, top - 22.0), Point::new(right, top - 22.0))
        .label(t!(i18n, ids::PROPERTIES_AXIS_W))
        .label_at(Point::new((left + right) / 2.0, top - 27.0), Anchor::Middle)
        .extension(Point::new(left, top - 2.0), Point::new(left, top - 25.0))
        .extension(Point::new(right, top - 2.0), Point::new(right, top - 25.0));
    let h_x = right + 16.0;
    let h_callout = Callout::new(Point::new(h_x, top), Point::new(h_x, bottom))
        .label(t!(i18n, ids::PROPERTIES_AXIS_H))
        .label_at(Point::new(h_x + 5.0, (top + bottom) / 2.0), Anchor::Start)
        .extension(Point::new(right + 2.0, top), Point::new(h_x + 3.0, top))
        .extension(
            Point::new(right + 2.0, bottom),
            Point::new(h_x + 3.0, bottom),
        );
    let width_leader = Leader {
        at: Point::new(left, bottom - frame.height * 0.15),
        label_at: Point::new(left - 10.0, bottom + 14.0),
        label: t!(i18n, ids::PROPERTIES_SHEET_LINE_WIDTH).into(),
        anchor: Anchor::End,
    };
    Illustration::new(VIEW_W, VIEW_H)
        .part(
            Part::new(Primitive::Rect { rect: frame }, edge).linked_all([
                SHEET_BORDER,
                SHEET_FILL,
                SHEET_LINE_WIDTH,
            ]),
        )
        .parts(entry_parts(sheet, frame))
        .part(
            Part::label(
                caption(
                    Point::new(left, top - 6.0),
                    UserText::new(sheet.name.clone()),
                    Anchor::Start,
                    TextRole::Caption,
                ),
                Ink::Secondary,
            )
            .linked(SHEET_NAME),
        )
        .part(
            Part::label(
                caption(
                    Point::new(right, bottom + 12.0),
                    UserText::new(sheet.filename.clone()),
                    Anchor::End,
                    TextRole::Note,
                ),
                Ink::Tertiary,
            )
            .linked(SHEET_FILE),
        )
        .part(Part::callout(w_callout).linked(SHEET_SIZE))
        .part(Part::callout(h_callout).linked(SHEET_SIZE))
        .part(Part::leader(width_leader).linked(SHEET_LINE_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Point as SchPoint;

    fn sheet(size: (f64, f64), pins: Vec<SheetPin>) -> ChildSheet {
        ChildSheet {
            uuid: uuid::Uuid::now_v7(),
            name: "Power".into(),
            filename: "power.snxsch".into(),
            position: SchPoint::new(10.0, 20.0),
            size,
            stroke_width: 0.0,
            fill: Default::default(),
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins,
            instances: Vec::new(),
        }
    }

    fn pin(name: &str, direction: &str, at: (f64, f64)) -> SheetPin {
        SheetPin {
            uuid: uuid::Uuid::now_v7(),
            name: name.into(),
            direction: direction.into(),
            position: SchPoint::new(at.0, at.1),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }
    }

    #[test]
    fn an_entry_sits_on_its_nearest_edge_at_its_place_along_it() {
        let s = sheet((40.0, 50.0), Vec::new());
        assert_eq!(
            entry_place(&s, &pin("A", "input", (10.0, 45.0))),
            (Edge::Left, 0.5)
        );
        assert_eq!(
            entry_place(&s, &pin("B", "output", (50.0, 20.0))).0,
            Edge::Right
        );
        assert_eq!(
            entry_place(&s, &pin("C", "passive", (30.0, 70.0))),
            (Edge::Bottom, 0.5)
        );
    }

    #[test]
    fn the_box_keeps_the_symbols_aspect_inside_the_view() {
        let wide = symbol_box((100.0, 10.0));
        assert!((wide.width / wide.height - 10.0).abs() < 1e-3);
        assert!(wide.width <= BOX_MAX_W + 1e-3 && wide.height <= BOX_MAX_H + 1e-3);
        let tall = symbol_box((10.0, 100.0));
        assert!((tall.height - BOX_MAX_H).abs() < 1e-3);
    }

    #[test]
    fn directions_read_from_their_stored_words() {
        assert_eq!(entry_direction("input"), EntryDirection::Input);
        assert_eq!(entry_direction("Output"), EntryDirection::Output);
        assert_eq!(entry_direction("tri_state"), EntryDirection::TriState);
        assert_eq!(entry_direction(""), EntryDirection::Unspecified);
        assert_eq!(entry_direction("weird"), EntryDirection::Other);
    }

    /// Each entry adds its shape and its name; the box, name, file, both
    /// dimensions and the Line Width callout are always there.
    #[test]
    fn the_drawing_has_a_shape_and_a_name_per_entry() {
        let i18n = I18n::en_us();
        let s = sheet(
            (40.0, 50.0),
            vec![
                pin("VIN", "input", (10.0, 30.0)),
                pin("PG", "output", (50.0, 40.0)),
            ],
        );
        let drawing = sheet_symbol_illustration(&s, &i18n);
        let entries = drawing
            .visible_parts()
            .filter(|p| p.is_linked_to(&SHEET_ENTRIES))
            .count();
        assert_eq!(entries, 4);
        assert_eq!(drawing.visible_parts().count(), 4 + 6);
    }
}
