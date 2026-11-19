//! Additional Options: eleven checkboxes, closed by default, the header
//! counting the ones that are on. The view honours none of them yet
//! (#2480), so each is greyed with the "not yet" tooltip.

use iced::widget::container;
use iced::{Element, Length, Padding, Widget};
use signex_i18n::{ids, t};

use super::super::rows::{SectionHead, not_yet, section_with};
use super::super::{PcbLayersPanel, SP};
use super::{LINE_H, greyed_checkbox};
use crate::panels::{PanelContext, PanelMsg};

/// The token of each option, in its order.
const OPTIONS: [&str; 11] = [
    "test_points",
    "status_info",
    "pad_nets",
    "pad_numbers",
    "via_nets",
    "via_span",
    "all_connections",
    "net_color_override",
    "layer_colors_for_connections",
    "repeated_net_names",
    "special_strings",
];

/// The section, closed until opened, its header reading how many options
/// are on.
pub(super) fn section<'a>(
    panel: &PcbLayersPanel,
    ctx: &'a PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    // No option is honoured yet, so none is on.
    let on: u32 = 0;
    let total = OPTIONS.len() as u32;
    let head = SectionHead {
        count: Some(t!(panel.i18n, ids::PCB_VIEW_OPTIONS_ON, "on" => on, "total" => total)),
        eye: None,
        closed_by_default: true,
    };
    let rows = OPTIONS
        .into_iter()
        .map(|token| option_row(panel, ctx, token))
        .collect();
    section_with(
        ctx,
        panel,
        "pcb_view_additional",
        ids::PCB_VIEW_ADDITIONAL,
        head,
        rows,
    )
}

/// One option: a greyed checkbox with its label.
fn option_row<'a>(
    panel: &PcbLayersPanel,
    ctx: &PanelContext,
    token: &'static str,
) -> Element<'a, PanelMsg> {
    let label = t!(panel.i18n, ids::PCB_VIEW_OPTION, "option" => token);
    container(not_yet(greyed_checkbox(Some(label)), panel, ctx))
        .padding(Padding {
            top: 0.0,
            right: SP.m,
            bottom: 0.0,
            left: SP.m + SP.s,
        })
        .center_y(LINE_H)
        .width(Length::Fill)
        .boxed()
}
