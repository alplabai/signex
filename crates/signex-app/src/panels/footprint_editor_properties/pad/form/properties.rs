//! The pad's Properties section (mockup row 125): Designator with Locked, Layer,
//! Electrical Type (not for a hole), Net, the read-only position, Rotation
//! and the template with its library.

use iced::widget::{Column, Space};
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{CheckboxLabel as _, text_input};

use super::super::super::super::PanelMsg;
use super::super::field::{Shown, labelled_row, number_input, pair, read_only_cell, with_refusals};
use super::{
    Form, PadClass, pad_designator_msg, pad_electrical_type_msg, pad_input_row_w, pad_locked_msg,
    pad_net_msg, pad_pick_row_w, pad_rotation_msg, pad_side_msg, pad_template_library_msg,
    pad_template_msg,
};
use crate::library::editor::footprint::{PadSide, pad_field};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(super) fn push<'a>(
    col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
) -> Column<Element<'a, PanelMsg>> {
    let (mut col, is_open) = form.section(
        col,
        "fp_pad_properties",
        ids::EDITORS_FP_PAD_SECTION_PROPERTIES,
    );
    if !is_open {
        return col;
    }
    let (v, target, i18n) = (form.values, form.target, form.i18n);
    let fw = form.fw();
    let muted = form.palette.muted;

    let designator = text_input(t!(i18n, ids::EDITORS_FP_PAD_AUTO), v.designator.clone())
        .size(TS.caption)
        .width(Length::Fill)
        .padding(SP.xs)
        .style(crate::panels::form_look::text_input_style)
        .on_input(move |s| pad_designator_msg(target, s))
        .boxed();
    let locked = signex_widgets::checkbox(v.locked)
        .labelled(t!(i18n, ids::EDITORS_PAD_LOCKED))
        .on_toggle(move |on| pad_locked_msg(target, on))
        .size(TS.label)
        .text_size(TS.caption)
        .spacing(SP.s)
        .style(crate::panels::form_look::checkbox_style)
        .boxed();
    col = col.push(labelled_row(
        t!(i18n, ids::EDITORS_PAD_DESIGNATOR),
        pair(designator, locked),
        fw,
        muted,
    ));

    // The pad's kind has no picker in this form, so Layer stays the way a
    // pad becomes through-hole (Multi-Layer) and back.
    col = col.push(pad_pick_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_LAYER),
        PadSide::ALL_OPTIONS,
        v.side,
        move |s: PadSide| pad_side_msg(target, s),
        muted,
    ));
    if form.class != PadClass::Npth {
        col = col.push(pad_pick_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_ELECTRICAL_TYPE),
            signex_sketch::attr::ElectricalType::ALL,
            v.electrical_type,
            move |e: signex_sketch::attr::ElectricalType| pad_electrical_type_msg(target, e),
            muted,
        ));
    }
    let net = text_input(t!(i18n, ids::EDITORS_FP_PAD_UNASSIGNED), v.net.clone())
        .size(TS.caption)
        .width(Length::Fill)
        .padding(SP.xs)
        .style(crate::panels::form_look::text_input_style)
        .on_input(move |s| pad_net_msg(target, s))
        .boxed();
    col = col.push(labelled_row(t!(i18n, ids::EDITORS_PAD_NET), net, fw, muted));

    // Position is read-only: a pad moves by dragging it on the canvas.
    if let Some([x, y]) = v.position_mm {
        let cell = |n: f64| read_only_cell(UserText::new(format!("{n:.3}")), form.tokens);
        col = col.push(labelled_row(
            t!(i18n, ids::EDITORS_FP_PAD_POSITION),
            pair(cell(x), cell(y)),
            fw,
            muted,
        ));
    }

    let rotation = Shown::from_buffer(
        form.buffer(pad_field::ROTATION),
        format!("{:.1}", v.rotation_deg),
        false,
        |_| true,
    );
    let input = number_input(&rotation, move |s| pad_rotation_msg(target, s), form.tokens);
    let row = labelled_row(
        t!(i18n, ids::EDITORS_PAD_ROTATION),
        pair(input, Space::new().width(Length::Fill).boxed()),
        fw,
        muted,
    );
    col = col.push(with_refusals(row, &[rotation.refusal], form.tokens, i18n));

    col = col.push(pad_input_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_TEMPLATE),
        "",
        v.template.clone(),
        move |s| pad_template_msg(target, s),
        muted,
    ));
    col.push(pad_input_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_TEMPLATE_LIBRARY),
        "",
        v.template_library.clone(),
        move |s| pad_template_library_msg(target, s),
        muted,
    ))
}
