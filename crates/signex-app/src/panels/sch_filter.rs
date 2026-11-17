//! SCH Filter panel (#2417): the schematic Selection Filter as a dock panel.
//! It is the chip grid of the Active Bar's filter dropdown and the board
//! filter (`active_bar::filter_menu_view`), wrapped to the panel's width, and
//! it edits the same filter: what a click, a box and the hover pick select.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::{PanelContext, PanelMsg};
use crate::active_bar::{ChipLayout, SelectionFilter, filter_menu_view, schematic_filter_menu};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What the SCH Filter panel's chip grid sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchFilterMsg {
    /// A category chip was clicked: turn that category on or off.
    KindToggled(SelectionFilter),
    /// All: every category on.
    AllPicked,
    /// None: every category off.
    NonePicked,
    /// A saved preset (by index) was clicked: its categories replace the set.
    PresetApplied(usize),
}

fn panel_msg(msg: SchFilterMsg) -> PanelMsg {
    PanelMsg::SchFilter(msg)
}

pub(super) fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let menu = schematic_filter_menu(
        &ctx.selection_filters,
        &ctx.custom_filter_presets,
        |kind| panel_msg(SchFilterMsg::KindToggled(kind)),
        |idx| panel_msg(SchFilterMsg::PresetApplied(idx)),
        panel_msg(SchFilterMsg::AllPicked),
        panel_msg(SchFilterMsg::NonePicked),
    );
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let hint = container(
        text(t!(ctx.i18n, ids::PANELS_SCH_FILTER_HINT))
            .size(TS.caption)
            .color(muted),
    )
    .padding([SP.s, SP.m]);
    Column::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(hint.boxed())
        .push(filter_menu_view(&ctx.tokens, menu, ChipLayout::Wrapped))
        .boxed()
}
