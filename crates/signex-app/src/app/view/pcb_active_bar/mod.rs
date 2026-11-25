//! The PCB Active Bar (#2352, mockup row 108): selection filter, move, align |
//! components | route, tune, via and pad, copper areas, copper drawing |
//! dimensions, text, graphics | board sketch | view.
//!
//! Every live button runs its command through the menu bar's own message, so
//! a click is the menu row. The tool groups (#2392, `groups.rs`) show the row
//! their group used last and open their rows on a right-click; a tool the
//! board editor cannot run yet is greyed with why, from the table Place on a
//! board greys by (`app/pcb_tools.rs`). Move and View open a dropdown on
//! right-click; dropdown rows are greyed by the same gate as their menu rows.
//! The Selection Filter opens its dropdown of object kinds on a click
//! (`filter.rs`) and carries a badge with the number of kinds that are off
//! (`badge.rs`).

use iced::alignment::Horizontal;
use iced::widget::{Space, column, container, row, stack, svg};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem, Metrics};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, MenuMetrics};

use crate::active_bar::CustomFilterPreset;
use crate::app::command::{CommandContext, is_enabled};
use crate::app::pcb_bar_groups::{PcbBarGroup, PcbBarLastTools};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, PcbBarMenu, PcbBarMsg, PcbMsg, Signex};
use crate::icons as ic;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

mod badge;
mod filter;
mod groups;

#[cfg(test)]
mod group_tests;
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
        let items = pcb_bar_items(
            tid,
            &self.ui_state.i18n,
            open,
            &self.ui_state.pcb_bar_last_tools,
            self.main_window_board_tool(),
        );
        let (offsets, bar_width) =
            signex_widgets::active_bar::slot_offsets(&items, &Metrics::GRAPHITE);
        let anchor = open.and_then(|menu| {
            let slot = items.iter().position(|item| opens(item, menu))?;
            Some((menu, offsets[slot]))
        });
        let filter_x = items
            .iter()
            .position(|item| opens(item, PcbBarMenu::Filter))
            .map(|slot| offsets[slot]);
        let bar = badge::with_filter_badge(
            signex_widgets::active_bar::view(holding_right_release(items), tokens),
            filter_x,
            badge::off_count(&self.ui_state.pcb_pick_filter),
            tokens,
            &self.ui_state.i18n,
            &self.document_state.panel_ctx.ui_font_name,
        );
        // Buttons take their own right press first. The rest of the bar
        // holds the release; with a dropdown open it counts as outside it.
        let bar_right_press = if open.is_some() {
            PcbBarMsg::FlyoutRightPressed
        } else {
            PcbBarMsg::RightPressTaken
        };
        let bar = iced::widget::mouse_area(bar)
            .on_right_press(bar_message(bar_right_press))
            .boxed();
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
        let ctx = self.command_context(None);
        let sources = MenuSources {
            ctx: &ctx,
            filter: &self.ui_state.pcb_pick_filter,
            presets: &self.ui_state.pcb_filter_presets,
            editing: self.ui_state.pcb_filter_preset_editing,
            last: &self.ui_state.pcb_bar_last_tools,
        };
        let entries = pcb_bar_menu_entries(menu, &self.ui_state.i18n, tokens, tid, &sources);
        // The Selection Filter's chip grid sizes itself, as on the schematic.
        let panel_width = (menu != PcbBarMenu::Filter).then(|| {
            MenuMetrics::GRAPHITE.fitted_width(
                &entries,
                bar_width,
                &self.document_state.panel_ctx.ui_font_name,
            )
        });
        let panel = iced::widget::mouse_area(signex_widgets::active_bar_dropdown::view(
            entries,
            tokens,
            panel_width,
        ))
        .on_right_press(bar_message(PcbBarMsg::RightPressTaken));
        // The panel stays inside the bar's width, so the centred bar does not
        // move while its dropdown is open.
        let panel_x = trigger_x
            .min(bar_width - panel_width.unwrap_or(0.0))
            .max(0.0);
        let bar_and_panel = column![
            bar,
            Space::new().height(crate::active_bar::BAR_DROPDOWN_GAP_PX),
            row![Space::new().width(panel_x), panel],
        ]
        .width(bar_width);
        let dismiss = Self::flyout_dismiss_layer(
            bar_message(PcbBarMsg::MenuDismissed),
            bar_message(PcbBarMsg::FlyoutRightPressed),
            crate::app::view::menu_scrim(),
        );
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

    /// The tool armed on the board the main window shows, whose group button
    /// the bar draws selected.
    fn main_window_board_tool(&self) -> Option<BoardPlaceTool> {
        let path = self.active_tab_path();
        self.interaction_state
            .pcb_tool
            .as_ref()
            .filter(|session| session.is_for(path.as_deref(), None))
            .map(|session| session.tool)
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

/// `items` as drawn: a right press on a dropdown button also holds its
/// release off the canvas.
fn holding_right_release(mut items: Vec<ActiveBarItem<Message>>) -> Vec<ActiveBarItem<Message>> {
    for item in &mut items {
        if let ActiveBarItem::Button(button) = item
            && let Some(Message::Pcb(PcbMsg::Bar(PcbBarMsg::MenuToggled(menu)))) =
                button.on_right_press
        {
            button.on_right_press = Some(bar_message(PcbBarMsg::MenuRightPressed(menu)));
        }
    }
    items
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
/// right-click or a click on its chevron (mockup row 108).
fn menu_button(
    command: &str,
    menu: PcbBarMenu,
    open: Option<PcbBarMenu>,
    icon: ActiveBarIcon,
    tooltip: String,
    tid: ThemeId,
) -> ActiveBarItem<Message> {
    let on_press = run(command);
    let toggle = bar_message(PcbBarMsg::MenuToggled(menu));
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip,
        enabled: on_press.is_some(),
        selected: open == Some(menu),
        on_press,
        on_right_press: Some(toggle.clone()),
        dropdown_indicator: Some(ActiveBarIcon::Svg(ic::icon_chevron_45(tid))),
        on_chevron_press: Some(toggle),
    })
}

/// A button whose click, like its right-click, opens `menu`.
fn dropdown_button(
    menu: PcbBarMenu,
    open: Option<PcbBarMenu>,
    icon: ActiveBarIcon,
    tooltip: String,
    tid: ThemeId,
) -> ActiveBarItem<Message> {
    let toggle = bar_message(PcbBarMsg::MenuToggled(menu));
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip,
        enabled: true,
        selected: open == Some(menu),
        on_press: Some(toggle.clone()),
        on_right_press: Some(toggle),
        dropdown_indicator: Some(ActiveBarIcon::Svg(ic::icon_chevron_45(tid))),
        on_chevron_press: None,
    })
}

/// The PCB Active Bar's slots in order. `open` is the open dropdown, whose
/// button is drawn selected; `last` is the row each tool group shows;
/// `armed` is the tool armed on the board, whose group is drawn selected.
pub(in crate::app::view) fn pcb_bar_items(
    tid: ThemeId,
    i18n: &I18n,
    open: Option<PcbBarMenu>,
    last: &PcbBarLastTools,
    armed: Option<BoardPlaceTool>,
) -> Vec<ActiveBarItem<Message>> {
    let svg = ActiveBarIcon::Svg;
    let group = |group: PcbBarGroup| {
        let state = groups::GroupState { open, last, armed };
        groups::group_button(group, &state, tid, i18n)
    };
    let mut items = vec![
        filter::filter_button(open, tid, i18n),
        menu_button(
            "move_object",
            PcbBarMenu::Move,
            open,
            svg(ic::icon_move(tid)),
            t!(i18n, ids::PCB_BAR_MOVE_TOOLTIP).into_string(),
            tid,
        ),
        dropdown_button(
            PcbBarMenu::Align,
            open,
            svg(ic::icon_align(tid)),
            t!(i18n, ids::PCB_BAR_ALIGN_TOOLTIP).into_string(),
            tid,
        ),
        ActiveBarItem::Separator,
    ];
    for (index, slots) in BAR_GROUPS.iter().enumerate() {
        if index > 0 {
            items.push(ActiveBarItem::Separator);
        }
        items.extend(slots.iter().map(|&slot| group(slot)));
    }
    items.extend([
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
            svg(ic::icon_dd_rotate_cw(tid)),
            t!(i18n, ids::PCB_VIEW_ROTATE_TOOLTIP).into_string(),
            tid,
        ),
    ]);
    items
}

/// The tool groups between the Align button and Board Sketch, a separator
/// between each run: what goes on the board, the copper (routing, tuning,
/// vias and pads, areas), then what annotates it (mockup row 108). Copper
/// lines and arcs are drawn with the Graphics tools (#2869).
const BAR_GROUPS: [&[PcbBarGroup]; 3] = [
    &[PcbBarGroup::Components],
    &[
        PcbBarGroup::Route,
        PcbBarGroup::Tune,
        PcbBarGroup::ViaPad,
        PcbBarGroup::CopperAreas,
    ],
    &[
        PcbBarGroup::Dimensions,
        PcbBarGroup::Text,
        PcbBarGroup::Graphics,
    ],
];

/// The bar icon of `tool`: the footprint editor's where one fits, the board
/// tool glyphs for pad, via, dimension and keepout, the hatched area for the
/// copper-area tools (mockup row 108).
fn tool_icon(tool: BoardPlaceTool, tid: ThemeId) -> ActiveBarIcon {
    use BoardPlaceTool as T;
    let svg = ActiveBarIcon::Svg;
    match tool {
        T::Component => svg(ic::icon_component(tid)),
        T::Pad => svg(ic::icon_pcb_pad(tid)),
        T::Via => svg(ic::icon_pcb_via(tid)),
        T::InteractiveRouting => svg(ic::icon_wire(tid)),
        T::DiffPairRouting => svg(ic::icon_pcb_diff_pair_route(tid)),
        T::QuickRoute => svg(ic::icon_pcb_quick_route(tid)),
        T::MultiRoute => svg(ic::icon_pcb_multi_route(tid)),
        T::Track => svg(ic::icon_shape_line(tid)),
        T::Arc => svg(ic::icon_shape_arc(tid)),
        T::Fill | T::SolidRegion | T::PolygonPour | T::PourCutout | T::SlicePour => {
            svg(ic::icon_pcb_copper_area(tid))
        }
        T::TextString => svg(ic::icon_text(tid)),
        T::Dimension(_) => svg(ic::icon_pcb_dimension(tid)),
        T::Keepout => svg(ic::icon_pcb_keepout(tid)),
        T::Room => ActiveBarIcon::Glyph("\u{2B1A}"),
        T::Line => svg(ic::icon_shape_line(tid)),
        T::GraphicArc => svg(ic::icon_shape_arc(tid)),
        T::FullCircle => svg(ic::icon_shape_circle(tid)),
        T::Rectangle => svg(ic::icon_shape_rect(tid)),
        T::TextFrame => svg(ic::icon_dd_text_frame(tid)),
    }
}

/// A dropdown row: the command it runs and its icon, if it has one.
type MenuRow = (&'static str, Option<fn(ThemeId) -> svg::Handle>);

/// The Move dropdown: the board placement commands of Edit ▸ Move.
const MOVE_ROWS: &[MenuRow] = &[
    ("move_object", Some(ic::icon_dd_move)),
    ("move_selection", Some(ic::icon_dd_move_sel)),
    ("rotate_counterclockwise", Some(ic::icon_dd_rotate)),
    ("rotate_clockwise", Some(ic::icon_dd_rotate_cw)),
    ("flip_selection", Some(ic::icon_dd_flip_x)),
];

/// The Align dropdown: the Align rows, which line footprints up on a board.
const ALIGN_ROWS: &[MenuRow] = &[
    ("align_left", Some(ic::icon_dd_align_left)),
    ("align_right", Some(ic::icon_dd_align_right)),
    ("align_horizontal_centers", Some(ic::icon_dd_align_hcenter)),
    ("distribute_horizontally", Some(ic::icon_dd_dist_horiz)),
    ("align_top", Some(ic::icon_dd_align_top)),
    ("align_bottom", Some(ic::icon_dd_align_bottom)),
    ("align_vertical_centers", Some(ic::icon_dd_align_vcenter)),
    ("distribute_vertically", Some(ic::icon_dd_dist_vert)),
    ("align_to_grid", Some(ic::icon_dd_align_grid)),
];

/// The View dropdown: the board view turns of the View menu. Each turn's
/// arrow points the way the row turns (mockup row 108); a half turn is drawn
/// clockwise, and Reset has no icon.
const VIEW_ROWS: &[MenuRow] = &[
    ("view_rotate", Some(ic::icon_dd_rotate_cw)),
    ("view_rotate_ccw", Some(ic::icon_dd_rotate)),
    ("view_rotate_180", Some(ic::icon_dd_rotate_cw)),
    ("view_rotate_reset", None),
];

fn menu_commands(menu: PcbBarMenu) -> &'static [MenuRow] {
    match menu {
        PcbBarMenu::Move => MOVE_ROWS,
        PcbBarMenu::View => VIEW_ROWS,
        PcbBarMenu::Align => ALIGN_ROWS,
        PcbBarMenu::Filter | PcbBarMenu::Group(_) => &[],
    }
}

/// What the bar's dropdowns are built from, besides the theme.
pub(in crate::app::view) struct MenuSources<'a> {
    /// The menu bar's gate, which greys a command row.
    pub ctx: &'a CommandContext,
    /// The Selection Filter the filter dropdown edits.
    pub filter: &'a pcb_geom::PickFilter,
    /// The saved filter presets.
    pub presets: &'a [CustomFilterPreset<pcb_geom::PickKind>],
    /// The preset selected for editing (#2873).
    pub editing: Option<usize>,
    /// The row each tool group used last, drawn current in its dropdown.
    pub last: &'a PcbBarLastTools,
}

/// The rows of `menu`: each runs its command, labelled and greyed as its
/// menu row is in `src.ctx`. The Selection Filter's rows toggle the kinds of
/// `src.filter` instead, and a tool group's rows come from its table.
pub(in crate::app::view) fn pcb_bar_menu_entries(
    menu: PcbBarMenu,
    i18n: &I18n,
    tokens: &ThemeTokens,
    tid: ThemeId,
    src: &MenuSources<'_>,
) -> Vec<DropdownEntry<Message>> {
    match menu {
        PcbBarMenu::Filter => {
            return filter::entries(i18n, tokens, src.filter, src.presets, src.editing);
        }
        PcbBarMenu::Group(group) => return groups::entries(group, i18n, tid, src.ctx, src.last),
        PcbBarMenu::Align | PcbBarMenu::Move | PcbBarMenu::View => {}
    }
    menu_commands(menu)
        .iter()
        .filter_map(|&(id, icon)| {
            let command = AppCommandId::new(id).ok()?;
            let enabled = is_enabled(&command, src.ctx);
            let label = crate::menu_bar::cmd_label(i18n, id, id);
            let pick = bar_message(PcbBarMsg::CommandPicked(command));
            let mut row = DropdownItem::new(label, pick)
                .command(id)
                .disabled(!enabled);
            if let Some(icon) = icon {
                row = row.icon(icon(tid));
            }
            Some(DropdownEntry::Item(row))
        })
        .collect()
}
