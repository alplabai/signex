//! The look sections of the Dimension panel (#2868): Arrows, Text,
//! Extension Lines, and Units and Format. Each pick or Enter is one undo
//! step; while the Dimension tool is armed they set what it places next.

use iced::widget::container;
use iced::{Element, Length, Widget as _};
use signex_engine::pcb::primitive_edit::DimensionStyleEdit as E;
use signex_i18n::{MessageId, ids};
use signex_types::pcb_dimension_style::{
    ArrowStyle, BoardFont, DimensionTextPosition, DimensionUnits,
};

use super::drawing::dimension as dd;
use super::linked::{Segment, linked_row, prefixed, segments, two_fields, unit, with_unit};
use super::rows::{Look, PAD_X, Section, field, live_checkbox, mm_field};
use super::{DimensionFacts, PcbPrimitiveMsg, PrimitiveField as F, SP};
use crate::panels::PanelMsg;

/// The four look sections, each `None` when the search leaves none of its
/// rows.
pub(super) fn sections<'a>(
    look: Look<'a>,
    f: &'a DimensionFacts,
) -> Vec<Option<Element<'a, PanelMsg>>> {
    vec![
        arrows(look, f),
        text(look, f),
        extensions(look, f),
        units(look, f),
    ]
}

/// A segmented row of `choices`, the one `shown` lit; a press sends `pick`.
fn picks<'a, T: Copy + PartialEq>(
    look: Look<'a>,
    f: &'a DimensionFacts,
    choices: &[(T, MessageId)],
    shown: Option<T>,
    pick: fn(T) -> E,
) -> Element<'a, PanelMsg> {
    let common = &f.common;
    let cells = choices
        .iter()
        .map(|&(value, label)| Segment {
            label: look.t(label),
            is_on: shown == Some(value),
            msg: common
                .is_editable()
                .then(|| PcbPrimitiveMsg::DimensionStylePicked {
                    target: common.target.clone(),
                    edit: pick(value),
                }),
        })
        .collect();
    segments(cells)
}

/// A length row in mm.
fn length<'a>(
    look: Look<'a>,
    f: &'a DimensionFacts,
    label: MessageId,
    id: F,
    value: Option<f64>,
) -> Element<'a, PanelMsg> {
    let mm = unit(look, ids::PCB_PRIM_UNIT_MM);
    linked_row(
        look,
        &f.common,
        dd::STYLE,
        label,
        with_unit(mm_field(&f.common, id, value, look), mm),
    )
}

/// A checkbox row; a tick or clear sends `pick`.
fn check<'a>(
    look: Look<'a>,
    f: &'a DimensionFacts,
    label: MessageId,
    checked: Option<bool>,
    pick: fn(bool) -> E,
) -> Element<'a, PanelMsg> {
    let target = f.common.target.clone();
    container(live_checkbox(look, &f.common, label, checked, move |on| {
        PcbPrimitiveMsg::DimensionStylePicked {
            target: target.clone(),
            edit: pick(on),
        }
    }))
    .padding([SP.xs, PAD_X])
    .width(Length::Fill)
    .boxed()
}

fn arrows<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let (common, st) = (&f.common, &f.style);
    let mut s = Section::new(look, ids::PCB_PRIM_DIM_ARROWS).collapsible(
        "pcb_prim.dimension.arrows",
        false,
        None,
    );
    let styles = [
        (ArrowStyle::Filled, ids::PCB_PRIM_DIM_ARROW_FILLED),
        (ArrowStyle::Open, ids::PCB_PRIM_DIM_ARROW_OPEN),
        (ArrowStyle::None, ids::PCB_PRIM_DIM_ARROW_NONE),
    ];
    let style = picks(look, f, &styles, st.arrow_style, E::ArrowStyle);
    s.push(
        common,
        ids::PCB_PRIM_DIM_ARROW_STYLE,
        linked_row(
            look,
            common,
            dd::STYLE,
            ids::PCB_PRIM_DIM_ARROW_STYLE,
            style,
        ),
    );
    for (label, id, value) in [
        (
            ids::PCB_PRIM_DIM_ARROW_LENGTH,
            F::ArrowLength,
            st.arrow_length,
        ),
        (ids::PCB_PRIM_DIM_ARROW_WIDTH, F::ArrowWidth, st.arrow_width),
    ] {
        s.push(common, label, length(look, f, label, id, value));
    }
    s.finish()
}

fn text<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let (common, st) = (&f.common, &f.style);
    let mut s = Section::new(look, ids::PCB_PRIM_DIM_TEXT).collapsible(
        "pcb_prim.dimension.text",
        false,
        None,
    );
    let fonts = [
        (BoardFont::Mono, ids::PCB_PRIM_DIM_FONT_MONO),
        (BoardFont::Sans, ids::PCB_PRIM_DIM_FONT_SANS),
    ];
    let font = picks(look, f, &fonts, st.font, E::Font);
    s.push(
        common,
        ids::PCB_PRIM_DIM_FONT,
        linked_row(look, common, dd::VALUE, ids::PCB_PRIM_DIM_FONT, font),
    );
    s.push(
        common,
        ids::PCB_PRIM_DIM_BOLD,
        check(look, f, ids::PCB_PRIM_DIM_BOLD, st.bold, E::Bold),
    );
    s.push(
        common,
        ids::PCB_PRIM_DIM_ITALIC,
        check(look, f, ids::PCB_PRIM_DIM_ITALIC, st.italic, E::Italic),
    );
    s.push(
        common,
        ids::PCB_PRIM_DIM_TEXT_WIDTH,
        length(
            look,
            f,
            ids::PCB_PRIM_DIM_TEXT_WIDTH,
            F::TextWidth,
            st.text_width,
        ),
    );
    let places = [
        (DimensionTextPosition::Above, ids::PCB_PRIM_DIM_TEXT_ABOVE),
        (DimensionTextPosition::Inline, ids::PCB_PRIM_DIM_TEXT_INLINE),
        (DimensionTextPosition::Manual, ids::PCB_PRIM_DIM_TEXT_MANUAL),
    ];
    let place = picks(look, f, &places, st.text_position, E::TextPosition);
    s.push(
        common,
        ids::PCB_PRIM_DIM_TEXT_POSITION,
        linked_row(
            look,
            common,
            dd::VALUE,
            ids::PCB_PRIM_DIM_TEXT_POSITION,
            place,
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_DIM_TEXT_GAP,
        length(look, f, ids::PCB_PRIM_DIM_TEXT_GAP, F::TextGap, st.text_gap),
    );
    if st.text_position == Some(DimensionTextPosition::Manual) {
        let (along, across) = st.text_at;
        let at = two_fields(
            prefixed(look, common, F::TextAlong, along, ids::PCB_PRIM_DIM_ALONG),
            prefixed(
                look,
                common,
                F::TextAcross,
                across,
                ids::PCB_PRIM_DIM_ACROSS,
            ),
            unit(look, ids::PCB_PRIM_UNIT_MM),
        );
        s.push(
            common,
            ids::PCB_PRIM_DIM_TEXT_AT,
            linked_row(look, common, dd::VALUE, ids::PCB_PRIM_DIM_TEXT_AT, at),
        );
    }
    s.finish()
}

fn extensions<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let (common, st) = (&f.common, &f.style);
    let mut s = Section::new(look, ids::PCB_PRIM_DIM_EXTENSIONS).collapsible(
        "pcb_prim.dimension.extensions",
        false,
        None,
    );
    for (label, id, value) in [
        (
            ids::PCB_PRIM_DIM_EXT_OFFSET,
            F::ExtensionOffset,
            st.extension_offset,
        ),
        (
            ids::PCB_PRIM_DIM_EXT_OVERSHOOT,
            F::ExtensionOvershoot,
            st.extension_overshoot,
        ),
        (
            ids::PCB_PRIM_DIM_EXT_WIDTH,
            F::ExtensionWidth,
            st.extension_width,
        ),
    ] {
        s.push(common, label, length(look, f, label, id, value));
    }
    s.finish()
}

fn units<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let (common, st) = (&f.common, &f.style);
    let mut s = Section::new(look, ids::PCB_PRIM_DIM_UNITS).collapsible(
        "pcb_prim.dimension.units",
        false,
        None,
    );
    let units = [
        (DimensionUnits::Mm, ids::PCB_PRIM_DIM_UNIT_MM),
        (DimensionUnits::Mil, ids::PCB_PRIM_DIM_UNIT_MIL),
        (DimensionUnits::In, ids::PCB_PRIM_DIM_UNIT_IN),
    ];
    let unit_row = picks(look, f, &units, st.unit, E::Unit);
    s.push(
        common,
        ids::PCB_PRIM_DIM_UNIT,
        linked_row(look, common, dd::VALUE, ids::PCB_PRIM_DIM_UNIT, unit_row),
    );
    s.push(
        common,
        ids::PCB_PRIM_DIM_SHOW_UNIT,
        check(
            look,
            f,
            ids::PCB_PRIM_DIM_SHOW_UNIT,
            st.show_unit,
            E::ShowUnit,
        ),
    );
    for (label, id, value) in [
        (ids::PCB_PRIM_DIM_PREFIX, F::Prefix, &st.prefix),
        (ids::PCB_PRIM_DIM_SUFFIX, F::Suffix, &st.suffix),
    ] {
        let input = field(common, id, value.clone(), None, look);
        s.push(
            common,
            label,
            linked_row(look, common, dd::VALUE, label, input),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_DIM_TRAILING_ZEROS,
        check(
            look,
            f,
            ids::PCB_PRIM_DIM_TRAILING_ZEROS,
            st.trailing_zeros,
            E::TrailingZeros,
        ),
    );
    s.finish()
}
