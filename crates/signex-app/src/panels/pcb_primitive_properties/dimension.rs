//! The Dimension panel (#2373, #2523, mockup row 101): the illustration, the measured
//! Value on a card under it, then where it stands (Start, End, Offset) and
//! its Style: line width, text height, decimals and layer. Lengths and the
//! Value follow the board's Units, mm or mils (Caner, 2026-11-24).

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::mm_per;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::schematic::Point;
use uuid::Uuid;

use super::drawing::dimension as dd;
use super::linked::{
    Segment, illustration, linked, linked_row, prefixed, segments, two_fields, unit, with_unit,
};
use super::rows::{Look, PAD_X, Section, any_layer_row, mm_field, stack};
use super::stack_rows::caption;
use super::{DimensionFacts, PcbPrimitiveMsg, PrimitiveField as F, SP, TS, parse_number};
use crate::panels::pcb_board_properties::shown_unit;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::resolve;

/// The most decimals a dimension's value shows.
const MAX_PRECISION: u32 = 6;

/// The dimension fields that hold a length, shown in the board's Units.
pub(crate) fn is_dimension_length(field: F) -> bool {
    matches!(
        field,
        F::StartX | F::StartY | F::EndX | F::EndY | F::Offset | F::LineWidth | F::TextHeight
    )
}

/// The millimetres a length typed into a dimension field stands for while
/// the board shows `unit`: a bare number is in `unit`, a number ending in
/// "mm" or "mil" is in that unit. `None` when it is not a finite number.
pub(crate) fn typed_length_mm(text: &str, unit: Unit) -> Option<f64> {
    let trimmed = text.trim();
    if let Some(mils) = trimmed
        .strip_suffix("mils")
        .or_else(|| trimmed.strip_suffix("mil"))
    {
        return parse_number(mils).map(|v| v * mm_per(Unit::Mil));
    }
    if trimmed.ends_with("mm") {
        return parse_number(trimmed);
    }
    parse_number(trimmed).map(|v| v * mm_per(shown_unit(unit)))
}

/// The unit the panel shows lengths in, and its label.
fn length_unit(look: Look<'_>) -> (Unit, MessageId) {
    match shown_unit(look.ctx.unit) {
        Unit::Mil => (Unit::Mil, ids::PCB_PRIM_UNIT_MIL),
        _ => (Unit::Mm, ids::PCB_PRIM_UNIT_MM),
    }
}

/// The Value the card shows: as drawn (in mm) while the board shows mm; in
/// mils the measured length to the same decimals. A selection whose points
/// or decimals differ keeps the drawn text. `None` before it is placed.
fn headline(look: Look<'_>, f: &DimensionFacts) -> Option<String> {
    let drawn = f.value.clone()?;
    let in_mils = || {
        let ((Some(x0), Some(y0)), (Some(x1), Some(y1))) = (f.start, f.end) else {
            return None;
        };
        let measured =
            LinearDimension::new(Uuid::nil(), Point::new(x0, y0), Point::new(x1, y1), 0.0);
        let mils = measured.value_mm() / mm_per(Unit::Mil);
        let number = format!("{:.*}", usize::from(f.precision?), mils);
        let line = t!(look.ctx.i18n, ids::PCB_PRIM_DIM_VALUE_MIL, "value" => number);
        Some(line.into_string())
    };
    match length_unit(look).0 {
        Unit::Mil => Some(in_mils().unwrap_or(drawn)),
        _ => Some(drawn),
    }
}

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a DimensionFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let drawing = illustration(
        look,
        &f.common,
        dd::illustration(&drawn(look, f), &ctx.i18n),
    );
    stack(vec![
        Some(drawing),
        value_card(look, f),
        properties(look, f),
    ])
}

/// `f` as the illustration shows it: the Value of the card and the text
/// height in the board's Units, so its callouts match the fields under it.
/// The points stay in mm; the drawing is fitted to its frame.
fn drawn(look: Look<'_>, f: &DimensionFacts) -> DimensionFacts {
    let shown = length_unit(look).0;
    DimensionFacts {
        value: headline(look, f),
        text_height: f.text_height.map(|v| v / mm_per(shown)),
        ..f.clone()
    }
}

/// The measured value as a headline card; none for a dimension not placed
/// yet.
fn value_card<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let value = headline(look, f)?;
    let mono = iced::Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    };
    let line = row![
        signex_widgets::text(look.t(ids::PCB_PRIM_VALUE))
            .size(TS.caption)
            .color(resolve(look.tokens()).ink3),
        signex_widgets::text(UserText::new(value))
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
    let (shown, unit_id) = length_unit(look);
    let unit_label = || unit(look, unit_id);
    let len = |value_mm: Option<f64>| value_mm.map(|v| v / mm_per(shown));
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES).collapsible(
        "pcb_prim.dimension.properties",
        false,
        None,
    );
    // What a dimension not placed yet measures is its clicks'.
    if !common.target.is_placement() {
        push_placement_rows(&mut s, look, f);
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
                with_unit(mm_field(common, field, len(value), look), unit_label()),
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

/// Where a placed dimension stands: Start, End and Offset, in the board's
/// Units.
fn push_placement_rows<'a>(s: &mut Section<'a>, look: Look<'a>, f: &'a DimensionFacts) {
    let common = &f.common;
    let (shown, unit_id) = length_unit(look);
    let unit_label = || unit(look, unit_id);
    let len = |value_mm: Option<f64>| value_mm.map(|v| v / mm_per(shown));
    let point = |label, field, x_field, y_field, (x, y): (Option<f64>, Option<f64>)| {
        linked_row(
            look,
            common,
            field,
            label,
            two_fields(
                prefixed(look, common, x_field, len(x), ids::PCB_PRIM_X),
                prefixed(look, common, y_field, len(y), ids::PCB_PRIM_Y),
                unit_label(),
            ),
        )
    };
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
            with_unit(
                mm_field(common, F::Offset, len(f.offset), look),
                unit_label(),
            ),
        ),
    );
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
        };
        let section = properties(Look::of(&ctx), &f).expect("the section");
        let note = t!(ctx.i18n, ids::PCB_PRIM_DIM_LAYER_NOTE).into_string();
        let texts = lay_out_texts(section, iced::Size::new(300.0, 1200.0));
        assert!(texts.iter().any(|t| t.text == note), "{texts:?}");
    }

    /// A 62 mm dimension drawn "62.00mm", Offset -4 mm, line 0.1 mm.
    fn sample() -> DimensionFacts {
        DimensionFacts {
            common: fixture::common(vec![ItemRef::Dimension(Uuid::from_u128(1))]),
            start: (Some(0.0), Some(0.0)),
            end: (Some(62.0), Some(0.0)),
            offset: Some(-4.0),
            line_width: Some(0.1),
            text_height: Some(1.0),
            precision: Some(2),
            value: Some("62.00mm".into()),
            layer: None,
            layers: Vec::new(),
        }
    }

    /// The panel context of a fresh app with the board showing `unit`.
    fn ctx_in(unit: Unit) -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.unit = unit;
        ctx
    }

    /// What the Properties section shows: its texts and its field values.
    fn shown(ctx: &PanelContext, f: &DimensionFacts) -> (Vec<String>, Vec<String>) {
        let window = iced::Size::new(300.0, 1200.0);
        let texts = lay_out_texts(properties(Look::of(ctx), f).expect("a section"), window);
        let inputs = crate::layout_probe::lay_out_inputs(
            properties(Look::of(ctx), f).expect("a section"),
            window,
        );
        (
            texts.into_iter().map(|t| t.text).collect(),
            inputs.into_iter().map(|t| t.text).collect(),
        )
    }

    /// The Value card's texts.
    fn headline_texts(ctx: &PanelContext, f: &DimensionFacts) -> Vec<String> {
        let card = value_card(Look::of(ctx), f).expect("a placed dimension");
        lay_out_texts(card, iced::Size::new(300.0, 200.0))
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    /// Mockup row 101 (Caner, 2026-11-24): while the board shows mm the
    /// fields are in mm and the Value is the drawn text.
    #[test]
    fn in_mm_the_fields_and_value_are_in_mm() {
        let ctx = ctx_in(Unit::Mm);
        let f = sample();
        let (texts, inputs) = shown(&ctx, &f);
        let mm = t!(ctx.i18n, ids::PCB_PRIM_UNIT_MM).into_string();
        let mil = t!(ctx.i18n, ids::PCB_PRIM_UNIT_MIL).into_string();
        assert!(texts.contains(&mm) && !texts.contains(&mil), "{texts:?}");
        for value in ["62", "-4", "0.1", "1"] {
            assert!(inputs.iter().any(|i| i == value), "{value} in {inputs:?}");
        }
        assert!(headline_texts(&ctx, &f).contains(&"62.00mm".to_string()));
    }

    /// Mockup row 101 (Caner, 2026-11-24): while the board shows mils every
    /// length field and the Value are in mils, the Value to the dimension's
    /// decimals.
    #[test]
    fn in_mils_the_fields_and_value_are_in_mils() {
        let ctx = ctx_in(Unit::Mil);
        let f = sample();
        let (texts, inputs) = shown(&ctx, &f);
        let mm = t!(ctx.i18n, ids::PCB_PRIM_UNIT_MM).into_string();
        let mil = t!(ctx.i18n, ids::PCB_PRIM_UNIT_MIL).into_string();
        assert!(texts.contains(&mil) && !texts.contains(&mm), "{texts:?}");
        // 62 mm, -4 mm, 0.1 mm and 1 mm in mils, to four decimals.
        for value in ["2440.9449", "-157.4803", "3.937", "39.3701"] {
            assert!(inputs.iter().any(|i| i == value), "{value} in {inputs:?}");
        }
        let value = t!(ctx.i18n, ids::PCB_PRIM_DIM_VALUE_MIL, "value" => "2440.94").into_string();
        assert!(headline_texts(&ctx, &f).contains(&value));
    }

    /// Mockup row 101: the illustration's value and text height callouts
    /// follow the board's Units with the card and the fields.
    #[test]
    fn the_illustration_follows_the_board_units() {
        let f = sample();
        let mm = drawn(Look::of(&ctx_in(Unit::Mm)), &f);
        assert_eq!(mm.value.as_deref(), Some("62.00mm"));
        assert_eq!(mm.text_height, Some(1.0));
        let ctx = ctx_in(Unit::Mil);
        let mils = drawn(Look::of(&ctx), &f);
        let value = t!(ctx.i18n, ids::PCB_PRIM_DIM_VALUE_MIL, "value" => "2440.94").into_string();
        assert_eq!(mils.value, Some(value));
        let height = mils.text_height.expect("a text height");
        assert!((height - 1.0 / 0.0254).abs() < 1e-9, "{height}");
        assert_eq!(
            (mils.start, mils.end),
            (f.start, f.end),
            "points stay in mm"
        );
    }

    /// A typed length is in the board's Units unless it names its unit.
    #[test]
    fn a_typed_length_is_read_in_the_board_units() {
        let near = |got: Option<f64>, want: f64| got.is_some_and(|v| (v - want).abs() < 1e-9);
        assert!(near(typed_length_mm("100", Unit::Mil), 2.54));
        assert!(near(typed_length_mm(" 100 mil ", Unit::Mil), 2.54));
        assert!(near(typed_length_mm("100mils", Unit::Mm), 2.54));
        assert!(near(typed_length_mm("2.5mm", Unit::Mil), 2.5));
        assert!(near(typed_length_mm("2.5", Unit::Mm), 2.5));
        assert_eq!(typed_length_mm("wide", Unit::Mil), None);
        assert!(is_dimension_length(F::LineWidth));
        assert!(!is_dimension_length(F::Precision));
    }
}
