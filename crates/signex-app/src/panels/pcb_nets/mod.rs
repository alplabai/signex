//! PCB panel (mockup row 106): one dock panel with a mode dropdown, each
//! mode a list over the selected item's detail. Nets lists the board's nets
//! with a filter, chips and a Class column; Net Classes the class table
//! with the selected class's rules and members; Differential Pairs the
//! pairs the router finds; xSignals the board's xSignals; Polygons the
//! pours of the Polygon Manager. A pure view over data the panel context
//! copies from the board, so a detached panel window (mockup row 8) shows
//! the same mode as the docked one.

mod detail;
mod model;
mod modes;
mod table;
#[cfg(test)]
mod tests;

use iced::widget::{column, container, pick_list};
use iced::{Background, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_types::net_class::NetClass;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text;

pub use model::{
    PcbDiffPairRow, PcbNetChip, PcbNetLayer, PcbNetPad, PcbNetRow, PcbNetsSummary, PcbPanelMode,
    PcbPanelMsg, PcbPanelUi, PcbPourChip, PcbPourRow, highlight_toggle,
};

use super::{PanelContext, PanelMsg};
use crate::styles::{HAIRLINE, Spacing, TypeScale, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One entry of the mode dropdown.
#[derive(Debug, Clone, PartialEq)]
struct ModeChoice {
    mode: PcbPanelMode,
    label: String,
}

fn msg(event: PcbPanelMsg) -> PanelMsg {
    PanelMsg::PcbPanel(event)
}

/// The project's classes the board reads; none outside a project.
fn board_classes(ctx: &PanelContext) -> &[NetClass] {
    ctx.net_classes
        .board
        .as_ref()
        .map(|board| board.classes.as_slice())
        .unwrap_or_default()
}

/// A `line-soft` hairline across the panel.
fn separator<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let rule = resolve(&ctx.tokens).line_soft;
    container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(rule)),
            ..container::Style::default()
        })
        .boxed()
}

/// A mode's dropdown entry: its name and how many items it lists.
fn mode_label(i18n: &signex_i18n::I18n, mode: PcbPanelMode, count: usize) -> String {
    let name = t!(i18n, mode.message()).into_string();
    t!(i18n, ids::PANELS_PCB_MODE_COUNT, "mode" => name, "count" => count).into_string()
}

/// The mode dropdown at the top of the panel, so a docked, floating and
/// detached panel all carry it.
fn mode_row<'a>(
    ctx: &'a PanelContext,
    summary: &PcbNetsSummary,
    current: PcbPanelMode,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let classes = board_classes(ctx);
    let choice = |mode: PcbPanelMode| ModeChoice {
        mode,
        label: mode_label(i18n, mode, mode.count(summary, classes)),
    };
    let options: Vec<ModeChoice> = PcbPanelMode::ALL.into_iter().map(choice).collect();
    container(
        pick_list(Some(choice(current)), options, |c: &ModeChoice| {
            c.label.clone()
        })
        .graphite()
        .on_select(|c: ModeChoice| msg(PcbPanelMsg::ModePicked(c.mode))),
    )
    .padding([SP.xs, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// Render the panel for the active PCB tab.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let Some(summary) = ctx.pcb_nets.as_ref() else {
        return container(
            text(t!(&*ctx.i18n, ids::PANELS_PCB_NETS_NO_BOARD))
                .size(TS.caption)
                .color(resolve(&ctx.tokens).ink3),
        )
        .padding([6.0, SP.m])
        .width(Length::Fill)
        .boxed();
    };
    let body: Element<'a, PanelMsg> = match ctx.pcb_panel.mode {
        PcbPanelMode::Nets => table::nets_mode(ctx, summary),
        PcbPanelMode::NetClasses => modes::classes_mode(ctx, summary),
        PcbPanelMode::DiffPairs => modes::pairs_mode(ctx, summary),
        PcbPanelMode::XSignals => modes::xsignals_mode(ctx),
        PcbPanelMode::Polygons => modes::polygons_mode(ctx, summary),
    };
    column![
        mode_row(ctx, summary, ctx.pcb_panel.mode),
        separator(ctx),
        body
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
