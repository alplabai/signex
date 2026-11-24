//! Line-art illustrations under the Properties header (epic #2523, mockup rows 40 to 51):
//! a part, a pin, a footprint thumbnail, a wire or bus, a net label, a port,
//! a power port, a text note, a sheet symbol and a drawing shape. Each
//! builder turns the selected object into plain `Illustration` data, with
//! callouts named after the rows below; [`show`] draws it. They are pure, so
//! the tests read the parts they produce instead of pixels. Rotation is never
//! drawn, so no builder takes one.

pub(super) mod drawing_shape;
pub(super) mod footprint_thumb;
pub(super) mod part;
pub(super) mod pin;
pub(super) mod sheet_symbol;

mod net_label;
mod port;
mod power_port;
mod text_note;
mod wire;

pub(super) use net_label::net_label;
pub(super) use port::port;
pub(super) use power_port::{PowerPortCallouts, power_port, style_glyph};
pub(super) use text_note::text_note;
pub(super) use wire::{RunShape, run};

use super::super::*;
use crate::styles::{Spacing, TypeScale};
use signex_widgets::property_illustration::{
    Illustration, IllustrationHover, property_illustration,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of a Properties illustration, in captions.
pub(super) const HEIGHT_EM: f32 = 10.0;

/// The field ids the callouts carry, named after the rows that hold the
/// values.
pub(super) mod fields {
    use signex_widgets::property_illustration::FieldId;

    pub const START: FieldId = FieldId::new("start");
    pub const END: FieldId = FieldId::new("end");
    pub const LENGTH: FieldId = FieldId::new("length");
    pub const WIDTH: FieldId = FieldId::new("width");
    pub const NET_NAME: FieldId = FieldId::new("net_name");
    pub const PORT_NAME: FieldId = FieldId::new("port_name");
    pub const NAME: FieldId = FieldId::new("name");
    pub const STYLE: FieldId = FieldId::new("style");
    pub const FONT_SIZE: FieldId = FieldId::new("font_size");
    pub const POSITION: FieldId = FieldId::new("position");
    pub const TEXT: FieldId = FieldId::new("text");
    pub const TEXT_SIZE: FieldId = FieldId::new("text_size");
}

/// `illustration` drawn on the panel ground, full width.
///
/// The hover link to the rows needs the panel's hover state, which the
/// Properties panel does not keep yet; until it does, the pointer's reports
/// are dropped and nothing is highlighted.
pub(super) fn show<'a>(illustration: Illustration, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let hover = IllustrationHover::default();
    let drawing = property_illustration(illustration, &hover, &ctx.tokens, TS.caption)
        .map(|_| PanelMsg::Noop);
    container(drawing)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::I18n;
    use signex_types::schematic::{HAlign, PowerPortStyle, VAlign, WireWidthPreset};
    use signex_widgets::property_illustration::{Ink, Paint, Primitive, Text};

    /// No builder draws a turn: a rotation would show as an arc.
    fn has_no_arc(illustration: &Illustration) -> bool {
        (illustration.visible_parts()).all(|p| !matches!(p.primitive, Primitive::Arc { .. }))
    }

    fn band_width_of(illustration: &Illustration) -> f32 {
        let band = illustration
            .visible_parts()
            .find_map(|p| match p.primitive {
                Primitive::Band { width, .. } => Some(width),
                _ => None,
            });
        band.expect("the run is drawn as a band")
    }

    #[test]
    fn a_run_is_drawn_at_its_width_preset() {
        let en = I18n::en_us();
        let at = |preset| band_width_of(&run(RunShape::Horizontal, preset, "1 mm".into(), &en));
        assert!(at(WireWidthPreset::Smallest) < at(WireWidthPreset::Small));
        assert!(at(WireWidthPreset::Medium) < at(WireWidthPreset::Large));
        for shape in [RunShape::Horizontal, RunShape::Vertical] {
            let drawing = run(shape, WireWidthPreset::Small, "1 mm".into(), &en);
            assert!(has_no_arc(&drawing), "{shape:?}");
            for field in [fields::START, fields::END, fields::LENGTH, fields::WIDTH] {
                let linked = drawing.visible_parts().any(|p| p.is_linked_to(&field));
                assert!(linked, "{shape:?} lacks a {field} callout");
            }
        }
    }

    /// Mockup row 43: the preview lies the way the run lies on the sheet,
    /// whichever kind it is.
    #[test]
    fn a_run_is_drawn_the_way_it_runs_on_the_sheet() {
        use signex_types::schematic::Point;
        let at = |x, y| Point::new(x, y);
        assert_eq!(
            RunShape::of_segment(at(160.0, 169.0), at(190.5, 169.0)),
            RunShape::Horizontal
        );
        assert_eq!(
            RunShape::of_segment(at(63.5, 50.8), at(63.5, 101.6)),
            RunShape::Vertical
        );
    }

    #[test]
    fn a_power_port_is_drawn_in_the_power_colour() {
        let en = I18n::en_us();
        let red = iced::Color::from_rgb8(0xC0, 0x39, 0x2B);
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            let callouts = PowerPortCallouts::new(&en, style.label(), 10, None);
            let drawing = power_port(&lib_id, "+3V3", red, callouts);
            assert!(has_no_arc(&drawing), "{style:?}");
            let red_parts = drawing.visible_parts().filter(|p| {
                p.style
                    .stroke
                    .is_some_and(|s| s.paint == Paint::Document(red))
            });
            let (mut glyph, mut name) = (false, false);
            for part in red_parts {
                glyph |= part.is_linked_to(&fields::STYLE);
                name |= matches!(part.primitive, Primitive::Label(_));
            }
            assert!(glyph && name, "{style:?}: glyph {glyph}, name {name}");
        }
    }

    /// Every label text the drawing carries, the bidi marks around
    /// arguments removed.
    fn label_texts(illustration: &Illustration) -> Vec<String> {
        let text = |t: &Text| t.as_str().replace(['\u{2068}', '\u{2069}'], "");
        (illustration.visible_parts())
            .filter_map(|p| match &p.primitive {
                Primitive::Leader(leader) => Some(text(&leader.label)),
                Primitive::Callout(callout) => callout.label.as_ref().map(text),
                Primitive::Label(label) => Some(text(&label.text)),
                _ => None,
            })
            .collect()
    }

    /// Mockup row 45: the callouts carry the row values (Style · Bar, the
    /// font size, Position x, y) and no bare field names.
    #[test]
    fn a_power_port_callouts_carry_their_values() {
        let en = I18n::en_us();
        let red = iced::Color::from_rgb8(0xC0, 0x39, 0x2B);
        let at = Some(("152.40".to_string(), "50.80".to_string()));
        let callouts = PowerPortCallouts::new(&en, "Bar", 10, at);
        let texts = label_texts(&power_port("power:bar", "+3V3", red, callouts));
        for wanted in [
            "+3V3",
            "Style · Bar",
            "Font Size 10",
            "Position 152.40, 50.80",
        ] {
            assert!(texts.iter().any(|t| t == wanted), "{wanted}: {texts:?}");
        }
        for bare in ["Name", "Style", "Font Size", "Position"] {
            assert!(!texts.iter().any(|t| t == bare), "{bare}: {texts:?}");
        }
    }

    /// Mockup row 45: the port sits on a wire stub drawn at the wire's
    /// width, running from the hotspot away from the head.
    #[test]
    fn a_power_port_sits_on_a_wire_stub() {
        let en = I18n::en_us();
        let red = iced::Color::from_rgb8(0xC0, 0x39, 0x2B);
        for (lib_id, runs_down) in [("power:bar", true), ("power:GND", false)] {
            let callouts = PowerPortCallouts::new(&en, "Bar", 10, None);
            let drawing = power_port(lib_id, "+3V3", red, callouts);
            assert_eq!(
                band_width_of(&drawing),
                wire::band_width(WireWidthPreset::Small),
                "{lib_id}"
            );
            let band = drawing.visible_parts().find_map(|p| match &p.primitive {
                Primitive::Band { points, .. } => Some(points.clone()),
                _ => None,
            });
            let points = band.expect("a stub");
            let goes_down = points[1].y > points[0].y;
            assert_eq!(goes_down, runs_down, "{lib_id}");
        }
    }

    #[test]
    fn each_style_tile_draws_a_glyph() {
        for style in PowerPortStyle::ALL {
            let tile = style_glyph(&format!("power:{}", style.lib_tag()), Ink::Accent);
            assert!(
                tile.visible_parts().count() >= 2,
                "{style:?}: stem and head"
            );
        }
    }

    #[test]
    fn a_net_label_and_a_port_carry_their_callouts() {
        let en = I18n::en_us();
        for justify in [HAlign::Left, HAlign::Center, HAlign::Right] {
            let drawing = net_label("SPI_CLK", justify, &en);
            assert!(has_no_arc(&drawing));
            for field in [fields::NET_NAME, fields::FONT_SIZE, fields::POSITION] {
                assert!(drawing.visible_parts().any(|p| p.is_linked_to(&field)));
            }
        }
        let drawing = port("USB_DP", &en);
        assert!(
            drawing
                .visible_parts()
                .any(|p| p.is_linked_to(&fields::PORT_NAME))
        );
    }

    /// The anchor dot follows Justify H and V; the Position leader starts on
    /// the frame's bottom edge under it.
    #[test]
    fn a_text_note_marks_the_anchor_its_justification_names() {
        let en = I18n::en_us();
        let dot = |h, v| {
            let drawing = text_note("Place C14 near U3", h, v, &en);
            assert!(has_no_arc(&drawing));
            drawing
                .visible_parts()
                .find_map(|p| match p.primitive {
                    Primitive::Circle { center, .. } => Some(center),
                    _ => None,
                })
                .expect("an anchor dot")
        };
        let top_left = dot(HAlign::Left, VAlign::Top);
        let bottom_right = dot(HAlign::Right, VAlign::Bottom);
        assert!(top_left.x < bottom_right.x && top_left.y < bottom_right.y);
        let drawing = text_note("x", HAlign::Center, VAlign::Top, &en);
        let leader = drawing.visible_parts().find_map(|p| match &p.primitive {
            Primitive::Leader(leader) => Some(leader.at),
            _ => None,
        });
        let centre = dot(HAlign::Center, VAlign::Top);
        let at = leader.expect("a Position leader");
        assert_eq!(at.x, centre.x);
        assert!(at.y > centre.y, "it leaves from the bottom edge");
    }

    #[test]
    fn note_text_wraps_at_words_and_keeps_its_breaks() {
        assert_eq!(
            text_note::wrap("one two three\nfour", 8, 4),
            vec!["one two", "three", "four"]
        );
        assert_eq!(
            text_note::wrap("abcdefghij", 4, 4),
            vec!["abcd", "efgh", "ij"]
        );
        let cut = text_note::wrap("a\nb\nc", 4, 2);
        assert_eq!(cut, vec!["a", "b\u{2026}"]);
    }
}
