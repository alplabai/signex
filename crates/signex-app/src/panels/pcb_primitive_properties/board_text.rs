//! The String panel (#2373, #2523, mockup row 100): under the illustration, the text
//! across the full width, its layer, position, height and rotation, and
//! quick rotation steps.

use iced::widget::container;
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};

use super::drawing::text as dt;
use super::linked::{
    Segment, illustration, linked, linked_row, prefixed, segments, two_fields, unit, with_unit,
};
use super::rows::{
    Look, PAD_X, Section, any_layer_row, field, input, labelled, labelled_text, stack,
    sunken_value, with_tip,
};
use super::{PcbPrimitiveMsg, PrimitiveField as F, SP, TS, TextFacts, format_number};
use crate::panels::{PanelContext, PanelMsg, form_look};

/// The quick rotation steps, in degrees.
const QUICK_TURNS: [u32; 4] = [0, 90, 180, 270];

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a TextFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let drawing = illustration(look, &f.common, dt::illustration(f, &ctx.i18n));
    stack(vec![Some(drawing), properties(look, f)])
}

fn properties<'a>(look: Look<'a>, f: &'a TextFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES).collapsible(
        "pcb_prim.text.properties",
        false,
        None,
    );
    // The text first, across the full width and a step taller.
    let text = input(common, F::Text, f.text.clone(), look)
        .size(TS.label)
        .padding([SP.s + SP.xs, SP.s])
        .style(form_look::text_input_style);
    s.push(
        common,
        ids::PCB_PRIM_TEXT,
        linked(
            look,
            common,
            dt::TEXT,
            container(text)
                .padding([SP.xs, PAD_X])
                .width(Length::Fill)
                .boxed(),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        linked(
            look,
            common,
            dt::LAYER,
            any_layer_row(look, common, &f.layers, f.layer.as_ref()),
        ),
    );
    // A fact, not a field: the layer's side decides it (mockup row 100).
    if f.is_mirrored() {
        s.push(
            common,
            ids::PCB_PRIM_STRING_MIRRORED,
            labelled_text(
                look,
                UserText::new(String::new()),
                sunken_value(look, look.t(ids::PCB_PRIM_STRING_MIRRORED)),
            ),
        );
    }
    // Where a string not placed yet goes is the pointer's.
    if !common.target.is_placement() {
        s.push(
            common,
            ids::PCB_PRIM_STRING_POSITION,
            linked_row(
                look,
                common,
                dt::POSITION,
                ids::PCB_PRIM_STRING_POSITION,
                two_fields(
                    prefixed(look, common, F::X, f.x, ids::PCB_PRIM_X),
                    prefixed(look, common, F::Y, f.y, ids::PCB_PRIM_Y),
                    unit(look, ids::PCB_PRIM_UNIT_MM),
                ),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_TEXT_HEIGHT,
        linked_row(
            look,
            common,
            dt::HEIGHT,
            ids::PCB_PRIM_TEXT_HEIGHT,
            with_unit(
                field(
                    common,
                    F::TextHeight,
                    f.height.map(format_number),
                    None,
                    look,
                ),
                unit(look, ids::PCB_PRIM_UNIT_MM),
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_ROTATION,
        labelled(
            look,
            ids::PCB_PRIM_ROTATION,
            with_unit(
                field(
                    common,
                    F::Rotation,
                    f.rotation.map(format_number),
                    None,
                    look,
                ),
                unit(look, ids::PCB_PRIM_UNIT_DEGREES),
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_ROTATION,
        labelled_text(
            look,
            UserText::new(String::new()),
            with_tip(
                look,
                quick_turns(look, f),
                ids::PCB_PRIM_STRING_QUICK_ROTATION,
            ),
        ),
    );
    s.finish()
}

/// 0°, 90°, 180° and 270°: a pick sets the Rotation as one undo step; the
/// step the rotation equals is on.
fn quick_turns<'a>(look: Look<'a>, f: &'a TextFacts) -> Element<'a, PanelMsg> {
    let common = &f.common;
    let turned = f.rotation.map(|r| r.rem_euclid(360.0));
    let cells = QUICK_TURNS
        .into_iter()
        .map(|degrees| Segment {
            label: t!(
                look.ctx.i18n,
                ids::PCB_PRIM_STRING_ROTATION_STEP,
                "degrees" => degrees
            ),
            is_on: turned.is_some_and(|r| (r - f64::from(degrees)).abs() < 1e-9),
            msg: common.is_editable().then(|| PcbPrimitiveMsg::ValuePicked {
                target: common.target.clone(),
                field: F::Rotation,
                value: f64::from(degrees),
            }),
        })
        .collect();
    segments(cells)
}

#[cfg(test)]
mod tests {
    use pcb_geom::ItemRef;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
    use uuid::Uuid;

    use super::*;
    use crate::layout_probe::lay_out_texts;
    use crate::panels::pcb_primitive_properties::AreaLayerChoice;
    use crate::panels::pcb_primitive_properties::drawing::fixture;

    fn on(layer: Option<PcbLayer>) -> TextFacts {
        TextFacts {
            common: fixture::common(vec![ItemRef::Text(Uuid::from_u128(1))]),
            text: Some("SENSOR-NODE rev B".into()),
            x: Some(4.0),
            y: Some(38.2),
            height: Some(1.0),
            rotation: Some(0.0),
            layer: layer.map(|layer| AreaLayerChoice {
                layer,
                label: format!("{layer:?}"),
            }),
            layers: Vec::new(),
        }
    }

    /// Mockup row 100 (Caner, 2026-11-24): a string reads mirrored when its
    /// layer is on the bottom side; a top, board-wide or mixed layer is not.
    #[test]
    fn a_string_on_a_bottom_layer_is_mirrored() {
        let bottom_silk = PcbLayer::Technical(TechnicalLayer::SilkscreenBottom);
        assert!(on(Some(bottom_silk)).is_mirrored());
        assert!(on(Some(PcbLayer::Copper(CopperLayer::Bottom))).is_mirrored());
        let top_silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
        assert!(!on(Some(top_silk)).is_mirrored());
        let mechanical = signex_types::pcb_layer::MechanicalLayer::new(1).unwrap();
        assert!(!on(Some(PcbLayer::Mechanical(mechanical))).is_mirrored());
        assert!(!on(None).is_mirrored(), "the strings' layers differ");
    }

    /// The panel says "Mirrored (bottom side)" for a bottom string only.
    #[test]
    fn the_panel_shows_the_mirrored_fact_for_a_bottom_string() {
        let (app, _boot) = crate::app::Signex::new();
        let ctx = app.document_state.panel_ctx.clone();
        let fact = t!(ctx.i18n, ids::PCB_PRIM_STRING_MIRRORED).into_string();
        let shows = |f: &TextFacts| {
            let section = properties(Look::of(&ctx), f).expect("the section");
            lay_out_texts(section, iced::Size::new(300.0, 1200.0))
                .iter()
                .any(|t| t.text == fact)
        };
        let bottom = on(Some(PcbLayer::Technical(TechnicalLayer::SilkscreenBottom)));
        assert!(shows(&bottom), "a bottom string is mirrored");
        let top = on(Some(PcbLayer::Technical(TechnicalLayer::SilkscreenTop)));
        assert!(!shows(&top), "a top string is not");
    }
}
