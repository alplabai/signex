//! The tool group buttons of the PCB Active Bar (#2392), from the group
//! table in `app/pcb_bar_groups.rs`. A button shows and runs the row its
//! group used last, drawn selected while one of its tools is armed or its
//! dropdown is open; a right-click opens the group's rows, greyed by the menu
//! bar's gate, or with why for a tool the board editor cannot run yet. A
//! group whose rows are all greyed is one greyed button that says why.

use iced::widget::svg;
use signex_i18n::{I18n, t};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use super::{bar_message, run, tool_icon};
use crate::app::command::{CommandContext, is_enabled};
use crate::app::pcb_bar_groups::{PcbBarGroup, PcbBarLastTools, PcbBarRow};
use crate::app::pcb_tools::{BoardPlaceTool, ToolState};
use crate::app::{Message, PcbBarMenu, PcbBarMsg};
use crate::icons as ic;
use crate::keymap::AppCommandId;

/// What the group buttons are drawn from.
pub(super) struct GroupState<'a> {
    /// The open dropdown.
    pub open: Option<PcbBarMenu>,
    /// The row each group shows.
    pub last: &'a PcbBarLastTools,
    /// The tool armed on the board in view.
    pub armed: Option<BoardPlaceTool>,
}

impl GroupState<'_> {
    /// Whether `group`'s button is drawn selected: its dropdown is open or
    /// one of its tools is armed, as on the schematic bar.
    fn is_selected(&self, group: PcbBarGroup) -> bool {
        let armed = self
            .armed
            .is_some_and(|tool| group.rows().contains(&PcbBarRow::Tool(tool)));
        armed || self.open == Some(PcbBarMenu::Group(group))
    }
}

/// The button of `group`: it runs the row `state.last` holds for it, or is
/// greyed with why when no row of the group can run yet.
pub(super) fn group_button(
    group: PcbBarGroup,
    state: &GroupState<'_>,
    tid: ThemeId,
    i18n: &I18n,
) -> ActiveBarItem<Message> {
    let row = state.last.current(group);
    let icon = button_icon(row, tid);
    let tooltip = row_tooltip(row, i18n);
    let Some(on_press) = row.command().and_then(run) else {
        return ActiveBarItem::Button(ActiveBarButton {
            icon,
            tooltip,
            enabled: false,
            ..ActiveBarButton::default()
        });
    };
    let menu = PcbBarMenu::Group(group);
    let has_menu = group.rows().len() > 1;
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip,
        enabled: true,
        selected: state.is_selected(group),
        on_press: Some(on_press),
        on_right_press: has_menu.then(|| bar_message(PcbBarMsg::MenuToggled(menu))),
        dropdown_indicator: has_menu.then(|| ActiveBarIcon::Svg(ic::icon_chevron_45(tid))),
    })
}

/// The rows of `group`'s dropdown. A live row runs its command, greyed as
/// its menu row is in `ctx`; a tool the board editor cannot run yet is
/// greyed and its tooltip says why.
pub(super) fn entries(
    group: PcbBarGroup,
    i18n: &I18n,
    tid: ThemeId,
    ctx: &CommandContext,
) -> Vec<DropdownEntry<Message>> {
    group
        .rows()
        .iter()
        .map(|&row| {
            let label = row_label(row, i18n);
            let live = row
                .command()
                .and_then(|id| Some((id, AppCommandId::new(id).ok()?)));
            let item = match live {
                Some((id, command)) => {
                    let enabled = is_enabled(&command, ctx);
                    DropdownItem::new(label, bar_message(PcbBarMsg::CommandPicked(command)))
                        .command(id)
                        .disabled(!enabled)
                }
                None => DropdownItem::passive(label)
                    .disabled(true)
                    .tooltip(row_tooltip(row, i18n)),
            };
            DropdownEntry::Item(item.icon(menu_icon(row, tid)))
        })
        .collect()
}

/// The row's name, as its Place (or Edit) row reads.
fn row_label(row: PcbBarRow, i18n: &I18n) -> String {
    match row {
        PcbBarRow::Tool(tool) => t!(i18n, tool.name()).into_string(),
        PcbBarRow::Command(id) => crate::menu_bar::cmd_label(i18n, id, id),
    }
}

/// The hover text of a button showing `row`: the live tool's tooltip, the
/// reason a tool cannot run yet, or the command's name.
fn row_tooltip(row: PcbBarRow, i18n: &I18n) -> String {
    match row {
        PcbBarRow::Tool(tool) => match tool.state() {
            ToolState::Live(_) => t!(i18n, tool.texts().0).into_string(),
            ToolState::Unavailable(reason) => t!(i18n, reason).into_string(),
        },
        PcbBarRow::Command(id) => crate::menu_bar::cmd_label(i18n, id, id),
    }
}

/// The bar button's icon for `row`: the tool's bar icon, the repour glyph
/// for Repour (#2216), the component glyph for a command of the Components
/// group.
fn button_icon(row: PcbBarRow, tid: ThemeId) -> ActiveBarIcon {
    match row {
        PcbBarRow::Tool(tool) => tool_icon(tool, tid),
        PcbBarRow::Command(id) if is_repour(id) => ActiveBarIcon::Svg(ic::icon_redo(tid)),
        PcbBarRow::Command(_) => ActiveBarIcon::Svg(ic::icon_component(tid)),
    }
}

/// Whether `command` is one of the Repour commands of the Copper Areas group.
fn is_repour(command: &str) -> bool {
    matches!(command, "repour_selected" | "repour_all")
}

/// The dropdown row's icon for `row`, from the dropdown set and the board
/// tool glyphs.
fn menu_icon(row: PcbBarRow, tid: ThemeId) -> svg::Handle {
    use BoardPlaceTool as T;
    let tool = match row {
        PcbBarRow::Tool(tool) => tool,
        PcbBarRow::Command(id) if is_repour(id) => return ic::icon_redo(tid),
        PcbBarRow::Command(_) => return ic::icon_dd_part_actions(tid),
    };
    match tool {
        T::Component => ic::icon_component(tid),
        T::Pad => ic::icon_pcb_pad(tid),
        T::Via => ic::icon_pcb_via(tid),
        T::InteractiveRouting => ic::icon_dd_wire(tid),
        T::Track => ic::icon_dd_line(tid),
        T::Arc => ic::icon_dd_arc(tid),
        T::Fill => ic::icon_dd_rect(tid),
        T::SolidRegion => ic::icon_shape_polygon(tid),
        T::PolygonPour => ic::icon_dd_polygon(tid),
        T::PourCutout => ic::icon_dd_cut(tid),
        T::SlicePour => ic::icon_dd_line(tid),
        T::TextString => ic::icon_dd_text_string(tid),
        T::Dimension => ic::icon_pcb_dimension(tid),
        T::Keepout => ic::icon_pcb_keepout(tid),
        T::Line => ic::icon_dd_line(tid),
        T::GraphicArc => ic::icon_dd_arc(tid),
        T::FullCircle => ic::icon_dd_circle(tid),
        T::Rectangle => ic::icon_dd_rect(tid),
        T::TextFrame => ic::icon_dd_text_frame(tid),
    }
}
