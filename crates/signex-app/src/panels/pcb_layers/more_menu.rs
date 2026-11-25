//! The more menu of the Layers panel (mockup row 103), opened in place by the more
//! button beside the search field. It saves and deletes layer sets (a set is
//! picked only in the Set dropdown of the Active row), holds Import... and
//! Export... (greyed until a view can be saved to a file), View From Bottom
//! Side and Layer Stack.... A click outside the card closes it.

use iced::widget::{Column, Space, Stack, opaque, row};
use iced::{Element, Length, Widget};
use signex_i18n::{MessageId, ids, t};
use signex_renderer::pcb::LayerSetId;
use signex_widgets::active_bar_dropdown::{self, DropdownEntry, DropdownItem};

use super::metrics::MENU;
use super::{LayersPanelMsg, PcbLayersPanel, SP, more_menu_top};
use crate::panels::click_away::click_away_backdrop;
use crate::panels::{PanelContext, PanelMsg};

fn panel_msg(msg: LayersPanelMsg) -> PanelMsg {
    PanelMsg::LayersPanel(msg)
}

/// The rows of the menu, top to bottom.
pub(super) fn entries(panel: &PcbLayersPanel) -> Vec<DropdownEntry<PanelMsg>> {
    let i18n = &panel.i18n;
    let label = |id: MessageId| t!(i18n, id).into_string();
    // A greyed row says why in its tooltip (#2480).
    let greyed = |id: MessageId, why: MessageId| {
        DropdownItem::passive(label(id))
            .disabled(true)
            .tooltip(label(why))
    };
    let delete = match &panel.layer_set {
        Some(LayerSetId::Saved(name)) => DropdownItem::new(
            label(ids::PCB_LAYERS_SET_DELETE_CURRENT),
            panel_msg(LayersPanelMsg::SetDeleted(name.clone())),
        ),
        _ => greyed(
            ids::PCB_LAYERS_SET_DELETE_CURRENT,
            ids::PCB_LAYERS_SET_DELETE_NONE,
        ),
    };
    let not_yet = |id: MessageId| greyed(id, ids::PCB_LAYERS_NOT_YET);
    let layer_stack = if panel.layer_stack_live {
        DropdownItem::new(
            label(ids::PCB_LAYERS_LAYER_STACK),
            panel_msg(LayersPanelMsg::LayerStackPicked),
        )
    } else {
        let why = panel.layer_stack_reason.unwrap_or(ids::PCB_LAYERS_NOT_YET);
        greyed(ids::PCB_LAYERS_LAYER_STACK, why)
    };
    let flip = DropdownItem::new(
        label(ids::PCB_LAYERS_VIEW_FROM_BOTTOM),
        panel_msg(LayersPanelMsg::BoardFlipPicked),
    )
    .lead_check(panel.flipped);
    vec![
        DropdownEntry::Header(i18n.upper_for_display(&label(ids::PCB_LAYERS_SETS))),
        DropdownEntry::Item(DropdownItem::new(
            label(ids::PCB_LAYER_SET_SAVE),
            panel_msg(LayersPanelMsg::SetNamingStarted),
        )),
        DropdownEntry::Item(delete),
        DropdownEntry::Separator,
        DropdownEntry::Item(not_yet(ids::PCB_LAYERS_IMPORT)),
        DropdownEntry::Item(not_yet(ids::PCB_LAYERS_EXPORT)),
        DropdownEntry::Separator,
        DropdownEntry::Item(flip),
        DropdownEntry::Separator,
        DropdownEntry::Item(layer_stack),
    ]
}

/// `body` with the open menu over it: a clear backdrop over the whole panel
/// that closes the menu, and the card under the more button at the right.
pub(super) fn overlay<'a>(
    body: Element<'a, PanelMsg>,
    panel: &PcbLayersPanel,
    ctx: &'a PanelContext,
) -> Element<'a, PanelMsg> {
    let card = active_bar_dropdown::view(entries(panel), &ctx.tokens, Some(MENU));
    let backdrop = click_away_backdrop(panel_msg(LayersPanelMsg::MoreMenuDismissed));
    // The card takes every press inside it, a greyed row's too, so only a
    // press outside reaches the backdrop.
    let anchored = Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(Space::new().height(more_menu_top()).boxed())
        .push(
            row![
                Space::new().width(Length::Fill),
                opaque(card).capture_scroll(true),
                Space::new().width(SP.m),
            ]
            .boxed(),
        );
    Stack::new()
        .push(body)
        .push(backdrop)
        .push(anchored.boxed())
        .boxed()
}
