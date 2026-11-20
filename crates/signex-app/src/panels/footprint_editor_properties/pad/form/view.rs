//! The pad form under the pad header (mockup row 125): the property illustration,
//! the layer chips of a through-hole pad, then the sections.

use iced::Element;
use iced::widget::Column;
use signex_types::theme::ThemeTokens;

use super::super::super::super::{
    CollapsedSections, FootprintEditorPanelContext, PanelMsg, PanelPalette,
};
use super::super::illustration::{layer_chips, pad_illustration};
use super::{Form, PadClass, PadEditTarget, PadFormValues};

/// The illustration and every pad section for `target`: the selected pad
/// or the next pad placed. Nothing is added when `target` names a pad the
/// context does not hold.
pub(in crate::panels::footprint_editor_properties) fn view_pad_form<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    target: PadEditTarget,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    collapsed_sections: &'a CollapsedSections,
    i18n: &signex_i18n::I18n,
) -> Column<Element<'a, PanelMsg>> {
    let values = match target {
        PadEditTarget::Next => PadFormValues::from_next_pad(fp),
        PadEditTarget::Selected(idx) => match fp.selected_pad.as_ref().filter(|p| p.idx == idx) {
            Some(pad) => PadFormValues::from_selected_pad(pad, fp),
            None => return col,
        },
    };
    let form = Form {
        values: &values,
        target,
        class: PadClass::of(&values),
        palette,
        tokens,
        hover: &fp.illustration_hover,
        i18n,
        collapsed: collapsed_sections,
    };
    let drawing = pad_illustration(&form);
    col = col.push(super::super::super::illustration::view_illustration(
        drawing,
        &fp.illustration_hover,
        tokens,
    ));
    if form.class.has_hole() {
        col = col.push(layer_chips(&form));
    }
    col = super::properties::push(col, &form);
    let shape_params: &'a [crate::panels::PadShapeParamSummary] = match target {
        PadEditTarget::Selected(_) => &fp.selected_pad_shape_params,
        PadEditTarget::Next => &[],
    };
    col = super::stack::push(col, &form, shape_params);
    col = super::hole::push(col, &form);
    col = super::paste_mask::push(col, &form);
    super::features::push(col, &form)
}
