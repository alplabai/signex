//! Secondary context submenu body (Place / Align / Add New to Project)
//! shown to the right of the canvas and project-tree context menus.
//!
//! Data-to-view (#269): each submenu is a pure `Vec<DropdownEntry<Message>>`
//! rendered by the shared `signex_widgets::active_bar_dropdown` widget, so
//! the flyout shares its row chrome with the parent menus and the active
//! bars (ADR-0003). Every Place / Align / Arrange row that has a command id
//! dispatches that id through the command registry, like the palette and the
//! keymap; only the power-port rows, which have none, send
//! `ContextAction::ActiveBar(...)`.

use super::*;

use super::items::{
    command_text, dd_cmd_disabled, dd_command, dd_disabled, dd_kb, dd_msg, dd_planned,
    dd_planned_because, dd_planned_pill, dd_ruled, tag_command, text,
};
use super::menus::CanvasSelection;
use crate::active_bar::ActiveBarAction as A;
use crate::app::command::active_bar::id_for_action;
use crate::icons as ic;
use signex_i18n::{I18n, MessageId, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// A flyout row for an Active Bar action. With a command id the label and
/// shortcut `hint` come from the catalog (#271); the literal `fallback` is
/// only for an action that has no id. A disabled row keeps its hint in the
/// right-hand column.
fn active_bar_row(
    i18n: &I18n,
    icon: iced::widget::svg::Handle,
    fallback: Option<MessageId>,
    action: crate::active_bar::ActiveBarAction,
    enabled: bool,
    planned: bool,
    hint: &dyn Fn(&str) -> String,
) -> DropdownEntry<Message> {
    let Some(id) = id_for_action(&action) else {
        let fallback_text = fallback.map_or_else(String::new, |id| text(i18n, id));
        return if !enabled {
            dd_disabled(Some(icon), &fallback_text, None)
        } else if planned {
            dd_planned(i18n, Some(icon), &fallback_text, None)
        } else {
            dd_kb(
                Some(icon),
                &fallback_text,
                "",
                ContextAction::ActiveBar(action),
            )
        };
    };
    let shortcut = hint(id);
    let right = (!shortcut.is_empty()).then_some(shortcut.as_str());
    if planned {
        // A planned tool is shown but cannot be armed (#1249).
        tag_command(
            dd_planned(i18n, Some(icon), &command_text(i18n, id), right),
            id,
        )
    } else if enabled {
        dd_command(i18n, Some(icon), id, &shortcut, None)
    } else {
        dd_cmd_disabled(i18n, Some(icon), id, &shortcut)
    }
}

/// One row of the Place flyout's table.
enum PlaceRow {
    /// A subhead over the rows that follow.
    Head(MessageId),
    /// A tool, labelled by its command (or `MessageId` when it has none).
    Tool(
        fn(ThemeId) -> iced::widget::svg::Handle,
        crate::active_bar::ActiveBarAction,
    ),
    /// A power port: its port glyph in the power colour, a short label, and
    /// the full name as its tooltip.
    Power(
        fn(ThemeId) -> iced::widget::svg::Handle,
        MessageId,
        MessageId,
        crate::active_bar::ActiveBarAction,
    ),
}

/// The Place flyout (Graphite mockup #62), under the Wiring / Ports and power / Parts and
/// sheets / Directives and text subheads. A planned tool is not listed in
/// its group: the planned ones are gathered in one greyed row at the end.
const PLACE_ROWS: &[PlaceRow] = &[
    PlaceRow::Head(menu_ids::CTX_PLACE_GROUP_WIRING),
    PlaceRow::Tool(ic::icon_dd_wire, A::DrawWire),
    PlaceRow::Tool(ic::icon_dd_bus, A::DrawBus),
    PlaceRow::Tool(ic::icon_dd_bus_entry, A::PlaceBusEntry),
    PlaceRow::Tool(ic::icon_dd_net_label, A::PlaceNetLabel),
    PlaceRow::Tool(ic::icon_dd_junction, A::PlaceJunction),
    PlaceRow::Head(menu_ids::CTX_PLACE_GROUP_PORTS_POWER),
    PlaceRow::Tool(ic::icon_dd_port, A::PlacePort),
    PlaceRow::Tool(ic::icon_dd_off_sheet, A::PlaceOffSheetConnector),
    PlaceRow::Power(
        ic::icon_dd_gnd,
        menu_ids::CTX_POWER_GND,
        menu_ids::CTX_PLACE_POWER_GND,
        A::PlacePowerGND,
    ),
    PlaceRow::Power(
        ic::icon_dd_vcc,
        menu_ids::CTX_POWER_VCC,
        menu_ids::CTX_PLACE_POWER_VCC,
        A::PlacePowerVCC,
    ),
    PlaceRow::Power(
        ic::icon_dd_pwr_plus5,
        menu_ids::CTX_POWER_PLUS5,
        menu_ids::CTX_PLACE_POWER_PLUS5,
        A::PlacePowerPlus5,
    ),
    PlaceRow::Power(
        ic::icon_dd_pwr_plus12,
        menu_ids::CTX_POWER_PLUS12,
        menu_ids::CTX_PLACE_POWER_PLUS12,
        A::PlacePowerPlus12,
    ),
    PlaceRow::Tool(ic::icon_dd_harness, A::PlaceSignalHarness),
    PlaceRow::Tool(ic::icon_dd_harness_conn, A::PlaceHarnessConnector),
    PlaceRow::Tool(ic::icon_dd_harness_entry, A::PlaceHarnessEntry),
    PlaceRow::Head(menu_ids::CTX_PLACE_GROUP_PARTS_SHEETS),
    PlaceRow::Tool(ic::icon_component, A::PlaceComponent),
    PlaceRow::Tool(ic::icon_dd_sheet_symbol, A::PlaceSheetSymbol),
    PlaceRow::Tool(ic::icon_dd_sheet_entry, A::PlaceSheetEntry),
    PlaceRow::Tool(ic::icon_dd_device_sheet, A::PlaceDeviceSheetSymbol),
    PlaceRow::Head(menu_ids::CTX_PLACE_GROUP_DIRECTIVES_TEXT),
    PlaceRow::Tool(ic::icon_dd_param_set, A::PlaceParameterSet),
    PlaceRow::Tool(ic::icon_dd_no_erc, A::PlaceNoERC),
    PlaceRow::Tool(ic::icon_dd_diff_pair, A::PlaceDiffPair),
    PlaceRow::Tool(ic::icon_dd_blanket, A::PlaceBlanket),
    PlaceRow::Tool(ic::icon_dd_blanket, A::PlaceCompileMask),
    PlaceRow::Tool(ic::icon_dd_text_string, A::PlaceTextString),
    PlaceRow::Tool(ic::icon_dd_text_frame, A::PlaceTextFrame),
    PlaceRow::Tool(ic::icon_dd_note, A::PlaceNote),
];

/// Place submenu (Graphite mockup #62): wiring, ports and power, parts and sheets,
/// directives and text, each under its subhead; then one greyed "N planned
/// tools…" row naming the planned ones in its tooltip. `hint` resolves a
/// command id to its keymap shortcut.
pub(super) fn place_entries(
    i18n: &I18n,
    tid: ThemeId,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    let ink = signex_widgets::theme_ext::power_port(&signex_types::theme::canvas_colors(tid));
    let mut planned: Vec<&'static str> = Vec::new();
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(PLACE_ROWS.len() + 2);
    for row in PLACE_ROWS {
        match row {
            PlaceRow::Head(id) => v.push(DropdownEntry::Header(text(i18n, *id))),
            PlaceRow::Tool(icon, action) => match crate::active_bar::planned_name(action) {
                Some(name) => planned.push(name),
                None => v.push(active_bar_row(
                    i18n,
                    icon(tid),
                    None,
                    action.clone(),
                    true,
                    false,
                    hint,
                )),
            },
            PlaceRow::Power(icon, short, full, action) => {
                let row = active_bar_row(
                    i18n,
                    icon(tid),
                    Some(*full),
                    action.clone(),
                    true,
                    false,
                    hint,
                );
                v.push(match row {
                    DropdownEntry::Item(mut item) => {
                        item.label = text(i18n, *short);
                        DropdownEntry::Item(item.icon_tint(ink).tooltip(text(i18n, *full)))
                    }
                    other => other,
                });
            }
        }
    }
    if !planned.is_empty() {
        v.push(DropdownEntry::Separator);
        v.push(planned_tools_row(i18n, &planned));
    }
    v
}

/// The greyed "N planned tools…" row; its tooltip names them.
fn planned_tools_row(i18n: &I18n, names: &[&str]) -> DropdownEntry<Message> {
    let label = i18n.message_plain(
        menu_ids::CTX_PLACE_PLANNED_TOOLS,
        &[("count", (names.len() as i64).into())],
    );
    let why = i18n.message_plain(
        menu_ids::CTX_PLACE_PLANNED_TOOLS_TOOLTIP,
        &[("names", i18n.list(names, signex_i18n::ListKind::And).into())],
    );
    match dd_disabled(None, &label, None) {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.tooltip(why)),
        other => other,
    }
}

/// The "Planned (N)" flyout of the sheet menu (Graphite mockup #62): the features not built
/// yet, greyed, each with what it will do as its tooltip.
pub(super) fn planned_entries(i18n: &I18n, tid: ThemeId) -> Vec<DropdownEntry<Message>> {
    vec![
        dd_planned_because(
            i18n,
            Some(ic::icon_dd_references(tid)),
            menu_ids::CTX_REFERENCES,
            menu_ids::CTX_REFERENCES_PLANNED,
            false,
        ),
        dd_planned(
            i18n,
            Some(ic::icon_dd_snippets(tid)),
            &text(i18n, menu_ids::CTX_SNIPPETS),
            None,
        ),
        dd_planned(
            i18n,
            Some(ic::icon_dd_comment(tid)),
            &text(i18n, menu_ids::CTX_COMMENT),
            None,
        ),
        dd_planned_because(
            i18n,
            Some(ic::icon_dd_pin_mapping(tid)),
            menu_ids::CTX_PIN_MAPPING,
            menu_ids::CTX_PIN_MAPPING_PLANNED,
            false,
        ),
        dd_planned_because(
            i18n,
            Some(ic::icon_dd_supplier_links(tid)),
            menu_ids::CTX_SUPPLIER_LINKS,
            menu_ids::CTX_SUPPLIER_LINKS_PLANNED,
            false,
        ),
    ]
}

/// Place on a board (#2405): the board placement tools in the order of the
/// menu bar's Place, each running its command and live when the command is
/// enabled in `ctx` (greyed on a read-only board), or greyed with the reason
/// the PCB Active Bar gives while the board editor cannot run it; then Board
/// Sketch.
pub(super) fn board_place_entries(
    i18n: &I18n,
    ctx: &crate::app::command::CommandContext,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    use crate::app::pcb_tools::{BoardPlaceTool, ToolState};
    let command = |id: &'static str| {
        let live = crate::keymap::AppCommandId::new(id)
            .is_ok_and(|command| crate::app::command::is_enabled(&command, ctx));
        if live {
            dd_command(i18n, None, id, &hint(id), None)
        } else {
            dd_cmd_disabled(i18n, None, id, &hint(id))
        }
    };
    let mut v: Vec<DropdownEntry<Message>> = BoardPlaceTool::PLACE_MENU
        .into_iter()
        .map(|tool| match tool.state() {
            ToolState::Live(id) => command(id),
            ToolState::Unavailable(reason) => {
                match dd_disabled(None, &text(i18n, tool.name()), None) {
                    DropdownEntry::Item(item) => {
                        DropdownEntry::Item(item.tooltip(text(i18n, reason)))
                    }
                    other => other,
                }
            }
        })
        .collect();
    v.push(DropdownEntry::Separator);
    v.push(command("board_sketch"));
    v
}

/// Altium gating for the Align submenu: pairwise aligns
/// (Left/Right/Top/Bottom/H/V Centers) need ≥2 items to make sense;
/// Distribute needs ≥3 (two endpoints + at least one item to space
/// between them). Returns `(pairwise_enabled, distribute_enabled)`.
/// Align To Grid works on a single item so it is always enabled.
pub(super) fn align_gate(selected: usize) -> (bool, bool) {
    (selected >= 2, selected >= 3)
}

/// Align submenu — pairwise aligns + distribute (gated by selection
/// count) and the always-on Align To Grid.
pub(super) fn align_entries(
    i18n: &I18n,
    tid: ThemeId,
    selected: usize,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    use crate::active_bar::ActiveBarAction as A;
    let (pair, dist) = align_gate(selected);
    let row = |icon, label: Option<MessageId>, action: A, enabled: bool| {
        active_bar_row(i18n, icon, label, action, enabled, false, hint)
    };
    vec![
        row(ic::icon_dd_align_left(tid), None, A::AlignLeft, pair),
        row(ic::icon_dd_align_right(tid), None, A::AlignRight, pair),
        row(
            ic::icon_dd_align_hcenter(tid),
            None,
            A::AlignHorizontalCenters,
            pair,
        ),
        row(
            ic::icon_dd_dist_horiz(tid),
            None,
            A::DistributeHorizontally,
            dist,
        ),
        DropdownEntry::Separator,
        row(ic::icon_dd_align_top(tid), None, A::AlignTop, pair),
        row(ic::icon_dd_align_bottom(tid), None, A::AlignBottom, pair),
        row(
            ic::icon_dd_align_vcenter(tid),
            None,
            A::AlignVerticalCenters,
            pair,
        ),
        row(
            ic::icon_dd_dist_vert(tid),
            None,
            A::DistributeVertically,
            dist,
        ),
        DropdownEntry::Separator,
        row(ic::icon_dd_align_grid(tid), None, A::AlignToGrid, true),
    ]
}

/// Arrange submenu — order the selected drawings (#1583). Offered only while
/// the selection holds a drawing, so every row is live.
pub(super) fn arrange_entries(
    i18n: &I18n,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    [
        "bring_drawings_to_front",
        "send_drawings_to_back",
        "bring_drawings_forward",
        "send_drawings_backward",
    ]
    .into_iter()
    .map(|id| dd_command(i18n, None, id, &hint(id), None))
    .collect()
}

/// Part Actions submenu (#2419) — the placed part's library actions, then
/// Autoplace Fields and Copy Attributes (#1667), live when one placed part is
/// selected and the command's own rule allows it.
pub(super) fn part_actions_entries(
    i18n: &I18n,
    selection: CanvasSelection,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    let row = |id: &'static str| {
        dd_ruled(
            i18n,
            None,
            id,
            &hint(id),
            &selection.commands,
            selection.placed_part_selected,
        )
    };
    vec![
        row("open_datasheet"),
        row("edit_selected_symbol_in_symbol_editor"),
        row("change_component"),
        DropdownEntry::Separator,
        row("autoplace_fields"),
        row("copy_attributes_or_add_vertex"),
    ]
}

/// Sheet Actions submenu (#2419) — the selected sheet symbol's child-sheet
/// actions (live with a sheet symbol selected), then the project's sheet
/// numbering, each also gated by the command's own rule.
pub(super) fn sheet_actions_entries(
    i18n: &I18n,
    tid: ThemeId,
    selection: CanvasSelection,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    let row = |icon: Option<iced::widget::svg::Handle>, id: &'static str, applies: bool| {
        dd_ruled(i18n, icon, id, &hint(id), &selection.commands, applies)
    };
    let child = selection.child_sheet_selected;
    vec![
        row(
            Some(ic::icon_dd_open_child_sheet(tid)),
            "open_child_sheet",
            child,
        ),
        row(None, "create_sheet_from_sheet_symbol", child),
        row(None, "sync_sheet_pins", child),
        // #1276: the sync with a preview, then a new symbol from a sheet file.
        row(None, "sync_sheet_entries", child),
        DropdownEntry::Separator,
        row(None, "create_sheet_symbol_from_sheet", true),
        row(None, "number_schematic_sheets", true),
    ]
}

/// Add New to Project submenu — the "Add New" picker for the right-clicked
/// project (resolved as `target`), grouped as in the tree menu (mockup row 20): the
/// design documents (Schematic, PCB), the libraries, then the project files
/// whose tabs are not built yet (Design Notebook, Output Job) as planned rows.
pub(super) fn add_new_entries(
    i18n: &I18n,
    tid: ThemeId,
    target: signex_widgets::tree_view::TreeKey,
) -> Vec<DropdownEntry<Message>> {
    use crate::app::ProjectTreeAction as P;
    let tree = |action: P| Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(action));
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(13);

    v.push(dd_msg(
        Some(ic::icon_menu_sheet(tid)),
        &text(i18n, menu_ids::CTX_ADD_SCHEMATIC),
        "",
        tree(P::AddNewSchematic(target.clone())),
    ));
    // The project's one board, opened unsaved (#1312).
    v.push(dd_msg(
        Some(ic::icon_menu_board(tid)),
        &text(i18n, menu_ids::CTX_ADD_PCB),
        "",
        tree(P::AddNewPcb(target.clone())),
    ));
    v.push(DropdownEntry::Separator);
    // A component library (the part table), wired through the menu bridge.
    v.push(dd_msg(
        Some(ic::icon_menu_library(tid)),
        &text(i18n, menu_ids::CTX_ADD_COMPONENT_LIBRARY),
        "",
        Message::Menu(crate::menu_bar::MenuMessage::AddComponentLibrary),
    ));
    // A `.snxsym` is a project document of its own: Save-As in the project
    // folder.
    v.push(dd_msg(
        Some(ic::icon_menu_library(tid)),
        &text(i18n, menu_ids::CTX_ADD_SYMBOL_LIBRARY),
        "",
        tree(P::AddProjectSymbolLibrary(target.clone())),
    ));
    // Hidden while the footprint editor is gated off for release.
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        v.push(dd_msg(
            Some(ic::icon_menu_library(tid)),
            &text(i18n, menu_ids::CTX_ADD_PCB_LIBRARY),
            "",
            tree(P::AddProjectFootprintLibrary(target)),
        ));
    }
    v.push(DropdownEntry::Separator);
    v.push(dd_planned_pill(
        i18n,
        Some(ic::icon_menu_notebook(tid)),
        &text(i18n, menu_ids::CTX_ADD_DESIGN_NOTEBOOK),
    ));
    v.push(dd_planned_pill(
        i18n,
        Some(ic::icon_menu_export(tid)),
        &text(i18n, menu_ids::CTX_ADD_OUTPUT_JOB),
    ));
    v.push(DropdownEntry::Separator);
    v.push(dd_planned_pill(
        i18n,
        Some(ic::icon_menu_file(tid)),
        &text(i18n, menu_ids::CTX_ADD_CONSTRAINT_FILE),
    ));
    v.push(dd_planned_pill(
        i18n,
        Some(ic::icon_menu_file(tid)),
        &text(i18n, menu_ids::CTX_ADD_VHDL_FILE),
    ));

    v
}

impl Signex {
    /// Build the secondary submenu (Place / Align / Arrange / Part Actions /
    /// Sheet Actions / Add New to Project / Planned / a library's Add New)
    /// shown to the right of the parent
    /// context menu. The selection is the one of the sheet `window_id` shows
    /// (#1910); the target project is resolved from `self`. The pure entry
    /// builders do the rest and the shared widget renders it.
    pub(in crate::app::view) fn view_context_submenu(
        &self,
        kind: ContextSubmenu,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        let tid = self.ui_state.effective_theme_id();
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let hint = |id: &str| self.keymap_shortcut_label(id);
        let entries = match kind {
            ContextSubmenu::Place => place_entries(i18n, tid, &hint),
            ContextSubmenu::Align => {
                let selected = self
                    .interaction_state
                    .canvas_for_window(window_id)
                    .selected
                    .len();
                align_entries(i18n, tid, selected, &hint)
            }
            ContextSubmenu::Arrange => arrange_entries(i18n, &hint),
            ContextSubmenu::PartActions => {
                part_actions_entries(i18n, self.canvas_menu_inputs(window_id).0, &hint)
            }
            ContextSubmenu::SheetActions => {
                sheet_actions_entries(i18n, tid, self.canvas_menu_inputs(window_id).0, &hint)
            }
            ContextSubmenu::Unions => {
                super::menus::unions_entries(i18n, self.canvas_menu_inputs(window_id).0, &hint)
            }
            ContextSubmenu::AddNewToProject => {
                // The right-clicked project lives at `tree_path[0]` —
                // resolve it off the captured menu state so the action
                // targets the right project even when another is active.
                let target = self
                    .interaction_state
                    .project_tree_context_menu
                    .as_ref()
                    .and_then(|m| m.key.clone())
                    .unwrap_or_default();
                add_new_entries(i18n, tid, target)
            }
            ContextSubmenu::Planned => planned_entries(i18n, tid),
            ContextSubmenu::ProjectPlanned => {
                super::project_tree::planned::planned_entries(i18n, tid)
            }
            ContextSubmenu::AddNewToLibrary => {
                super::project_tree::library::library_add_new_entries(i18n, tid)
            }
        };
        super::placement::menu_card(entries, tokens, &self.document_state.panel_ctx.ui_font_name)
            .element
    }

    /// The flyout of the board menu opened in `window_id` (#2405): Place
    /// lists the board's tools; Align lines up the selected footprints, live
    /// for an unlocked selection on an editable board.
    pub(in crate::app::view) fn view_board_context_submenu(
        &self,
        kind: ContextSubmenu,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        let i18n = &self.ui_state.i18n;
        let tid = self.ui_state.effective_theme_id();
        let context = self.command_context(Some(window_id));
        let hint = |id: &str| self.keymap_shortcut_label(id);
        let entries = match kind {
            ContextSubmenu::Align => {
                let footprints = if context.pcb.placeable {
                    context.pcb.footprints
                } else {
                    0
                };
                align_entries(i18n, tid, footprints, &hint)
            }
            _ => board_place_entries(i18n, &context, &hint),
        };
        let panel_ctx = &self.document_state.panel_ctx;
        super::placement::menu_card(entries, &panel_ctx.tokens, &panel_ctx.ui_font_name).element
    }
}
