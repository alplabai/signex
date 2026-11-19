//! The drawing illustrations (m102, `R.graphic`): a line with its Width and
//! ends, a circle or arc with its Radius and Width, a rectangle or text
//! frame with its Width and Height (and a frame's Text Height).

use iced::Point;
use signex_i18n::{I18n, ids, t};
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_widgets::property_illustration::{
    Anchor, Illustration, Ink, Part, Primitive, Style, TextRole, Tint, Weight,
};

use super::{at, band, callout, canvas, cross, keys, label, layer_label, layer_note};
use crate::panels::pcb_primitive_properties::GraphicFacts;

/// The width and radius a mixed selection is drawn with, mm.
const STAND_IN_WIDTH_MM: f64 = 0.15;
const STAND_IN_RADIUS_MM: f64 = 1.6;

/// The illustration of drawings of one kind; `None` for a mixed selection.
pub(in crate::panels::pcb_primitive_properties) fn illustration(
    i18n: &I18n,
    f: &GraphicFacts,
) -> Option<Illustration> {
    let width = f.width.unwrap_or(STAND_IN_WIDTH_MM).max(0.001) as f32;
    let layer = f
        .layer
        .as_ref()
        .map_or(PcbLayer::Technical(TechnicalLayer::Notes), |l| l.layer);
    let paint = super::layer_paint(layer);
    let drawing = match f.kind? {
        BoardGraphicKind::Line => line(i18n, width, paint),
        BoardGraphicKind::Circle => round(i18n, f, width, paint, 360.0),
        BoardGraphicKind::Arc => round(i18n, f, width, paint, 180.0),
        BoardGraphicKind::Rectangle => frame(i18n, paint, false),
        BoardGraphicKind::TextFrame => frame(i18n, paint, true),
    };
    let name = f.layer.as_ref().map_or("", |l| l.label.as_str());
    Some(drawing.part(layer_note(i18n, &layer_label(i18n, layer, None, name))))
}

/// A line from Start to End: its band, the two end crosses and labels, and
/// the Width callout across its middle.
fn line(
    i18n: &I18n,
    width: f32,
    paint: signex_widgets::property_illustration::Paint,
) -> Illustration {
    let (a, b) = (at(44.0, 100.0), at(214.0, 40.0));
    let ws = (width * 40.0).clamp(2.0, 14.0);
    let length = (b.x - a.x).hypot(b.y - a.y);
    let (ux, uy) = ((b.x - a.x) / length, (b.y - a.y) / length);
    let (nx, ny) = (uy, -ux);
    let mid = at((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    canvas()
        .part(band(
            vec![a, b],
            ws,
            paint,
            &[keys::WIDTH, keys::LAYER, keys::START, keys::END, keys::ENDS],
        ))
        .parts(cross(a, 4.0, &[keys::START]))
        .parts(cross(b, 4.0, &[keys::END]))
        .part(label(
            at(a.x - 2.0, a.y + 14.0),
            t!(i18n, ids::PCB_PRIM_POINT_START),
            Anchor::Middle,
            TextRole::Note,
            &[keys::START],
        ))
        .part(label(
            at(b.x + 2.0, b.y - 12.0),
            t!(i18n, ids::PCB_PRIM_POINT_END),
            Anchor::Middle,
            TextRole::Note,
            &[keys::END],
        ))
        .part(callout(
            at(mid.x - nx * ws / 2.0, mid.y - ny * ws / 2.0),
            at(mid.x + nx * ws / 2.0, mid.y + ny * ws / 2.0),
            Some((
                t!(i18n, ids::PCB_PRIM_WIDTH).into(),
                at(mid.x - nx * 20.0, mid.y - ny * 20.0),
                Anchor::Middle,
            )),
            &[keys::WIDTH],
        ))
}

/// A circle (`sweep` 360) or an arc of a circle round the centre, with its
/// Radius and Width callouts.
fn round(
    i18n: &I18n,
    f: &GraphicFacts,
    width: f32,
    paint: signex_widgets::property_illustration::Paint,
    sweep: f32,
) -> Illustration {
    let (centre, r) = (at(132.0, 74.0), 46.0);
    let radius = f.radius.unwrap_or(STAND_IN_RADIUS_MM).max(0.01) as f32;
    let ws = (width / radius * r).clamp(1.5, 12.0);
    let point = |deg: f32, d: f32| {
        let (sin, cos) = deg.to_radians().sin_cos();
        at(centre.x + d * cos, centre.y - d * sin)
    };
    let steps = (sweep / 3.0).ceil() as usize;
    let points: Vec<Point> = (0..=steps)
        .map(|i| point(sweep * i as f32 / steps as f32, r))
        .collect();
    let rim = point(35.0, r);
    let left = if sweep >= 360.0 { 180.0 } else { 0.0 };
    let (inner, outer) = (point(left, r - ws / 2.0), point(left, r + ws / 2.0));
    let width_label_at = if sweep >= 360.0 {
        (at(outer.x - 12.0, outer.y), Anchor::End)
    } else {
        (at(outer.x + 10.0, outer.y), Anchor::Start)
    };
    canvas()
        .part(band(
            points,
            ws,
            paint,
            &[keys::RADIUS, keys::WIDTH, keys::LAYER],
        ))
        .parts(cross(centre, 4.0, &[keys::CENTRE]))
        .part(callout(
            centre,
            rim,
            Some((
                t!(i18n, ids::PCB_PRIM_RADIUS).into(),
                at(rim.x + 10.0, rim.y - 6.0),
                Anchor::Start,
            )),
            &[keys::RADIUS],
        ))
        .part(callout(
            inner,
            outer,
            Some((
                t!(i18n, ids::PCB_PRIM_WIDTH).into(),
                width_label_at.0,
                width_label_at.1,
            )),
            &[keys::WIDTH],
        ))
}

/// A rectangle or text frame: its box, its Width and Height callouts, and
/// for a frame the text lines and the Text Height callout.
fn frame(
    i18n: &I18n,
    paint: signex_widgets::property_illustration::Paint,
    has_text: bool,
) -> Illustration {
    let (x0, y0, x1, y1) = (40.0, 26.0, 224.0, 108.0);
    let outline = Part::new(
        Primitive::Rect {
            rect: iced::Rectangle::new(at(x0, y0), iced::Size::new(x1 - x0, y1 - y0)),
        },
        Style::edge().with_fill(paint, Tint::Light),
    )
    .linked_all([keys::START, keys::END, keys::WIDTH, keys::LAYER]);
    let mut drawing = canvas().part(outline);
    if has_text {
        for i in 0..3 {
            let y = y0 + 16.0 + i as f32 * 14.0;
            drawing = drawing.part(
                Part::new(
                    Primitive::Line {
                        from: at(x0 + 8.0, y),
                        to: at(x1 - 8.0 - i as f32 * 40.0, y),
                    },
                    Style::outline(Ink::Faint, Weight::Heavy),
                )
                .linked_all([keys::TEXT, keys::TEXT_HEIGHT]),
            );
        }
    }
    let size = |from: Point,
                to: Point,
                id: signex_i18n::MessageId,
                place: Point,
                anchor: Anchor,
                ext: [(Point, Point); 2]| {
        let mut c = signex_widgets::property_illustration::Callout::new(from, to)
            .label(t!(i18n, id))
            .label_at(place, anchor);
        for (a, b) in ext {
            c = c.extension(a, b);
        }
        Part::callout(c).linked_all([keys::START, keys::END])
    };
    drawing = drawing
        .part(size(
            at(x0, y0 - 8.0),
            at(x1, y0 - 8.0),
            ids::PCB_PRIM_WIDTH,
            at((x0 + x1) / 2.0, y0 - 16.0),
            Anchor::Middle,
            [
                (at(x0, y0 - 2.0), at(x0, y0 - 11.0)),
                (at(x1, y0 - 2.0), at(x1, y0 - 11.0)),
            ],
        ))
        .part(size(
            at(x1 + 9.0, y0),
            at(x1 + 9.0, y1),
            ids::PCB_PRIM_DRAWING_HEIGHT,
            at(x1 + 14.0, (y0 + y1) / 2.0),
            Anchor::Start,
            [
                (at(x1 + 2.0, y0), at(x1 + 12.0, y0)),
                (at(x1 + 2.0, y1), at(x1 + 12.0, y1)),
            ],
        ));
    if has_text {
        drawing = drawing.part(callout(
            at(x0 + 4.0, y0 + 10.0),
            at(x0 + 4.0, y0 + 16.0),
            Some((
                t!(i18n, ids::PCB_PRIM_TEXT_HEIGHT).into(),
                at(x0 - 2.0, y0 + 13.0),
                Anchor::End,
            )),
            &[keys::TEXT_HEIGHT],
        ));
    }
    drawing
}

#[cfg(test)]
mod tests {
    use pcb_geom::ItemRef;
    use uuid::Uuid;

    use super::super::fixture::{common, lights};
    use super::*;

    fn facts(kind: Option<BoardGraphicKind>) -> GraphicFacts {
        GraphicFacts {
            common: common(vec![ItemRef::Graphic {
                graphic: Uuid::nil(),
                kind: pcb_geom::PickKind::Other,
            }]),
            kind,
            layer: None,
            layers: Vec::new(),
            width: Some(0.15),
            start: (None, None),
            end: (None, None),
            center: (None, None),
            radius: Some(1.6),
            text: None,
            text_height: None,
        }
    }

    #[test]
    fn each_kind_lights_the_rows_it_shows() {
        let i18n = I18n::en_us();
        let line = illustration(&i18n, &facts(Some(BoardGraphicKind::Line))).unwrap();
        for key in [keys::WIDTH, keys::START, keys::END, keys::ENDS] {
            assert!(lights(&line, &key), "line {key}");
        }
        let circle = illustration(&i18n, &facts(Some(BoardGraphicKind::Circle))).unwrap();
        for key in [keys::RADIUS, keys::WIDTH, keys::CENTRE] {
            assert!(lights(&circle, &key), "circle {key}");
        }
        let frame = illustration(&i18n, &facts(Some(BoardGraphicKind::TextFrame))).unwrap();
        for key in [keys::START, keys::TEXT, keys::TEXT_HEIGHT] {
            assert!(lights(&frame, &key), "frame {key}");
        }
        let rectangle = illustration(&i18n, &facts(Some(BoardGraphicKind::Rectangle))).unwrap();
        assert!(
            !lights(&rectangle, &keys::TEXT_HEIGHT),
            "a rectangle has no text"
        );
    }

    #[test]
    fn a_mixed_selection_has_no_illustration() {
        assert!(illustration(&I18n::en_us(), &facts(None)).is_none());
    }
}
