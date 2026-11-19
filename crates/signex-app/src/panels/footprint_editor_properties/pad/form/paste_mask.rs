//! Paste & Solder (#125). Each paste and solder mask layer is an enable
//! header over its Rule | Manual expansion. An SMD pad shows its own
//! layer's paste and mask; a through-hole pad or a hole shows both mask
//! layers and no paste (the stored paste values are kept). The mask header
//! is "Solder mask on" = an opening, so it writes the tented flag inverted.

use iced::Element;
use iced::widget::Column;
use signex_i18n::{MessageId, ids, t};
use signex_widgets::enable_section::EnableSection;
use signex_widgets::property_illustration::FieldId;

use super::super::super::super::PanelMsg;
use super::super::field::caption_line;
use super::super::illustration::{MASK, MASK_BOTTOM, PASTE};
use super::super::rule_or_manual_row;
use super::hole::with_header_caption;
use super::{
    Form, PadClass, PadEditTarget, pad_mask_bottom_msg, pad_mask_tented_bottom_msg,
    pad_mask_tented_top_msg, pad_mask_top_msg, pad_paste_bottom_msg, pad_paste_enabled_bottom_msg,
    pad_paste_enabled_top_msg, pad_paste_top_msg,
};
use crate::library::editor::footprint::state::{PadSide, pad_field};
use crate::panels::enable_section::enable_section_style;
use crate::styles::tokens::resolve;

/// One board side's paste or mask layer, as the section draws it.
struct Layer {
    title: MessageId,
    field: FieldId,
    buffer_field: &'static str,
    value_mm: Option<f64>,
    is_on: bool,
    on_toggle: fn(PadEditTarget, bool) -> PanelMsg,
    on_input: fn(PadEditTarget, String) -> PanelMsg,
}

pub(super) fn push<'a>(
    col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
) -> Column<Element<'a, PanelMsg>> {
    let (mut col, is_open) = form.section(
        col,
        "fp_pad_paste_mask",
        ids::EDITORS_FP_PAD_SECTION_PASTE_SOLDER,
    );
    if !is_open {
        return col;
    }
    match form.class {
        PadClass::Smd(side) => {
            let bottom = side == PadSide::Bottom;
            col = col.push(paste(form, bottom));
            col.push(mask(form, bottom))
        }
        PadClass::ThroughHole | PadClass::Slot | PadClass::Npth => {
            col = col.push(mask(form, false));
            col = col.push(mask(form, true));
            let ink = resolve(form.tokens).ink4;
            col.push(caption_line(
                t!(form.i18n, ids::EDITORS_FP_PAD_NO_THROUGH_HOLE_PASTE),
                ink,
            ))
        }
    }
}

fn paste<'a>(form: &Form<'_>, bottom: bool) -> Element<'a, PanelMsg> {
    let stack = &form.values.stack;
    let layer = if bottom {
        Layer {
            title: ids::EDITORS_FP_PAD_BOTTOM_PASTE,
            field: PASTE,
            buffer_field: pad_field::PASTE_BOTTOM,
            value_mm: stack.paste_margin_bottom,
            is_on: stack.paste_enabled_bottom,
            on_toggle: pad_paste_enabled_bottom_msg,
            on_input: pad_paste_bottom_msg,
        }
    } else {
        Layer {
            title: ids::EDITORS_FP_PAD_TOP_PASTE,
            field: PASTE,
            buffer_field: pad_field::PASTE_TOP,
            value_mm: stack.paste_margin_top,
            is_on: stack.paste_enabled_top,
            on_toggle: pad_paste_enabled_top_msg,
            on_input: pad_paste_top_msg,
        }
    };
    let caption = (!layer.is_on).then(|| t!(form.i18n, ids::EDITORS_FP_PAD_PASTE_OFF));
    let target = form.target;
    let on_toggle = layer.on_toggle;
    enable_layer(form, layer, move |on| on_toggle(target, on), caption)
}

/// A solder mask layer. Its header is on while the mask opens over the pad,
/// so a tick clears the tented flag and a clear sets it.
fn mask<'a>(form: &Form<'_>, bottom: bool) -> Element<'a, PanelMsg> {
    let stack = &form.values.stack;
    let layer = if bottom {
        Layer {
            title: ids::EDITORS_FP_PAD_BOTTOM_SOLDER_MASK,
            field: if form.class.has_hole() {
                MASK_BOTTOM
            } else {
                MASK
            },
            buffer_field: pad_field::MASK_BOTTOM,
            value_mm: stack.mask_margin_bottom,
            is_on: !stack.mask_tented_bottom,
            on_toggle: pad_mask_tented_bottom_msg,
            on_input: pad_mask_bottom_msg,
        }
    } else {
        Layer {
            title: ids::EDITORS_FP_PAD_TOP_SOLDER_MASK,
            field: MASK,
            buffer_field: pad_field::MASK_TOP,
            value_mm: stack.mask_margin_top,
            is_on: !stack.mask_tented_top,
            on_toggle: pad_mask_tented_top_msg,
            on_input: pad_mask_top_msg,
        }
    };
    let caption = t!(
        form.i18n,
        if layer.is_on {
            ids::EDITORS_FP_PAD_MASK_OPENING
        } else {
            ids::EDITORS_FP_PAD_MASK_TENTED
        }
    );
    let target = form.target;
    let tented = layer.on_toggle;
    enable_layer(
        form,
        layer,
        move |open| tented(target, !open),
        Some(caption),
    )
}

/// `layer` as an enable header over its Rule | Manual expansion.
fn enable_layer<'a>(
    form: &Form<'_>,
    layer: Layer,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
    caption: Option<signex_i18n::Localized>,
) -> Element<'a, PanelMsg> {
    let target = form.target;
    let on_input = layer.on_input;
    let row = rule_or_manual_row(
        t!(form.i18n, ids::EDITORS_FP_PAD_EXPANSION),
        layer.field,
        layer.value_mm,
        form.buffer(layer.buffer_field).cloned(),
        move |text| on_input(target, text),
        form.palette,
        form.tokens,
        form.hover,
        form.i18n,
    );
    let section = EnableSection::new(t!(form.i18n, layer.title), layer.is_on)
        .on_toggle(on_toggle)
        .view(enable_section_style(form.tokens), row);
    with_header_caption(section, caption, form.tokens)
}
