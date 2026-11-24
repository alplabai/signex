//! Properties panel of the Footprint editor: the router between the pad
//! form (a selected pad, or the next pad while placing) and the footprint
//! body (everything else), both in one scrollable.

use iced::widget::{Column, container, scrollable};
use iced::{Color, Element, Length, Widget};
use signex_types::theme::ThemeTokens;

use super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, PanelMsg, PanelPalette,
};
use crate::styles::{Spacing, TypeScale};
use pad::PadEditTarget;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Room kept on the right of the scrolled form so the scrollbar never
/// covers the right edge of the fields.
const SCROLLBAR_GUTTER: f32 = 12.0;

// Submodule declarations (split from the original 4570-line file).
mod bake_outcome;
mod bake_panel;
mod board;
mod body;
mod conflicts;
mod dof_card;
mod illustration;
mod managers;
mod pad;
mod pad_stack_check;
mod redundant;
mod roles;
mod rows;
mod sections;
mod selection;
mod silk;
mod snap_options;
mod subforms;

#[cfg(test)]
mod header_tests;

/// The Properties panel of the active footprint editor. Pads mode with a
/// pad selected (outside placement), or a placement running, shows the pad
/// header and the pad form; every other state shows the footprint body.
pub(super) fn view_footprint_editor_properties<'a>(
    fp: &'a FootprintEditorPanelContext,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    collapsed_sections: &'a CollapsedSections,
    unit: signex_types::coord::Unit,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    if let Some(target) = pad_target(fp) {
        let mut col = Column::new().spacing(0).width(Length::Fill);
        if let Some(outcome) = fp.bake_outcome.as_ref() {
            let pad = fp.selected_pad.as_ref();
            col = bake_outcome::view_bake_outcome(col, outcome, pad, palette, tokens, i18n);
        }
        col = col.push(pad::view_pad_header(fp, target, tokens, i18n));
        if let Some(section) = pad::view_driven_by(fp, target, palette, tokens, i18n) {
            col = col.push(section);
        }
        return scrolled(pad::view_pad_form(
            col,
            fp,
            target,
            palette,
            tokens,
            collapsed_sections,
            i18n,
        ));
    }
    scrolled(body::view_body(
        fp,
        palette,
        tokens,
        collapsed_sections,
        unit,
        i18n,
    ))
}

/// The pad the form edits: the next pad while a placement runs or is
/// paused, else the selected pad in Pads mode. `None` shows the body.
fn pad_target(fp: &FootprintEditorPanelContext) -> Option<PadEditTarget> {
    if fp.mode_kind != FootprintModeKind::Pads {
        return None;
    }
    if fp.placement_active || fp.placement_paused {
        return Some(PadEditTarget::Next);
    }
    fp.selected_pad
        .as_ref()
        .map(|pad| PadEditTarget::Selected(pad.idx))
}

/// `col` in the panel's scrollable, with the scrollbar gutter.
fn scrolled<'a>(col: Column<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    scrollable(container(col).padding(iced::Padding {
        top: 0.0,
        right: SCROLLBAR_GUTTER,
        bottom: 0.0,
        left: 0.0,
    }))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::panels::form_look::scrollbar_style)
    .boxed()
}

/// Read-only key-value row — delegates to the schematic Properties
/// panel's `form_input_row` so the footprint editor uses identical
/// chrome (orange-accent border, dark-blue selection-tinted background).
/// Returns the updated Column to keep the chained-update call style
/// the rest of this module uses.
fn props_kv_row<'a>(
    col: Column<Element<'a, PanelMsg>>,
    label_c: Color,
    input_bg: Color,
    key: &str,
    value: String,
) -> Column<Element<'a, PanelMsg>> {
    col.push(rows::value_row(key, &value, label_c, input_bg))
}
