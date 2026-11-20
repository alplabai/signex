//! The copper section of the pad form (#125). An SMD pad has one layer and
//! no pad stack (R12): "Pad · Top Layer" with its shape, size, corner
//! radius and copper offset. A through-hole pad gets "Pad Stack" with the
//! Simple / Top-Middle-Bottom / Full Stack mode; the per-layer modes show
//! the copper table. A non-plated hole has no copper, so no section.

use iced::widget::{Column, Space, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb_layer::CopperLayer;
use signex_widgets::{Label, raw, segmented};

use super::super::super::super::{PadShapeParamSummary, PanelMsg};
use super::super::choices::PadShapeChoice;
use super::super::field::{Shown, labelled_row, number_input, pair, segment_style, with_refusals};
use super::super::illustration::{CORNER, SHAPE, SIZE_X, SIZE_Y};
use super::super::table::{COPPER_CELLS, pad_copper_row, pad_size_buf, pad_table_header};
use super::{
    Form, PadClass, PadEditTarget, pad_copper_offset_x_msg, pad_copper_offset_y_msg,
    pad_corner_radius_msg, pad_input_row_w, pad_pick_row_w, pad_shape_msg, pad_size_x_msg,
    pad_size_y_msg, pad_trapezoid_delta_x_msg, pad_trapezoid_delta_y_msg,
};
use crate::library::editor::footprint::{PadSide, PadStackTab, pad_field};
use crate::pcb_canvas::layer_style::copper_name;
use crate::styles::Spacing;

const SP: Spacing = Spacing::DEFAULT;

pub(super) fn push<'a>(
    col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
    shape_params: &'a [PadShapeParamSummary],
) -> Column<Element<'a, PanelMsg>> {
    let title = match form.class {
        PadClass::Npth => return col,
        PadClass::Smd(PadSide::Bottom) => ids::EDITORS_FP_PAD_SECTION_PAD_BOTTOM,
        PadClass::Smd(_) => ids::EDITORS_FP_PAD_SECTION_PAD_TOP,
        PadClass::ThroughHole | PadClass::Slot => ids::EDITORS_FP_PAD_SECTION_PAD_STACK,
    };
    let (mut col, is_open) = form.section(col, "fp_pad_stack", title);
    if !is_open {
        return col;
    }
    let (v, target) = (form.values, form.target);
    let (fw, muted) = (form.fw(), form.palette.muted);
    let current_shape = PadShapeChoice::from_lib(&v.shape);
    if form.class.has_copper_stack() {
        col = col.push(mode_segment(form));
    }
    col = if form.class.has_copper_stack() && v.pad_stack_tab != PadStackTab::Simple {
        push_copper_table(col, form, current_shape)
    } else {
        push_shape_and_size(col, form, current_shape)
    };

    if matches!(current_shape, PadShapeChoice::RoundedRectangle) {
        let corner = Shown::from_buffer(
            form.buffer(pad_field::CORNER_RADIUS),
            v.stack
                .corner_radius_pct
                .map(|pct| format!("{pct:.0}"))
                .unwrap_or_default(),
            false,
            |_| true,
        );
        let input = number_input(
            &corner,
            move |s| pad_corner_radius_msg(target, s),
            form.tokens,
        );
        let row = labelled_row(
            t!(form.i18n, ids::EDITORS_PAD_CORNER_RADIUS),
            pair(input, Space::new().width(Length::Fill).boxed()),
            fw,
            muted,
        );
        let row = with_refusals(row, &[corner.refusal], form.tokens, form.i18n);
        col = col.push(form.linked(row, CORNER));
    }
    // A trapezoid's delta, a shape modifier like the corner radius (#2242).
    if let signex_library::PadShape::Trapezoid { delta } = v.shape {
        col = col.push(pad_input_row_w(
            fw,
            t!(form.i18n, ids::EDITORS_PAD_TRAPEZOID_DELTA_X),
            "0",
            pad_size_buf(delta[0]),
            move |s| pad_trapezoid_delta_x_msg(target, s),
            muted,
        ));
        col = col.push(pad_input_row_w(
            fw,
            t!(form.i18n, ids::EDITORS_PAD_TRAPEZOID_DELTA_Y),
            "0",
            pad_size_buf(delta[1]),
            move |s| pad_trapezoid_delta_y_msg(target, s),
            muted,
        ));
    }
    // The sketch parameters the selected pad's shape is bound to, edited
    // here without entering Sketch mode (none while placing).
    if let PadEditTarget::Selected(pad_idx) = target {
        for entry in shape_params {
            let key = entry.key.clone();
            col = col.push(pad_input_row_w(
                fw,
                raw(entry.label.as_str()),
                "0.25mm",
                entry.current_expr.clone(),
                move |value| PanelMsg::FpEditorEditPadShapeParam {
                    pad_idx,
                    key: key.clone(),
                    value,
                },
                muted,
            ));
        }
    }

    let mm = |field: &str, v: Option<f64>| {
        let stored = v.map(|v| format!("{v:.3}")).unwrap_or_default();
        Shown::from_buffer(form.buffer(field), stored, false, |_| true)
    };
    let (x, y) = (
        mm(pad_field::COPPER_OFFSET_X, v.copper_offset_x_mm),
        mm(pad_field::COPPER_OFFSET_Y, v.copper_offset_y_mm),
    );
    let offset = pair(
        number_input(&x, move |s| pad_copper_offset_x_msg(target, s), form.tokens),
        number_input(&y, move |s| pad_copper_offset_y_msg(target, s), form.tokens),
    );
    let row = labelled_row(
        t!(form.i18n, ids::EDITORS_FP_PAD_COPPER_OFFSET),
        offset,
        fw,
        muted,
    );
    col.push(with_refusals(
        row,
        &[x.refusal, y.refusal],
        form.tokens,
        form.i18n,
    ))
}

/// The Simple / Top-Middle-Bottom / Full Stack mode of a through-hole pad.
fn mode_segment<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let options = PadStackTab::ALL
        .iter()
        .map(|tab| (*tab, Label::from(t!(form.i18n, mode_label(*tab)))));
    container(segmented(
        options,
        form.values.pad_stack_tab,
        segment_style(form.tokens).fill_width(true),
        PanelMsg::FpEditorSetPadStackTab,
    ))
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

fn mode_label(tab: PadStackTab) -> MessageId {
    match tab {
        PadStackTab::Simple => ids::EDITORS_FP_PAD_STACK_SIMPLE,
        PadStackTab::TopMiddleBottom => ids::EDITORS_FP_PAD_STACK_TOP_MIDDLE_BOTTOM,
        PadStackTab::FullStack => ids::EDITORS_FP_PAD_STACK_FULL,
    }
}

/// One shape and one size for every layer: an SMD pad, or the Simple mode.
fn push_shape_and_size<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
    current: PadShapeChoice,
) -> Column<Element<'a, PanelMsg>> {
    let (v, target) = (form.values, form.target);
    let (fw, muted) = (form.fw(), form.palette.muted);
    let shape = pad_pick_row_w(
        fw,
        t!(form.i18n, ids::EDITORS_PAD_SHAPE),
        PadShapeChoice::ALL,
        current,
        move |c: PadShapeChoice| pad_shape_msg(target, c.to_lib()),
        muted,
    );
    col = col.push(form.linked(shape, SHAPE));
    // A refused size above zero can only be one the trapezoid rejects.
    let x = Shown::from_buffer(
        form.buffer(pad_field::SIZE_X),
        pad_size_buf(v.size_x_mm),
        true,
        |_| false,
    );
    let y = Shown::from_buffer(
        form.buffer(pad_field::SIZE_Y),
        pad_size_buf(v.size_y_mm),
        true,
        |_| false,
    );
    let sizes = pair(
        form.linked(
            number_input(&x, move |s| pad_size_x_msg(target, s), form.tokens),
            SIZE_X,
        ),
        form.linked(
            number_input(&y, move |s| pad_size_y_msg(target, s), form.tokens),
            SIZE_Y,
        ),
    );
    let row = labelled_row(t!(form.i18n, ids::EDITORS_FP_PAD_SIZE), sizes, fw, muted);
    col.push(with_refusals(
        row,
        &[x.refusal, y.refusal],
        form.tokens,
        form.i18n,
    ))
}

/// The per-layer copper table: Top, the inner layers and Bottom. Only the
/// Top row edits; the others mirror it until the model stores per-layer
/// copper. Full Stack names the inner layers by position (L2…Ln−1).
fn push_copper_table<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
    current: PadShapeChoice,
) -> Column<Element<'a, PanelMsg>> {
    let palette = form.palette;
    col = col.push(pad_table_header(
        &["COPPER", "X-Size", "Y-Size", "Shape", "Relief"],
        &COPPER_CELLS,
        palette.muted,
        palette.border,
    ));
    let inner = if form.values.pad_stack_tab == PadStackTab::FullStack {
        ids::EDITORS_FP_PAD_LAYER_INNER_RANGE
    } else {
        ids::EDITORS_FP_PAD_LAYER_INNER
    };
    let rows = [
        copper_name(form.i18n, CopperLayer::Top).into_string(),
        t!(form.i18n, inner).into_string(),
        copper_name(form.i18n, CopperLayer::Bottom).into_string(),
    ];
    for (idx, label) in rows.into_iter().enumerate() {
        col = col.push(pad_copper_row(
            label,
            form.values,
            current,
            form.target,
            palette,
            idx == 0,
        ));
    }
    col
}
