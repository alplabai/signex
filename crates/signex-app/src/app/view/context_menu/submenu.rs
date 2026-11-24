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
use super::project_tree::kind_inks::{KindInks, kind_tinted};
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

/// In variant submenu (mockup row 62, the rows of mockup row 59 b): headed
/// "<part> in <variant>" for one selected part. Changing a part's fit, part
/// or value in one variant is not built, so those rows are greyed with what
/// they will do; Show in Variant Manager and New variant… open the
/// project's Variant Manager.
pub(super) fn in_variant_entries(
    i18n: &I18n,
    variant: &super::menus::MenuVariant,
    part: Option<&str>,
) -> Vec<DropdownEntry<Message>> {
    let planned =
        |label| dd_planned_because(i18n, None, label, menu_ids::CTX_IN_VARIANT_PLANNED, false);
    let mut v = Vec::with_capacity(8);
    if let Some(part) = part {
        v.push(DropdownEntry::Header(i18n.message_plain(
            menu_ids::CTX_IN_VARIANT_PART,
            &[
                ("part", part.to_string().into()),
                ("variant", variant.name.clone().into()),
            ],
        )));
    }
    v.extend([
        planned(signex_i18n::ids::PROJECT_VARIANTS_FITTED),
        planned(signex_i18n::ids::PROJECT_VARIANTS_NOT_FITTED),
        planned(menu_ids::CTX_IN_VARIANT_ALTERNATE_PART),
        planned(menu_ids::CTX_IN_VARIANT_EDIT_VALUE),
        DropdownEntry::Separator,
        dd_kb(
            None,
            &text(i18n, menu_ids::CTX_IN_VARIANT_SHOW_MANAGER),
            "",
            ContextAction::ShowInVariantManager(variant.project_id),
        ),
        dd_kb(
            None,
            &text(i18n, menu_ids::CTX_IN_VARIANT_NEW),
            "",
            ContextAction::NewVariant(variant.project_id),
        ),
    ]);
    v
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
    let file_missing = child && selection.child_sheet_file_missing;
    let open = row(
        Some(ic::icon_dd_open_child_sheet(tid)),
        "open_child_sheet",
        child && !file_missing,
    );
    let open = match open {
        DropdownEntry::Item(item) if file_missing => DropdownEntry::Item(
            item.tooltip(text(i18n, signex_i18n::ids::PROPERTIES_SHEET_OPEN_MISSING)),
        ),
        other => other,
    };
    vec![
        open,
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
/// whose tabs are not built yet (Design Notebook, Export Job) as planned rows.
/// The live rows' glyphs take their file kind's `inks`.
pub(super) fn add_new_entries(
    i18n: &I18n,
    tid: ThemeId,
    inks: KindInks,
    target: signex_widgets::tree_view::TreeKey,
) -> Vec<DropdownEntry<Message>> {
    use crate::app::ProjectTreeAction as P;
    let tree = |action: P| Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(action));
    let library = |label, message| {
        let row = dd_msg(
            Some(ic::icon_menu_library(tid)),
            &text(i18n, label),
            "",
            message,
        );
        kind_tinted(row, inks.library)
    };
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(13);

    v.push(kind_tinted(
        dd_msg(
            Some(ic::icon_menu_sheet(tid)),
            &text(i18n, menu_ids::CTX_ADD_SCHEMATIC),
            "",
            tree(P::AddNewSchematic(target.clone())),
        ),
        inks.schematic,
    ));
    // The project's one board, opened unsaved (#1312).
    v.push(kind_tinted(
        dd_msg(
            Some(ic::icon_menu_board(tid)),
            &text(i18n, menu_ids::CTX_ADD_PCB),
            "",
            tree(P::AddNewPcb(target.clone())),
        ),
        inks.pcb,
    ));
    v.push(DropdownEntry::Separator);
    // A component library (the part table), wired through the menu bridge.
    v.push(library(
        menu_ids::CTX_ADD_COMPONENT_LIBRARY,
        Message::Menu(crate::menu_bar::MenuMessage::AddComponentLibrary),
    ));
    // A `.snxsym` is a project document of its own: Save-As in the project
    // folder.
    v.push(library(
        menu_ids::CTX_ADD_SYMBOL_LIBRARY,
        tree(P::AddProjectSymbolLibrary(target.clone())),
    ));
    // Hidden while the footprint editor is gated off for release.
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        v.push(library(
            menu_ids::CTX_ADD_PCB_LIBRARY,
            tree(P::AddProjectFootprintLibrary(target)),
        ));
    }
    v.extend(add_new_planned_entries(i18n, tid));
    v
}

/// The planned tail of Add New to Project: Design Notebook and Export Job,
/// then Constraint File and VHDL File, each greyed with the pill.
fn add_new_planned_entries(i18n: &I18n, tid: ThemeId) -> [DropdownEntry<Message>; 6] {
    let planned = |icon, label| dd_planned_pill(i18n, Some(icon), &text(i18n, label));
    [
        DropdownEntry::Separator,
        planned(
            ic::icon_menu_notebook(tid),
            menu_ids::CTX_ADD_DESIGN_NOTEBOOK,
        ),
        planned(ic::icon_menu_export(tid), menu_ids::CTX_ADD_OUTPUT_JOB),
        DropdownEntry::Separator,
        planned(ic::icon_menu_file(tid), menu_ids::CTX_ADD_CONSTRAINT_FILE),
        planned(ic::icon_menu_file(tid), menu_ids::CTX_ADD_VHDL_FILE),
    ]
}

impl Signex {
    /// Build the secondary submenu (Place / Align / Arrange / Part Actions /
    /// Sheet Actions / In variant / Add New to Project / Planned / a
    /// library's Add New)
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
                add_new_entries(i18n, tid, KindInks::of(tokens), target)
            }
            ContextSubmenu::Planned => planned_entries(i18n, tid),
            ContextSubmenu::ProjectPlanned => {
                super::project_tree::planned::planned_entries(i18n, tid)
            }
            ContextSubmenu::AddNewToLibrary => {
                super::project_tree::library::library_add_new_entries(
                    i18n,
                    tid,
                    KindInks::of(tokens),
                )
            }
            ContextSubmenu::InVariant => match self.canvas_menu_variant(window_id) {
                Some(variant) => {
                    let part = match self.canvas_menu_context(window_id) {
                        super::menus::MenuContext::Selection {
                            part: Some((reference, _)),
                            ..
                        } => Some(reference),
                        _ => None,
                    };
                    in_variant_entries(i18n, &variant, part.as_deref())
                }
                None => Vec::new(),
            },
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

/// Mockup row 62: "In variant: <name> ›" after Part Actions when the
/// project has a variant besides Base, and its flyout.
#[cfg(test)]
mod tests {
    use super::super::menus::{
        CanvasSelection, CanvasShortcuts, MenuContext, MenuVariant, canvas_menu_with_variant,
    };
    use super::*;
    use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
    use crate::app::{ProjectSection, TabInfo, TabKind};
    use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};

    const TID: ThemeId = ThemeId::GraphiteDark;

    /// Each entry as one line: an item's label, `# header`, `---`.
    fn layout(entries: &[DropdownEntry<Message>]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| match entry {
                DropdownEntry::Item(item) => item.label.clone(),
                DropdownEntry::Header(header) => format!("# {header}"),
                _ => "---".to_string(),
            })
            .collect()
    }

    fn item<'a>(
        entries: &'a [DropdownEntry<Message>],
        label: &str,
    ) -> &'a signex_widgets::active_bar_dropdown::DropdownItem<Message> {
        entries
            .iter()
            .find_map(|entry| match entry {
                DropdownEntry::Item(item) if item.label == label => Some(item),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no row {label:?} in {:?}", layout(entries)))
    }

    fn selected() -> CanvasSelection {
        CanvasSelection {
            has_selection: true,
            placed_part_selected: true,
            commands: crate::app::command::CommandContext {
                has_schematic: true,
                has_selection: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// The selection menu's rows for `variant`.
    fn selection_menu(variant: Option<&MenuVariant>) -> Vec<String> {
        layout(&canvas_menu_with_variant(
            I18n::en_us_ref(),
            TID,
            selected(),
            None,
            None,
            &CanvasShortcuts::default(),
            variant,
        ))
    }

    /// The rows from Part Actions to Place.
    fn flyout_block(rows: &[String]) -> Vec<String> {
        let from = rows
            .iter()
            .position(|r| r == "Part Actions")
            .expect("Part Actions");
        let to = rows.iter().position(|r| r == "Place").expect("Place");
        rows[from..=to].to_vec()
    }

    /// A project whose one sheet, open in the active tab, places R5 (31.6k),
    /// which is selected; the project defines `definitions` with `active`
    /// active. The folder lives as long as the returned guard.
    fn app_with(
        definitions: &[&str],
        active: Option<&str>,
    ) -> (Signex, crate::app::ProjectId, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("a folder");
        let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
        let path = dir.path().join("a.snxsch");
        let part = uuid::Uuid::from_u128(5);
        let sheet = SchematicSheet {
            symbols: vec![Symbol {
                uuid: part,
                lib_id: "Device:R".to_string(),
                reference: "R5".to_string(),
                value: "31.6k".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        open_with(&mut app.document_state, &path, sheet);
        let id = app.document_state.projects[0].id;
        app.document_state.tabs.push(TabInfo {
            title: "a".to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: Some(id),
            kind: TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state.active_path = Some(path);
        let data = &mut app.document_state.projects[0].data;
        data.variant_definitions = definitions.iter().map(|d| d.to_string()).collect();
        data.active_variant = active.map(str::to_string);
        app.interaction_state.active_canvas_mut().selected =
            vec![SelectedItem::new(part, SelectedKind::Symbol)];
        (app, id, dir)
    }

    /// The variant the main window's menu names.
    fn main_variant(app: &Signex) -> Option<MenuVariant> {
        let window = app.ui_state.main_window_id.expect("main window");
        app.canvas_menu_variant(window)
    }

    fn lite(app: &Signex) -> MenuVariant {
        main_variant(app).expect("the project has a variant")
    }

    /// A second project, B, in its own folder, defining `definitions`, whose
    /// one sheet `b.snxsch` (placing R9) shows in an undocked window with R9
    /// selected. The main window keeps project A's sheet active.
    fn undock_project_b(
        app: &mut Signex,
        definitions: &[&str],
    ) -> (iced::window::Id, crate::app::ProjectId, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("a folder");
        let id = app.document_state.mint_project_id();
        let mut project = app.document_state.projects[0].clone();
        project.id = id;
        project.path = dir.path().join("ProjectB.snxprj");
        project.data.name = "ProjectB".to_string();
        project.data.dir = dir.path().to_string_lossy().into_owned();
        project.data.schematic_root = Some("b.snxsch".to_string());
        project.data.sheets[0].name = "b.snxsch".to_string();
        project.data.sheets[0].filename = "b.snxsch".to_string();
        project.data.variant_definitions = definitions.iter().map(|d| d.to_string()).collect();
        project.data.active_variant = definitions.first().map(|d| d.to_string());
        app.document_state.projects.push(project);
        let path = dir.path().join("b.snxsch");
        let part = uuid::Uuid::from_u128(9);
        let sheet = SchematicSheet {
            symbols: vec![Symbol {
                uuid: part,
                lib_id: "Device:R".to_string(),
                reference: "R9".to_string(),
                value: "10k".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        open_with(&mut app.document_state, &path, sheet);
        let window = iced::window::Id::unique();
        app.ui_state.windows.insert(
            window,
            crate::app::state::WindowKind::UndockedTab {
                path,
                title: "b".into(),
            },
        );
        let mut slot = crate::canvas::CanvasSlot::new();
        slot.selected = vec![SelectedItem::new(part, SelectedKind::Symbol)];
        app.interaction_state.canvases.insert(window, slot);
        (window, id, dir)
    }

    #[test]
    fn an_undocked_sheet_names_the_variant_of_its_own_project() {
        // A defines Lite and its sheet is active in the main window; B has
        // only Base and its sheet is undocked.
        let (mut app, a, _dir) = app_with(&["Lite"], Some("Lite"));
        let (window, _b, _dir_b) = undock_project_b(&mut app, &[]);
        assert_eq!(app.canvas_menu_variant(window), None, "B has no variant");
        assert_eq!(main_variant(&app).map(|v| v.project_id), Some(a));

        // The reverse: A has only Base, B defines Pro.
        let (mut app, _a, _dir) = app_with(&[], None);
        let (window, b, _dir_b) = undock_project_b(&mut app, &["Pro"]);
        assert_eq!(main_variant(&app), None, "A has no variant");
        let variant = app.canvas_menu_variant(window).expect("B's variant");
        assert_eq!(
            variant,
            MenuVariant {
                project_id: b,
                name: "Pro".to_string(),
            }
        );
        let MenuContext::Selection {
            part: Some((reference, _)),
            ..
        } = app.canvas_menu_context(window)
        else {
            panic!("R9 is selected in the undocked window");
        };
        let rows = in_variant_entries(I18n::en_us_ref(), &variant, Some(&reference));
        assert_eq!(layout(&rows)[0], "# R9 in Pro");

        // New variant… from B's window adds to B, not to the active A.
        let press = item(&rows, "New variant…")
            .on_press
            .clone()
            .expect("a live row");
        let _ = app.update(press);
        assert!(
            app.document_state.projects[0]
                .data
                .variant_definitions
                .is_empty()
        );
        assert_eq!(
            app.document_state.projects[1].data.variant_definitions,
            ["Pro", "Variant 1"]
        );
    }

    #[test]
    fn the_in_variant_row_follows_part_actions_and_names_the_active_variant() {
        let (app, id, _dir) = app_with(&["Lite"], Some("Lite"));
        let variant = lite(&app);
        assert_eq!(
            variant,
            MenuVariant {
                project_id: id,
                name: "Lite".to_string(),
            }
        );
        assert_eq!(
            flyout_block(&selection_menu(Some(&variant))),
            ["Part Actions", "In variant: Lite", "Sheet Actions", "Place"]
        );
        let rows = canvas_menu_with_variant(
            I18n::en_us_ref(),
            TID,
            selected(),
            Some(ContextSubmenu::InVariant),
            None,
            &CanvasShortcuts::default(),
            Some(&variant),
        );
        let row = item(&rows, "In variant: Lite");
        assert!(row.submenu && row.active && !row.disabled);
        assert!(row.on_hover.is_some(), "hovering opens the flyout");
    }

    #[test]
    fn a_project_with_only_base_has_no_in_variant_row() {
        let (app, _id, _dir) = app_with(&[], None);
        assert_eq!(main_variant(&app), None);
        assert_eq!(
            flyout_block(&selection_menu(None)),
            ["Part Actions", "Sheet Actions", "Place"]
        );
    }

    #[test]
    fn a_loose_sheet_has_no_in_variant_row() {
        let (mut app, _id, _dir) = app_with(&["Lite"], Some("Lite"));
        app.document_state.projects.clear();
        assert_eq!(main_variant(&app), None);
    }

    #[test]
    fn with_base_active_the_row_names_base() {
        let (app, _id, _dir) = app_with(&["Lite"], None);
        let variant = lite(&app);
        assert_eq!(variant.name, "Base");
        assert!(selection_menu(Some(&variant)).contains(&"In variant: Base".to_string()));
    }

    #[test]
    fn the_sheet_menu_has_no_in_variant_row() {
        let (app, _id, _dir) = app_with(&["Lite"], Some("Lite"));
        let variant = lite(&app);
        let rows = layout(&canvas_menu_with_variant(
            I18n::en_us_ref(),
            TID,
            CanvasSelection::default(),
            None,
            None,
            &CanvasShortcuts::default(),
            Some(&variant),
        ));
        assert!(
            !rows.iter().any(|r| r.starts_with("In variant")),
            "{rows:?}"
        );
    }

    #[test]
    fn the_flyout_heads_with_the_part_and_greys_the_edits_not_built() {
        let (app, _id, _dir) = app_with(&["Lite"], Some("Lite"));
        let window = app.ui_state.main_window_id.expect("main window");
        let MenuContext::Selection {
            part: Some((reference, _)),
            ..
        } = app.canvas_menu_context(window)
        else {
            panic!("one part is selected");
        };
        let rows = in_variant_entries(I18n::en_us_ref(), &lite(&app), Some(&reference));
        assert_eq!(
            layout(&rows),
            [
                "# R5 in Lite",
                "Fitted",
                "Not fitted",
                "Alternate part…",
                "Edit value for this variant…",
                "---",
                "Show in Variant Manager",
                "New variant…",
            ]
        );
        for label in [
            "Fitted",
            "Not fitted",
            "Alternate part…",
            "Edit value for this variant…",
        ] {
            let row = item(&rows, label);
            assert!(row.disabled && row.on_press.is_none(), "{label}");
            assert_eq!(
                row.tooltip.as_deref(),
                Some(
                    "Planned: change a part's fit, part or value in one variant. \
                     The Variant Manager lists each part's fit today."
                ),
                "{label}"
            );
        }
        // A multi-selection names no single part.
        let rows = in_variant_entries(I18n::en_us_ref(), &lite(&app), None);
        assert_eq!(layout(&rows)[0], "Fitted");
    }

    #[test]
    fn show_in_variant_manager_opens_the_projects_variants_section() {
        let (mut app, id, _dir) = app_with(&["Lite"], Some("Lite"));
        app.interaction_state.context_menu = Some(crate::app::ContextMenuState {
            x: 0.0,
            y: 0.0,
            world: None,
            window: None,
        });
        let rows = in_variant_entries(I18n::en_us_ref(), &lite(&app), None);
        let press = item(&rows, "Show in Variant Manager")
            .on_press
            .clone()
            .expect("a live row");
        let _ = app.update(press);
        assert!(
            app.interaction_state.context_menu.is_none(),
            "the menu closes"
        );
        let tab = &app.document_state.tabs[app.document_state.active_tab];
        assert!(matches!(tab.kind, TabKind::Project(p) if p == id));
        assert_eq!(
            app.document_state.project_tabs[&id].section,
            ProjectSection::Variants
        );
        assert_eq!(
            app.document_state.projects[0].data.variant_definitions,
            ["Lite"]
        );
    }

    #[test]
    fn new_variant_adds_one_from_base_in_the_variant_manager() {
        let (mut app, id, _dir) = app_with(&["Lite"], Some("Lite"));
        let rows = in_variant_entries(I18n::en_us_ref(), &lite(&app), None);
        let press = item(&rows, "New variant…")
            .on_press
            .clone()
            .expect("a live row");
        let _ = app.update(press);
        let data = &app.document_state.projects[0].data;
        assert_eq!(data.variant_definitions, ["Lite", "Variant 1"]);
        assert_eq!(
            data.active_variant.as_deref(),
            Some("Lite"),
            "not made active"
        );
        let tab = &app.document_state.tabs[app.document_state.active_tab];
        assert!(matches!(tab.kind, TabKind::Project(p) if p == id));
        let state = &app.document_state.project_tabs[&id];
        assert_eq!(state.section, ProjectSection::Variants);
        assert_eq!(state.variant_selected.as_deref(), Some("Variant 1"));
        assert!(
            state.variant_rename.is_some(),
            "its name is open for typing"
        );
    }

    #[test]
    fn a_pick_for_a_closed_project_is_dropped() {
        let (mut app, id, _dir) = app_with(&["Lite"], Some("Lite"));
        let rows = in_variant_entries(I18n::en_us_ref(), &lite(&app), None);
        let press = item(&rows, "New variant…")
            .on_press
            .clone()
            .expect("a live row");
        let tabs = app.document_state.tabs.len();
        app.document_state.projects.clear();
        let _ = app.update(press);
        assert_eq!(app.document_state.tabs.len(), tabs, "no tab opens");
        assert!(!app.document_state.project_tabs.contains_key(&id));
    }
}
