//! Role sub-forms of a selected sketch entity (mockup row 126): Pour, Keepout,
//! Cutout, Sketch pad and Pattern. Each opens with its own header and a
//! line-art illustration, then its sections; the entity's attributes decide
//! which apply (an entity can carry several, a pattern source is also a
//! sketch pad).

use iced::widget::Column;
use iced::{Element, Length, Widget as _};
use signex_sketch::id::SketchEntityId;
use signex_types::theme::ThemeTokens;

use signex_types::sketch::SketchHost;

use super::super::{CollapsedSections, FootprintEditorPanelContext, PanelMsg, PanelPalette};

mod board_pattern;
mod cutout;
mod drawing;
mod expansion;
mod helpers;
mod keepout;
mod pattern;
mod pattern_drawing;
mod pour;
mod sketch_pad;
mod sketch_pad_drawing;

#[cfg(test)]
mod tests;

/// The sub-forms that apply to the selected entity `id`, one under the
/// other; `None` when the entity has no role attribute and sources no
/// pattern.
pub(in crate::panels::footprint_editor_properties) fn view_role_subform<'a>(
    fp: &'a FootprintEditorPanelContext,
    id: SketchEntityId,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    collapsed_sections: &'a CollapsedSections,
    i18n: &signex_i18n::I18n,
) -> Option<Element<'a, PanelMsg>> {
    let collapsed = collapsed_sections;
    let mut forms: Vec<Element<'a, PanelMsg>> = Vec::new();
    if let Some(pour) = fp.selected_pour.as_ref() {
        forms.push(pour::view(fp, pour, id, tokens, collapsed, i18n));
    }
    if let Some(keepout) = fp.selected_keepout.as_ref() {
        forms.push(keepout::view(fp, keepout, id, tokens, collapsed, i18n));
    }
    if let Some(cutout) = fp.selected_cutout.as_ref() {
        forms.push(cutout::view(fp, cutout, id, tokens, collapsed, i18n));
    }
    if let Some(pad) = fp.selected_sketch_pad.as_ref() {
        forms.push(sketch_pad::view(fp, pad, palette, tokens, collapsed, i18n));
    }
    if let Some(pattern) = fp.selected_array.as_ref() {
        // A board item has no pad number: the board form (mockup row 179e).
        forms.push(match fp.sketch_host {
            SketchHost::Board => board_pattern::view(fp, pattern, tokens, i18n),
            SketchHost::Footprint => pattern::view(fp, pattern, tokens, collapsed, i18n),
        });
    }
    if forms.is_empty() {
        return None;
    }
    Some(
        Column::with_children(forms)
            .spacing(0)
            .width(Length::Fill)
            .boxed(),
    )
}
