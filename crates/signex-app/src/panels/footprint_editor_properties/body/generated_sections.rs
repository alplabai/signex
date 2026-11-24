//! The generated features under the courtyard (mockup row 124): Silkscreen
//! outline with its line width, pad clearance and pin 1 marker, and Assembly
//! outline with the body size. A switch that is off greys its fields.

use iced::widget::{Column, container, row, stack};
use iced::{Element, Length, Padding, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::enable_section::EnableSection;
use signex_widgets::{Label, text};

use super::super::pad::{Shown, number_input, with_refusals};
use super::super::rows::control_row;
use super::Ctx;
use crate::library::editor::footprint::{PAD_CLEARANCE_BUFFER, SILK_LINE_WIDTH_BUFFER};
use crate::panels::enable_section::enable_section_style;
use crate::panels::{FootprintFeatureEdit as Edit, PanelMsg, form_look};
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn msg(edit: Edit) -> PanelMsg {
    PanelMsg::FpEditorGeneratedFeature(edit)
}

pub(super) fn view<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    col.push(silk_section(cx)).push(assembly_section(cx))
}

/// The typed text while it is refused or spells the stored value; text an
/// undo has left behind gives way to the stored value.
pub(super) fn live_text(
    buffer: Option<&String>,
    stored: f64,
    fits: fn(f64) -> bool,
) -> Option<&String> {
    buffer.filter(|text| match crate::numeric_field::parse_field_f64(text) {
        Some(value) if fits(value) => value == stored,
        _ => true,
    })
}

fn silk_section<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let settings = &fp.generated;
    let muted = cx.palette.muted;
    let width_text = live_text(
        fp.numeric_buffers.get(SILK_LINE_WIDTH_BUFFER),
        settings.silk_line_width_mm,
        |v| v > 0.0,
    );
    let width = Shown::from_buffer(
        width_text,
        settings.silk_line_width_mm.to_string(),
        true,
        |_| true,
    );
    let clearance_text = live_text(
        fp.numeric_buffers.get(PAD_CLEARANCE_BUFFER),
        settings.pad_clearance_mm,
        |v| v >= 0.0,
    );
    let clearance = Shown::non_negative(clearance_text, settings.pad_clearance_mm.to_string());
    let width_row = control_row(
        &t!(i18n, ids::EDITORS_FP_PROPS_LINE_WIDTH).into_string(),
        number_input(&width, |v| msg(Edit::SilkLineWidthTyped(v)), tokens),
        muted,
    );
    let clearance_row = control_row(
        &t!(i18n, ids::EDITORS_FP_PROPS_PAD_CLEARANCE).into_string(),
        number_input(&clearance, |v| msg(Edit::PadClearanceTyped(v)), tokens),
        muted,
    );
    let mut body = Column::new()
        .width(Length::Fill)
        .push(with_refusals(width_row, &[width.refusal], tokens, i18n))
        .push(with_refusals(
            clearance_row,
            &[clearance.refusal],
            tokens,
            i18n,
        ))
        .push(pin_one_row(cx));
    if fp.pin_one_dot_missing {
        body = body.push(pin_one_missing_note(cx));
    }
    let section = EnableSection::new(
        t!(i18n, ids::EDITORS_FP_PROPS_SILK_OUTLINE),
        settings.silk_outline,
    )
    .on_toggle(|_| msg(Edit::SilkOutlineToggled))
    .view(enable_section_style(tokens), body.boxed());
    let note = if fp.body_extent_mm.is_some() {
        t!(i18n, ids::EDITORS_FP_PROPS_SILK_CLIPPED)
    } else {
        t!(i18n, ids::EDITORS_FP_PROPS_NO_BODY)
    };
    with_header_note(section, note, cx)
}

fn pin_one_row<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let Ctx { fp, i18n, .. } = cx;
    let muted = cx.palette.muted;
    let check = signex_widgets::checkbox(fp.generated.pin_one_marker)
        .on_toggle(|_| msg(Edit::PinOneMarkerToggled))
        .size(TS.body)
        .spacing(0)
        .style(form_look::checkbox_style);
    let dot = text(t!(i18n, ids::EDITORS_FP_PROPS_PIN_ONE_DOT))
        .size(TS.caption)
        .color(muted);
    let field = row![check.boxed(), dot.boxed()]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    control_row(
        &t!(i18n, ids::EDITORS_FP_PROPS_ILLUSTRATION_PIN_ONE).into_string(),
        field.boxed(),
        muted,
    )
}

/// Says why the marker that is on draws no dot.
fn pin_one_missing_note<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let note = text(t!(cx.i18n, ids::EDITORS_FP_PROPS_PIN_ONE_NO_DOT))
        .size(TS.caption)
        .color(cx.palette.muted);
    container(note)
        .padding([0.0, SP.m])
        .width(Length::Fill)
        .boxed()
}

fn assembly_section<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let body: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    let section = EnableSection::new(
        t!(i18n, ids::EDITORS_FP_PROPS_ILLUSTRATION_ASSEMBLY),
        fp.generated.assembly_outline,
    )
    .on_toggle(|_| msg(Edit::AssemblyOutlineToggled))
    .view(enable_section_style(tokens), body.boxed());
    let note = match fp.body_extent_mm {
        Some([x0, y0, x1, y1]) => t!(
            i18n,
            ids::EDITORS_FP_PROPS_ASSEMBLY_BODY,
            "width" => format!("{:.2}", x1 - x0),
            "height" => format!("{:.2}", y1 - y0)
        ),
        None => t!(i18n, ids::EDITORS_FP_PROPS_NO_BODY),
    };
    with_header_note(section, note, cx)
}

/// `note` right-aligned on the section's header line.
fn with_header_note<'a>(
    section: Element<'a, PanelMsg>,
    note: impl Into<Label<'a>>,
    cx: Ctx<'a, '_>,
) -> Element<'a, PanelMsg> {
    let note = text(note).size(TS.caption).color(resolve(cx.tokens).ink4);
    let note = container(note)
        .padding(Padding {
            top: SP.m,
            right: SP.l,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right);
    stack![section, note].width(Length::Fill).boxed()
}
