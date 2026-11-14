//! The PCB Active Bar (#2352), laid out like the footprint editor's in the
//! usual PCB tool order: selection filter, move, align | place component,
//! pad, via, track, arc, fill, region, polygon pour, text, dimension, keepout
//! | interactive routing | board sketch | view.
//!
//! Every live button runs its command through the menu bar's own message, so
//! a click is the menu row. A tool the board editor cannot run yet is greyed
//! and its tooltip says so, from the table Place on a board greys by
//! (`app/pcb_tools.rs`). Move and View open a dropdown on right-click; its
//! rows are greyed by the same gate as their menu rows.

use iced::alignment::Horizontal;
use iced::widget::{Space, column, container, mouse_area, row, stack, svg};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem, Metrics};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, MenuMetrics};

use crate::app::command::{CommandContext, is_enabled};
use crate::app::pcb_tools::UnavailableBoardTool;
use crate::app::{Message, PcbBarMenu, PcbBarMsg, PcbMsg, Signex};
use crate::icons as ic;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

#[cfg(test)]
mod tests;

impl Signex {
    /// The PCB Active Bar of the main window, with its open dropdown under
    /// its button and a click-outside layer that closes it. Nothing in an
    /// undocked window.
    pub(in crate::app::view) fn pcb_active_bar(&self, is_main: bool) -> Element<'_, Message> {
        if !is_main {
            return Space::new().boxed();
        }
        let tid = self.ui_state.effective_theme_id();
        let tokens = &self.document_state.panel_ctx.tokens;
        let open = self.interaction_state.pcb_bar_menu;
        let items = pcb_bar_items(tid, &self.ui_state.i18n, open);
        let (offsets, bar_width) =
            signex_widgets::active_bar::slot_offsets(&items, &Metrics::GRAPHITE);
        let anchor = open.and_then(|menu| {
            let slot = items.iter().position(|item| opens(item, menu))?;
            Some((menu, offsets[slot]))
        });
        let bar = signex_widgets::active_bar::view(items, tokens);
        let top = Space::new().height(crate::active_bar::bar_top_margin());
        let Some((menu, trigger_x)) = anchor else {
            return column![
                top,
                container(bar)
                    .width(Length::Fill)
                    .align_x(Horizontal::Center),
            ]
            .boxed();
        };
        let entries =
            pcb_bar_menu_entries(menu, &self.ui_state.i18n, tid, &self.command_context(None));
        let panel_width = MenuMetrics::GRAPHITE.fitted_width(
            &entries,
            bar_width,
            &self.document_state.panel_ctx.ui_font_name,
        );
        let panel = signex_widgets::active_bar_dropdown::view(entries, tokens, Some(panel_width));
        // The panel stays inside the bar's width, so the centred bar does not
        // move while its dropdown is open.
        let panel_x = trigger_x.min(bar_width - panel_width).max(0.0);
        let bar_and_panel = column![
            bar,
            Space::new().height(crate::active_bar::BAR_DROPDOWN_GAP_PX),
            row![Space::new().width(panel_x), panel],
        ]
        .width(bar_width);
        let dismiss = mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(bar_message(PcbBarMsg::MenuDismissed));
        stack![
            dismiss,
            column![
                top,
                container(bar_and_panel)
                    .width(Length::Fill)
                    .align_x(Horizontal::Center),
            ],
        ]
        .boxed()
    }
}

fn bar_message(event: PcbBarMsg) -> Message {
    Message::Pcb(PcbMsg::Bar(event))
}

/// The message a button or row sends to run `command` as its menu row does.
fn run(command: &str) -> Option<Message> {
    AppCommandId::new(command)
        .ok()
        .map(|id| Message::Menu(MenuMessage::Command(id)))
}

/// Whether `item` is the button whose right-click opens `menu`.
fn opens(item: &ActiveBarItem<Message>, menu: PcbBarMenu) -> bool {
    let ActiveBarItem::Button(button) = item else {
        return false;
    };
    matches!(
        &button.on_right_press,
        Some(Message::Pcb(PcbMsg::Bar(PcbBarMsg::MenuToggled(m)))) if *m == menu
    )
}

/// A button that runs `command`.
fn command_button(command: &str, icon: ActiveBarIcon, tooltip: String) -> ActiveBarItem<Message> {
    let on_press = run(command);
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip,
        enabled: on_press.is_some(),
        on_press,
        ..ActiveBarButton::default()
    })
}

/// A button that runs `command` on a click and opens `menu` on a
/// right-click, chevron and all.
fn menu_button(
    command: &str,
    menu: PcbBarMenu,
    open: Option<PcbBarMenu>,
    icon: ActiveBarIcon,
    tooltip: String,
    tid: ThemeId,
) -> ActiveBarItem<Message> {
    let on_press = run(command);
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip,
        enabled: on_press.is_some(),
        selected: open == Some(menu),
        on_press,
        on_right_press: Some(bar_message(PcbBarMsg::MenuToggled(menu))),
        dropdown_indicator: Some(ActiveBarIcon::Svg(ic::icon_chevron_45(tid))),
    })
}

/// A greyed button for a tool the board editor cannot run yet; its tooltip
/// says so.
fn unavailable_button(
    icon: ActiveBarIcon,
    tooltip: MessageId,
    i18n: &I18n,
) -> ActiveBarItem<Message> {
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip: t!(i18n, tooltip).into_string(),
        enabled: false,
        ..ActiveBarButton::default()
    })
}

/// The PCB Active Bar's slots in order. `open` is the open dropdown, whose
/// button is drawn selected.
pub(in crate::app::view) fn pcb_bar_items(
    tid: ThemeId,
    i18n: &I18n,
    open: Option<PcbBarMenu>,
) -> Vec<ActiveBarItem<Message>> {
    let svg = ActiveBarIcon::Svg;
    let off = |icon, tooltip| unavailable_button(icon, tooltip, i18n);
    let tool = |tool: UnavailableBoardTool| off(tool_icon(tool, tid), tool.reason());
    let mut items = vec![
        off(svg(ic::icon_filter(tid)), ids::PCB_BAR_FILTER_UNAVAILABLE),
        menu_button(
            "move_object",
            PcbBarMenu::Move,
            open,
            svg(ic::icon_move(tid)),
            t!(i18n, ids::PCB_BAR_MOVE_TOOLTIP).into_string(),
            tid,
        ),
        off(svg(ic::icon_align(tid)), ids::PCB_BAR_ALIGN_UNAVAILABLE),
        ActiveBarItem::Separator,
    ];
    items.extend(BAR_PLACE_TOOLS.into_iter().map(&tool));
    items.extend([
        ActiveBarItem::Separator,
        tool(UnavailableBoardTool::InteractiveRouting),
        ActiveBarItem::Separator,
        command_button(
            "board_sketch",
            svg(ic::icon_board_sketch(tid)),
            t!(i18n, ids::BOARD_SKETCH_TOOLTIP).into_string(),
        ),
        ActiveBarItem::Separator,
        menu_button(
            "view_rotate",
            PcbBarMenu::View,
            open,
            svg(ic::icon_view_rotate(tid)),
            t!(i18n, ids::PCB_VIEW_ROTATE_TOOLTIP).into_string(),
            tid,
        ),
    ]);
    items
}

/// The place group's greyed tools, in the PCB editor's order; Interactive
/// Routing has a group of its own.
const BAR_PLACE_TOOLS: [UnavailableBoardTool; 11] = [
    UnavailableBoardTool::Component,
    UnavailableBoardTool::Pad,
    UnavailableBoardTool::Via,
    UnavailableBoardTool::Track,
    UnavailableBoardTool::Arc,
    UnavailableBoardTool::Fill,
    UnavailableBoardTool::SolidRegion,
    UnavailableBoardTool::PolygonPour,
    UnavailableBoardTool::TextString,
    UnavailableBoardTool::Dimension,
    UnavailableBoardTool::Keepout,
];

/// The button icon of `tool`: the footprint editor's where one fits, its
/// glyphs for pad and via.
fn tool_icon(tool: UnavailableBoardTool, tid: ThemeId) -> ActiveBarIcon {
    use UnavailableBoardTool as T;
    let svg = ActiveBarIcon::Svg;
    match tool {
        T::Component => svg(ic::icon_component(tid)),
        T::Pad => ActiveBarIcon::Glyph("\u{25CF}"),
        T::Via => ActiveBarIcon::Glyph("\u{25CE}"),
        T::InteractiveRouting => svg(ic::icon_wire(tid)),
        T::Track => svg(ic::icon_shape_line(tid)),
        T::Arc => svg(ic::icon_shape_arc(tid)),
        T::Fill => svg(ic::icon_shape_rect(tid)),
        T::SolidRegion => svg(ic::icon_shape_polygon(tid)),
        T::PolygonPour => svg(ic::icon_dd_polygon(tid)),
        T::TextString => svg(ic::icon_text(tid)),
        T::Dimension => ActiveBarIcon::Glyph("\u{2194}"),
        T::Keepout => ActiveBarIcon::Glyph("\u{2298}"),
    }
}

/// A dropdown row: the command it runs and its icon.
type MenuRow = (&'static str, fn(ThemeId) -> svg::Handle);

/// The Move dropdown: the board placement commands of Edit ▸ Move.
const MOVE_ROWS: &[MenuRow] = &[
    ("move_object", ic::icon_dd_move),
    ("move_selection", ic::icon_dd_move_sel),
    ("rotate_counterclockwise", ic::icon_dd_rotate),
    ("rotate_clockwise", ic::icon_dd_rotate_cw),
    ("flip_selection", ic::icon_dd_flip_x),
];

/// The View dropdown: the board view turns of the View menu.
const VIEW_ROWS: &[MenuRow] = &[
    ("view_rotate", ic::icon_dd_rotate_cw),
    ("view_rotate_ccw", ic::icon_dd_rotate),
    ("view_rotate_180", ic::icon_view_rotate),
    ("view_rotate_reset", ic::icon_view_rotate),
];

fn menu_commands(menu: PcbBarMenu) -> &'static [MenuRow] {
    match menu {
        PcbBarMenu::Move => MOVE_ROWS,
        PcbBarMenu::View => VIEW_ROWS,
    }
}

/// The rows of `menu`: each runs its command, labelled and greyed as its
/// menu row is in `ctx`.
pub(in crate::app::view) fn pcb_bar_menu_entries(
    menu: PcbBarMenu,
    i18n: &I18n,
    tid: ThemeId,
    ctx: &CommandContext,
) -> Vec<DropdownEntry<Message>> {
    menu_commands(menu)
        .iter()
        .filter_map(|&(id, icon)| {
            let command = AppCommandId::new(id).ok()?;
            let enabled = is_enabled(&command, ctx);
            let label = crate::menu_bar::cmd_label(i18n, id, id);
            let pick = bar_message(PcbBarMsg::CommandPicked(command));
            let row = DropdownItem::new(label, pick)
                .icon(icon(tid))
                .command(id)
                .disabled(!enabled);
            Some(DropdownEntry::Item(row))
        })
        .collect()
}
