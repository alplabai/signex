//! Line-art illustrations under the Properties header (epic #2523, BB2): a
//! wire or bus, a net label, a port, a power port and a text note. Each
//! builder turns the selected object into plain `Illustration` data, with
//! callouts named after the rows below; [`show`] draws it. Rotation is never
//! drawn, so no builder takes one.

mod net_label;
mod port;
mod power_port;
mod text_note;
mod wire;

pub(super) use net_label::net_label;
pub(super) use port::port;
pub(super) use power_port::{power_port, style_glyph};
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
    use signex_widgets::property_illustration::{Ink, Paint, Primitive};

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
        let at = |preset| band_width_of(&run(RunShape::Wire, preset, "1 mm".into(), &en));
        assert!(at(WireWidthPreset::Smallest) < at(WireWidthPreset::Small));
        assert!(at(WireWidthPreset::Medium) < at(WireWidthPreset::Large));
        for shape in [RunShape::Wire, RunShape::Bus] {
            let drawing = run(shape, WireWidthPreset::Small, "1 mm".into(), &en);
            assert!(has_no_arc(&drawing), "{shape:?}");
            for field in [fields::START, fields::END, fields::LENGTH, fields::WIDTH] {
                let linked = drawing.visible_parts().any(|p| p.is_linked_to(&field));
                assert!(linked, "{shape:?} lacks a {field} callout");
            }
        }
    }

    #[test]
    fn a_power_port_is_drawn_in_the_power_colour() {
        let en = I18n::en_us();
        let red = iced::Color::from_rgb8(0xC0, 0x39, 0x2B);
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            let drawing = power_port(&lib_id, "+3V3", red, &en);
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
