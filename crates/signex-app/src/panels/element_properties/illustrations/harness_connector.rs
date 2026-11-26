//! A harness connector (mockup row 152): the body with its tip, each entry
//! on the flat side with the net wired into it, the harness wire leaving the
//! tip with the type, and the Width and Entry pitch callouts. The body keeps
//! the connector's width-to-pitch proportion within the drawing's bounds.
//! In the mismatch state an entry the far side expects under another name is
//! drawn in the warn colour, `expects CS_ADC` in its net's place (the Entries
//! list above the drawing still names the net).

use iced::Point;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::HarnessConnector;
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Paint, Part, Primitive, Style, Text, TextLabel, TextRole,
    Tint, Weight,
};

use super::{HEIGHT_EM, fields};

const VIEW_W: f32 = 296.0;
const VIEW_H: f32 = 160.0;
/// The flat side of the body; the nets run in from the left of it.
const BODY_X: f32 = 94.0;
/// The top of the body, under the Width callout.
const BODY_Y: f32 = 36.0;
/// The room the body may take, right of it the tip, the harness wire and
/// the Entry pitch callout, below it the margin.
const BODY_MAX_W: f32 = VIEW_W - 196.0;
const BODY_MAX_H: f32 = VIEW_H - BODY_Y - 30.0;
/// The smallest body drawn, so a lopsided connector stays readable.
const BODY_MIN: f32 = 24.0;
/// The largest tip cut.
const TIP_MAX: f32 = 14.0;
/// Entries drawn; more would not be legible at this size.
pub(super) const MAX_ENTRIES: usize = 6;
/// The entry glyph: its length along the wire and half its height.
const ENTRY_LEN: f32 = 10.0;
const ENTRY_HALF: f32 = 4.0;
/// Where the nets come in from.
const NET_X: f32 = 8.0;

/// What the drawing shows of a connector beyond the connector itself.
#[derive(Debug, Clone, Copy)]
pub struct HarnessArt<'a> {
    /// Per entry: the net wired into it, empty when none.
    pub nets: &'a [String],
    /// Per entry: the entry the far side expects in its place, if any.
    pub expects: &'a [Option<String>],
    /// The project's harness colour.
    pub colour: iced::Color,
    /// The colour a mismatched entry is drawn in.
    pub warn: iced::Color,
}

/// How one entry row is drawn: the note above its wire (the net, or what
/// the far side expects) and the inks of the note, the glyph and the name.
struct EntryRow {
    note: Text,
    note_style: Style,
    edge: Style,
    name_style: Style,
}

/// A label in the document colour `colour`.
fn coloured_text(colour: iced::Color) -> Style {
    Style::outline(Ink::Secondary, Weight::Thin).with_stroke_paint(Paint::Document(colour))
}

/// The connector `c` with the nets and the mismatches of `art`.
pub fn harness_connector(c: &HarnessConnector, art: HarnessArt<'_>, i18n: &I18n) -> Illustration {
    let shown = c.entries.len().min(MAX_ENTRIES);
    let slots = (shown + 1) as f32;
    let size = |mm: f64| {
        if mm.is_finite() && mm > 0.0 {
            mm as f32
        } else {
            1.0
        }
    };
    let (width, pitch) = (size(c.width), size(c.entry_pitch));
    let scale = (BODY_MAX_H / (slots * pitch)).min(BODY_MAX_W / width);
    let body_w = (width * scale).clamp(BODY_MIN, BODY_MAX_W);
    let body_h = (slots * pitch * scale).clamp(BODY_MIN, BODY_MAX_H);
    let step = body_h / slots;
    let tip = TIP_MAX.min(body_h / 3.0).min(body_w / 2.0);
    let (left, top, right, bottom) = (BODY_X, BODY_Y, BODY_X + body_w, BODY_Y + body_h);
    let harness = Paint::Document(art.colour);
    let harness_edge = Style::edge().with_stroke_paint(harness);
    let body = Part::new(
        Primitive::Polyline {
            points: vec![
                Point::new(left, top),
                Point::new(right - tip, top),
                Point::new(right, top + tip),
                Point::new(right, bottom - tip),
                Point::new(right - tip, bottom),
                Point::new(left, bottom),
            ],
            is_closed: true,
        },
        harness_edge.with_fill(harness, Tint::Light),
    );
    let mut parts = vec![body];
    for (i, entry) in c.entries.iter().take(shown).enumerate() {
        let y = top + (i + 1) as f32 * step;
        let row = match art.expects.get(i).and_then(Option::as_deref) {
            Some(expects) => EntryRow {
                note: t!(i18n, ids::PANELS_HARNESS_EXPECTS, "expects" => expects).into(),
                note_style: coloured_text(art.warn),
                edge: Style::outline(Ink::Secondary, Weight::Heavy)
                    .with_stroke_paint(Paint::Document(art.warn))
                    .with_fill(Paint::Document(art.warn), Tint::Light),
                name_style: coloured_text(art.warn),
            },
            None => EntryRow {
                note: UserText::new(art.nets.get(i).cloned().unwrap_or_default()).into(),
                note_style: Style::outline(Ink::Tertiary, Weight::Thin),
                edge: harness_edge,
                name_style: Style::outline(Ink::Secondary, Weight::Thin),
            },
        };
        parts.extend(entry_parts(&entry.name, row, y));
    }
    let mid = (top + bottom) / 2.0;
    parts.push(Part::new(
        Primitive::Line {
            from: Point::new(right, mid),
            to: Point::new(VIEW_W - NET_X, mid),
        },
        Style::outline(Ink::Secondary, Weight::Heavy).with_stroke_paint(harness),
    ));
    // Under the wire: the Entry pitch callout takes the room above it.
    parts.push(Part::new(
        Primitive::Label(TextLabel {
            at: Point::new(VIEW_W - NET_X, mid + 10.0),
            text: UserText::new(c.harness_type.clone()).into(),
            anchor: Anchor::End,
            role: TextRole::Mono,
        }),
        Style::outline(Ink::Secondary, Weight::Thin).with_stroke_paint(harness),
    ));
    let width_y = top - 11.0;
    let width_callout = Callout::new(Point::new(left, width_y), Point::new(right, width_y))
        .label(t!(i18n, ids::PANELS_HARNESS_WIDTH))
        .label_at(
            Point::new((left + right) / 2.0, width_y - 5.0),
            Anchor::Middle,
        )
        .extension(Point::new(left, top - 2.0), Point::new(left, width_y - 3.0))
        .extension(
            Point::new(right, top + tip - 2.0),
            Point::new(right, width_y - 3.0),
        );
    // From the top edge to the first entry: one pitch, as on the sheet.
    let pitch_x = right + 14.0;
    let first = top + step;
    let pitch_callout = Callout::new(Point::new(pitch_x, top), Point::new(pitch_x, first))
        .label(t!(i18n, ids::PANELS_HARNESS_PITCH))
        .label_at(
            Point::new(pitch_x + 6.0, (top + first) / 2.0),
            Anchor::Start,
        )
        .extension(
            Point::new(right - tip + 2.0, top),
            Point::new(pitch_x + 3.0, top),
        )
        .extension(
            Point::new(left + ENTRY_LEN + 2.0, first),
            Point::new(pitch_x + 3.0, first),
        );
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .parts(parts)
        .part(Part::callout(width_callout).linked(fields::WIDTH))
        .part(Part::callout(pitch_callout).linked(fields::ENTRY_PITCH))
}

/// One entry at height `y`: the net's wire running in, the row's note above
/// the wire, the entry glyph on the flat side and the entry's name.
fn entry_parts(name: &str, row: EntryRow, y: f32) -> Vec<Part> {
    let wire = Part::new(
        Primitive::Line {
            from: Point::new(NET_X, y),
            to: Point::new(BODY_X, y),
        },
        Style::outline(Ink::Secondary, Weight::Thin),
    );
    let glyph = Part::new(
        Primitive::Polyline {
            points: vec![
                Point::new(BODY_X, y - ENTRY_HALF),
                Point::new(BODY_X + ENTRY_LEN - 3.0, y - ENTRY_HALF),
                Point::new(BODY_X + ENTRY_LEN, y),
                Point::new(BODY_X + ENTRY_LEN - 3.0, y + ENTRY_HALF),
                Point::new(BODY_X, y + ENTRY_HALF),
            ],
            is_closed: true,
        },
        row.edge,
    );
    let entry = Part::new(
        Primitive::Label(TextLabel {
            at: Point::new(BODY_X + ENTRY_LEN + 3.0, y),
            text: UserText::new(name.to_string()).into(),
            anchor: Anchor::Start,
            role: TextRole::Caption,
        }),
        row.name_style,
    );
    let mut parts = vec![wire, glyph, entry];
    if !row.note.as_str().is_empty() {
        parts.push(Part::new(
            Primitive::Label(TextLabel {
                at: Point::new(NET_X + 2.0, y - 6.0),
                text: row.note,
                anchor: Anchor::Start,
                role: TextRole::Note,
            }),
            row.note_style,
        ));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{HarnessEntry, Point as SheetPoint};
    use signex_widgets::property_illustration::FieldId;

    fn connector(width: f64, pitch: f64, names: &[&str]) -> HarnessConnector {
        HarnessConnector {
            uuid: uuid::Uuid::from_u128(1),
            harness_type: "SPI_BUS".into(),
            position: SheetPoint::new(63.5, 16.51),
            width,
            entry_pitch: pitch,
            rotation: 0.0,
            entries: names
                .iter()
                .enumerate()
                .map(|(i, n)| HarnessEntry {
                    uuid: uuid::Uuid::from_u128(10 + i as u128),
                    name: (*n).into(),
                })
                .collect(),
            locked: false,
        }
    }

    fn spi() -> HarnessConnector {
        connector(10.16, 5.08, &["SCK", "MOSI", "MISO", "CS"])
    }

    fn nets() -> Vec<String> {
        ["SPI_SCK", "SPI_MOSI", "SPI_MISO", "SPI_CS"]
            .map(String::from)
            .to_vec()
    }

    fn colour() -> iced::Color {
        iced::Color::from_rgb8(0x6B, 0x4F, 0xA0)
    }

    fn warn() -> iced::Color {
        iced::Color::from_rgb8(0x8A, 0x5A, 0x10)
    }

    fn art<'a>(nets: &'a [String], expects: &'a [Option<String>]) -> HarnessArt<'a> {
        HarnessArt {
            nets,
            expects,
            colour: colour(),
            warn: warn(),
        }
    }

    fn callout(drawing: &Illustration, field: &FieldId) -> Callout {
        drawing
            .visible_parts()
            .filter(|p| p.is_linked_to(field))
            .find_map(|p| match &p.primitive {
                Primitive::Callout(c) => Some(c.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no {field} callout"))
    }

    fn labels(drawing: &Illustration) -> Vec<String> {
        drawing
            .visible_parts()
            .filter_map(|p| match &p.primitive {
                Primitive::Label(l) => Some(l.text.as_str().to_string()),
                _ => None,
            })
            .collect()
    }

    /// Mockup row 152: Width and Entry pitch callouts, named like the
    /// Placement rows, and no turn drawn.
    #[test]
    fn the_connector_carries_width_and_entry_pitch_callouts() {
        let en = I18n::en_us();
        let drawing = harness_connector(&spi(), art(&nets(), &[]), &en);
        let width = callout(&drawing, &fields::WIDTH);
        let pitch = callout(&drawing, &fields::ENTRY_PITCH);
        assert_eq!(
            width.label.as_ref().map(|l| l.as_str().to_string()),
            Some(t!(en, ids::PANELS_HARNESS_WIDTH).into_string())
        );
        assert_eq!(
            pitch.label.as_ref().map(|l| l.as_str().to_string()),
            Some(t!(en, ids::PANELS_HARNESS_PITCH).into_string())
        );
        assert_eq!(width.from.y, width.to.y, "Width runs across");
        assert_eq!(pitch.from.x, pitch.to.x, "Entry pitch runs down");
        let arcs = drawing
            .visible_parts()
            .any(|p| matches!(p.primitive, Primitive::Arc { .. }));
        assert!(!arcs);
    }

    /// Each entry, the net wired into it and the harness type are drawn; the
    /// body and the harness wire take the harness colour.
    #[test]
    fn entries_nets_and_the_type_are_drawn_in_the_harness_colour() {
        let en = I18n::en_us();
        let drawing = harness_connector(&spi(), art(&nets(), &[]), &en);
        let shown = labels(&drawing);
        for text in ["SCK", "MOSI", "MISO", "CS", "SPI_SCK", "SPI_CS", "SPI_BUS"] {
            assert!(shown.iter().any(|l| l.contains(text)), "{text}: {shown:?}");
        }
        let in_colour = drawing
            .visible_parts()
            .filter(|p| {
                p.style
                    .stroke
                    .is_some_and(|s| s.paint == Paint::Document(colour()))
            })
            .count();
        // The body, four entry glyphs, the harness wire and the type.
        assert_eq!(in_colour, 7);
    }

    /// An entry with no wire shows no net; a long connector draws only the
    /// first entries it can fit.
    #[test]
    fn an_unwired_entry_shows_no_net_and_long_connectors_are_cut() {
        let en = I18n::en_us();
        let names: Vec<String> = (1..=10).map(|n| format!("E{n}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let drawing = harness_connector(&connector(10.16, 2.54, &refs), art(&[], &[]), &en);
        let shown = labels(&drawing);
        let entries = shown.iter().filter(|l| l.starts_with('E')).count();
        assert_eq!(entries, MAX_ENTRIES, "{shown:?}");
    }

    /// The Width callout follows the connector's width against its pitch.
    #[test]
    fn a_wider_connector_draws_a_wider_body() {
        let en = I18n::en_us();
        let span = |width| {
            let c = connector(width, 5.08, &["SCK", "MOSI"]);
            let w = callout(&harness_connector(&c, art(&[], &[]), &en), &fields::WIDTH);
            w.to.x - w.from.x
        };
        assert!(span(5.08) < span(10.16));
        let bad = connector(f64::NAN, 0.0, &["SCK"]);
        let w = callout(&harness_connector(&bad, art(&[], &[]), &en), &fields::WIDTH);
        assert!((w.to.x - w.from.x).is_finite() && w.to.x > w.from.x);
    }

    fn in_paint(drawing: &Illustration, paint: Paint) -> Vec<&Primitive> {
        drawing
            .visible_parts()
            .filter(|p| p.style.stroke.is_some_and(|s| s.paint == paint))
            .map(|p| &p.primitive)
            .collect()
    }

    /// Mockup row 152, Entry mismatch: the mismatched entry's glyph and name
    /// turn warn and `expects CS_ADC` takes its net's place; the other
    /// entries keep the harness colour and their nets.
    #[test]
    fn a_mismatched_entry_is_drawn_warn_with_what_the_far_side_expects() {
        let en = I18n::en_us();
        let expects = [None, None, None, Some("CS_ADC".to_string())];
        let drawing = harness_connector(&spi(), art(&nets(), &expects), &en);
        let caption = t!(en, ids::PANELS_HARNESS_EXPECTS, "expects" => "CS_ADC").into_string();
        let warned = in_paint(&drawing, Paint::Document(warn()));
        let warned_labels: Vec<&str> = warned
            .iter()
            .filter_map(|p| match p {
                Primitive::Label(l) => Some(l.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(warned_labels, ["CS", caption.as_str()], "{warned_labels:?}");
        let warned_glyphs = warned
            .iter()
            .filter(|p| matches!(p, Primitive::Polyline { .. }))
            .count();
        assert_eq!(warned_glyphs, 1);
        let shown = labels(&drawing);
        assert!(!shown.iter().any(|l| l == "SPI_CS"), "{shown:?}");
        assert!(shown.iter().any(|l| l == "SPI_MISO"), "{shown:?}");
        // The body, three entry glyphs, the harness wire and the type.
        assert_eq!(in_paint(&drawing, Paint::Document(colour())).len(), 6);
    }
}
