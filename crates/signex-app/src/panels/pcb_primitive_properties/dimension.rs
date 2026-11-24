//! The Dimension panel (#2373, #2523, mockup row 101): the illustration, the measured
//! Value on a card under it, then where it stands (Start, End, Offset) and
//! its Style: line width, text height, decimals and layer; then its Arrows,
//! Text, Extension Lines, and Units and Format (`dimension_style`, #2868).

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};

use super::drawing::dimension as dd;
use super::linked::{
    Segment, illustration, linked, linked_row, prefixed, segments, two_fields, unit, with_unit,
};
use super::rows::{Look, PAD_X, Section, any_layer_row, mm_field, stack};
use super::stack_rows::caption;
use super::{DimensionFacts, PcbPrimitiveMsg, PrimitiveField as F, SP, TS};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::resolve;

/// The most decimals a dimension's value shows.
const MAX_PRECISION: u32 = 6;

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a DimensionFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let drawing = illustration(look, &f.common, dd::illustration(f, &ctx.i18n));
    let mut sections = vec![Some(drawing), value_card(look, f), properties(look, f)];
    sections.extend(super::dimension_style::sections(look, f));
    stack(sections)
}

/// The measured value as a headline card; none for a dimension not placed
/// yet.
fn value_card<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let value = f.value.as_ref()?;
    let mono = iced::Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    };
    let line = row![
        signex_widgets::text(look.t(ids::PCB_PRIM_VALUE))
            .size(TS.caption)
            .color(resolve(look.tokens()).ink3),
        signex_widgets::text(UserText::new(value.clone()))
            .size(TS.title)
            .font(mono)
            .color(look.ink)
            .align_x(iced::alignment::Horizontal::Right)
            .wrapping(iced::widget::text::Wrapping::None)
            .width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let card = container(line)
        .padding([SP.m, SP.m + SP.xs])
        .width(Length::Fill)
        .style(super::zone::card_style(look.tokens()));
    Some(linked(
        look,
        &f.common,
        dd::VALUE,
        container(card)
            .padding([SP.xs, PAD_X])
            .width(Length::Fill)
            .boxed(),
    ))
}

fn properties<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mm = || unit(look, ids::PCB_PRIM_UNIT_MM);
    let point = |label, field, x_field, y_field, (x, y): (Option<f64>, Option<f64>)| {
        linked_row(
            look,
            common,
            field,
            label,
            two_fields(
                prefixed(look, common, x_field, x, ids::PCB_PRIM_X),
                prefixed(look, common, y_field, y, ids::PCB_PRIM_Y),
                mm(),
            ),
        )
    };
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES).collapsible(
        "pcb_prim.dimension.properties",
        false,
        None,
    );
    // What a dimension not placed yet measures is its clicks'.
    if !common.target.is_placement() {
        s.push(
            common,
            ids::PCB_PRIM_POINT_START,
            point(
                ids::PCB_PRIM_POINT_START,
                dd::START,
                F::StartX,
                F::StartY,
                f.start,
            ),
        );
        s.push(
            common,
            ids::PCB_PRIM_POINT_END,
            point(ids::PCB_PRIM_POINT_END, dd::END, F::EndX, F::EndY, f.end),
        );
        s.push(
            common,
            ids::PCB_PRIM_OFFSET,
            linked_row(
                look,
                common,
                dd::OFFSET,
                ids::PCB_PRIM_OFFSET,
                with_unit(mm_field(common, F::Offset, f.offset, look), mm()),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_DIM_STYLE,
        caption(look, look.t(ids::PCB_PRIM_DIM_STYLE)),
    );
    for (label, field, value) in [
        (ids::PCB_PRIM_LINE_WIDTH, F::LineWidth, f.line_width),
        (ids::PCB_PRIM_TEXT_HEIGHT, F::TextHeight, f.text_height),
    ] {
        s.push(
            common,
            label,
            linked_row(
                look,
                common,
                dd::STYLE,
                label,
                with_unit(mm_field(common, field, value, look), mm()),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_PRECISION,
        linked_row(
            look,
            common,
            dd::VALUE,
            ids::PCB_PRIM_PRECISION,
            precision(look, f),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        linked(
            look,
            common,
            dd::LAYER,
            any_layer_row(look, common, &f.layers, f.layer.as_ref()),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        caption(look, look.t(ids::PCB_PRIM_DIM_LAYER_NOTE)),
    );
    s.finish()
}

/// Precision as 0 to 6 decimals: a pick is one undo step, so a precision
/// outside 0 to 6 cannot be typed.
fn precision<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Element<'a, PanelMsg> {
    let common = &f.common;
    let cells = (0..=MAX_PRECISION)
        .map(|decimals| Segment {
            label: t!(
                look.ctx.i18n,
                ids::PCB_PRIM_DIM_PRECISION_STEP,
                "decimals" => decimals
            ),
            is_on: f.precision.map(u32::from) == Some(decimals),
            msg: common.is_editable().then(|| PcbPrimitiveMsg::ValuePicked {
                target: common.target.clone(),
                field: F::Precision,
                value: f64::from(decimals),
            }),
        })
        .collect();
    segments(cells)
}

#[cfg(test)]
mod tests {
    use pcb_geom::ItemRef;
    use uuid::Uuid;

    use super::*;
    use crate::layout_probe::lay_out_texts;
    use crate::panels::pcb_primitive_properties::drawing::fixture;

    /// Mockup row 101, decision of 2026-11-21: the Layer row says a
    /// dimension may sit on any layer and is only a drawing on copper.
    #[test]
    fn the_layer_row_carries_the_drawing_only_note() {
        let (app, _boot) = crate::app::Signex::new();
        let ctx = app.document_state.panel_ctx.clone();
        let f = DimensionFacts {
            common: fixture::common(vec![ItemRef::Via(Uuid::from_u128(1))]),
            start: (Some(0.0), Some(0.0)),
            end: (Some(62.0), Some(0.0)),
            offset: Some(-4.0),
            line_width: Some(0.1),
            text_height: Some(1.0),
            precision: Some(2),
            value: Some("62.00 mm".into()),
            layer: None,
            layers: Vec::new(),
            style: Default::default(),
        };
        let section = properties(Look::of(&ctx), &f).expect("the section");
        let note = t!(ctx.i18n, ids::PCB_PRIM_DIM_LAYER_NOTE).into_string();
        let texts = lay_out_texts(section, iced::Size::new(300.0, 1200.0));
        assert!(texts.iter().any(|t| t.text == note), "{texts:?}");
    }
}
