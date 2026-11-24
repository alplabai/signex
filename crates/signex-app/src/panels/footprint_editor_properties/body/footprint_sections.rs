//! The Footprint part of the nothing-selected body: the pads-and-courtyard
//! illustration, the Footprint
//! section (name, description, designator, type, height, version, units)
//! and the Courtyard & Silkscreen section, whose generated silkscreen and
//! assembly rows are in `generated_sections`.

use iced::widget::{Column, container, stack};
use iced::{Element, Length, Padding, Widget as _};
use signex_i18n::{ids, t};
use signex_types::coord::Unit;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::{Label, text};

use super::header::{is_open, section, segment};
use super::{Ctx, fields};
use crate::panels::PanelMsg;
use crate::panels::enable_section::enable_section_style;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const FOOTPRINT_KEY: &str = "fp_footprint";
const COURTYARD_KEY: &str = "fp_courtyard";

pub(super) fn view<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let fp = cx.fp;
    if let Some(illustration) = super::footprint_illustration::illustration(fp, cx.i18n) {
        col = col.push(super::super::illustration::view_illustration(
            illustration,
            &fp.illustration_hover,
            cx.tokens,
        ));
    }
    col = footprint_section(col, cx);
    col = courtyard_section(col, cx);
    if is_open(COURTYARD_KEY, cx.collapsed) {
        col = super::generated_sections::view(col, cx);
    }
    col
}

fn footprint_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    use super::super::pad::{Shown, number_input, with_refusals};
    use super::super::rows::{control_row, edit_row, edit_row_note, pick_row};
    use crate::library::editor::footprint::FOOTPRINT_HEIGHT_BUFFER;
    use signex_library::primitive::footprint::ComponentType;
    let Ctx { fp, i18n, .. } = cx;
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_FOOTPRINT);
    col = col.push(section(
        FOOTPRINT_KEY,
        &title,
        None,
        cx.collapsed,
        cx.tokens,
        i18n,
    ));
    if !is_open(FOOTPRINT_KEY, cx.collapsed) {
        return col;
    }
    let muted = cx.palette.muted;
    let key = |id: signex_i18n::MessageId| t!(i18n, id).into_string();
    col = col.push(edit_row(
        &key(ids::EDITORS_FP_PROPS_NAME),
        &fp.footprint_name,
        muted,
        PanelMsg::FpEditorSetFootprintName,
    ));
    col = col.push(edit_row(
        &key(ids::EDITORS_FP_PROPS_DESCRIPTION),
        &fp.footprint_description,
        muted,
        PanelMsg::FpEditorSetFootprintDescription,
    ));
    col = col.push(edit_row_note(
        &key(ids::EDITORS_FP_PROPS_DESIGNATOR),
        &fp.footprint_default_designator,
        t!(i18n, ids::EDITORS_FP_PROPS_DESIGNATOR_DEFAULT),
        muted,
        PanelMsg::FpEditorSetFootprintDefaultDesignator,
    ));
    col = col.push(pick_row(
        &key(ids::EDITORS_FP_PROPS_TYPE),
        ComponentType::ALL,
        fp.footprint_component_type,
        PanelMsg::FpEditorSetFootprintComponentType,
        muted,
    ));
    // Refused text stays in the field with its reason (mockup row 124).
    let height = Shown::from_buffer(
        fp.numeric_buffers.get(FOOTPRINT_HEIGHT_BUFFER),
        fp.footprint_height_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default(),
        false,
        |_| true,
    );
    let height_row = control_row(
        &key(ids::EDITORS_FP_PROPS_HEIGHT),
        number_input(&height, PanelMsg::FpEditorSetFootprintHeight, cx.tokens),
        muted,
    );
    col = col.push(with_refusals(
        height_row,
        &[height.refusal],
        cx.tokens,
        i18n,
    ));
    col = super::super::props_kv_row(
        col,
        muted,
        cx.palette.input_bg,
        &key(ids::EDITORS_FP_PROPS_VERSION),
        fp.version.clone(),
    );
    col.push(units_row(cx))
}

/// The Units row: mm or mils for the whole panel (`PanelMsg::SetUnit`).
pub(super) fn units_row<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let i18n = cx.i18n;
    let units = [
        (Unit::Mm, Label::from(t!(i18n, ids::PROPERTIES_UNIT_MM))),
        (Unit::Mil, Label::from(t!(i18n, ids::PROPERTIES_UNIT_MILS))),
    ];
    super::super::rows::control_row(
        &t!(i18n, ids::PANELS_PROPERTIES_UNITS).into_string(),
        segment(units, cx.unit, cx.tokens, PanelMsg::SetUnit),
        cx.palette.muted,
    )
}

/// The courtyard's Line width (mockup row 124): refused text stays in the
/// field with its reason; live whether or not the courtyard is auto-fit.
fn courtyard_line_width_row<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    use super::super::pad::{Shown, number_input, with_refusals};
    use crate::library::editor::footprint::COURTYARD_LINE_WIDTH_BUFFER;
    use crate::panels::FootprintFeatureEdit;
    let stored = cx.fp.generated.courtyard_line_width_mm;
    let text = super::generated_sections::live_text(
        cx.fp.numeric_buffers.get(COURTYARD_LINE_WIDTH_BUFFER),
        stored,
        |v| v > 0.0,
    );
    let width = Shown::from_buffer(text, stored.to_string(), true, |_| true);
    let row = super::super::rows::control_row(
        &t!(cx.i18n, ids::EDITORS_FP_PROPS_LINE_WIDTH).into_string(),
        number_input(
            &width,
            |v| {
                PanelMsg::FpEditorGeneratedFeature(FootprintFeatureEdit::CourtyardLineWidthTyped(v))
            },
            cx.tokens,
        ),
        cx.palette.muted,
    );
    with_refusals(row, &[width.refusal], cx.tokens, cx.i18n)
}

/// Courtyard & Silkscreen: the generated courtyard as an enable header,
/// its size at the header's right and its fixed expansion below; then the
/// courtyard line width, live with the switch off.
fn courtyard_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    use super::super::illustration::linked_field;
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_COURTYARD);
    col = col.push(section(
        COURTYARD_KEY,
        &title,
        None,
        cx.collapsed,
        tokens,
        i18n,
    ));
    if !is_open(COURTYARD_KEY, cx.collapsed) {
        return col;
    }
    let hover = &fp.illustration_hover;
    let expansion = t!(
        i18n,
        ids::EDITORS_FP_PROPS_LENGTH_MM,
        "value" => format!("{:.2}", fp.courtyard_expansion_mm)
    );
    let expansion = super::super::rows::value_row(
        &t!(i18n, ids::EDITORS_FP_PROPS_EXPANSION).into_string(),
        &expansion.into_string(),
        cx.palette.muted,
        cx.palette.input_bg,
    );
    let body = Column::new().width(Length::Fill).push(linked_field(
        expansion,
        fields::EXPANSION,
        hover,
        tokens,
    ));
    let courtyard = EnableSection::new(
        t!(i18n, ids::EDITORS_FP_PROPS_COURTYARD),
        fp.auto_fit_courtyard,
    )
    .on_toggle(|_| PanelMsg::FpEditorToggleAutoFitCourtyard)
    .view(enable_section_style(tokens), body.boxed());
    // The line width strokes a hand-drawn courtyard too, so it stays live
    // under the switch (maintainer decision 2026-11-24).
    let header = match fp.courtyard_extent_mm {
        Some(extent) => courtyard_with_size(courtyard, extent, cx),
        None => courtyard,
    };
    col.push(header).push(courtyard_line_width_row(cx))
}

/// The courtyard section with its size on the header line.
fn courtyard_with_size<'a>(
    courtyard: Element<'a, PanelMsg>,
    [x0, y0, x1, y1]: [f64; 4],
    cx: Ctx<'a, '_>,
) -> Element<'a, PanelMsg> {
    use super::super::illustration::linked_field;
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let hover = &fp.illustration_hover;
    let size = t!(
        i18n,
        ids::EDITORS_FP_PROPS_COURTYARD_SIZE,
        "width" => format!("{:.2}", x1 - x0),
        "height" => format!("{:.2}", y1 - y0)
    );
    // The size sits on the header line, right-aligned over the section.
    let size = text(size).size(TS.caption).color(resolve(tokens).ink4);
    let size = linked_field(size.boxed(), fields::COURTYARD, hover, tokens);
    let size = container(size)
        .padding(Padding {
            top: SP.m,
            right: SP.l,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right);
    stack![courtyard, size].width(Length::Fill).boxed()
}
