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
    command_text, dd_cmd_disabled, dd_command, dd_disabled, dd_kb, dd_msg, dd_planned, tag_command,
    text,
};
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

/// Place submenu — wires, buses, ports, power, directives, harness, sheet
/// symbols, component, and text. Rows for planned tools are greyed with a
/// tooltip; every other row is always enabled. `hint` resolves a command id
/// to its keymap shortcut.
pub(super) fn place_entries(
    i18n: &I18n,
    tid: ThemeId,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    use crate::active_bar::ActiveBarAction as A;
    let ab = |icon, label: Option<MessageId>, action: A| {
        let planned = crate::active_bar::is_planned(&action);
        active_bar_row(i18n, icon, label, action, true, planned, hint)
    };
    vec![
        // Wires + buses + entries
        ab(ic::icon_dd_wire(tid), None, A::DrawWire),
        ab(ic::icon_dd_bus(tid), None, A::DrawBus),
        ab(ic::icon_dd_bus_entry(tid), None, A::PlaceBusEntry),
        ab(ic::icon_dd_net_label(tid), None, A::PlaceNetLabel),
        ab(ic::icon_dd_junction(tid), None, A::PlaceJunction),
        DropdownEntry::Separator,
        // Ports
        ab(ic::icon_dd_port(tid), None, A::PlacePort),
        ab(ic::icon_dd_off_sheet(tid), None, A::PlaceOffSheetConnector),
        DropdownEntry::Separator,
        // Power ports (the four most common)
        ab(
            ic::icon_dd_gnd(tid),
            Some(menu_ids::CTX_PLACE_POWER_GND),
            A::PlacePowerGND,
        ),
        ab(
            ic::icon_dd_vcc(tid),
            Some(menu_ids::CTX_PLACE_POWER_VCC),
            A::PlacePowerVCC,
        ),
        ab(
            ic::icon_dd_pwr_plus5(tid),
            Some(menu_ids::CTX_PLACE_POWER_PLUS5),
            A::PlacePowerPlus5,
        ),
        ab(
            ic::icon_dd_pwr_plus12(tid),
            Some(menu_ids::CTX_PLACE_POWER_PLUS12),
            A::PlacePowerPlus12,
        ),
        DropdownEntry::Separator,
        // Directives
        ab(ic::icon_dd_param_set(tid), None, A::PlaceParameterSet),
        ab(ic::icon_dd_no_erc(tid), None, A::PlaceNoERC),
        ab(ic::icon_dd_diff_pair(tid), None, A::PlaceDiffPair),
        ab(ic::icon_dd_blanket(tid), None, A::PlaceBlanket),
        DropdownEntry::Separator,
        // Harness
        ab(ic::icon_dd_harness(tid), None, A::PlaceSignalHarness),
        ab(
            ic::icon_dd_harness_conn(tid),
            None,
            A::PlaceHarnessConnector,
        ),
        ab(ic::icon_dd_harness_entry(tid), None, A::PlaceHarnessEntry),
        DropdownEntry::Separator,
        // Sheet symbols
        ab(ic::icon_dd_sheet_symbol(tid), None, A::PlaceSheetSymbol),
        ab(ic::icon_dd_sheet_entry(tid), None, A::PlaceSheetEntry),
        ab(
            ic::icon_dd_device_sheet(tid),
            None,
            A::PlaceDeviceSheetSymbol,
        ),
        DropdownEntry::Separator,
        // Component
        ab(ic::icon_component(tid), None, A::PlaceComponent),
        DropdownEntry::Separator,
        // Text
        ab(ic::icon_dd_text_string(tid), None, A::PlaceTextString),
        ab(ic::icon_dd_text_frame(tid), None, A::PlaceTextFrame),
        ab(ic::icon_dd_note(tid), None, A::PlaceNote),
    ]
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

/// Add New to Project submenu — the master "Add New" picker for the
/// right-clicked project (resolved as `target`). Schematic / Component
/// Library / Symbol Library are wired; the rest stay version-badged stubs
/// until their editors land.
pub(super) fn add_new_entries(
    i18n: &I18n,
    tid: ThemeId,
    target: signex_widgets::tree_view::TreeKey,
) -> Vec<DropdownEntry<Message>> {
    use crate::app::ProjectTreeAction as P;
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(12);

    v.push(dd_msg(
        Some(ic::icon_dd_wire(tid)),
        &text(i18n, menu_ids::CTX_ADD_SCHEMATIC),
        "",
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(P::AddNewSchematic(
            target.clone(),
        ))),
    ));
    // Component Library is the Altium-style replacement for the legacy
    // "Schematic Library" row, wired through the menu bridge.
    v.push(dd_msg(
        Some(ic::icon_component(tid)),
        &text(i18n, menu_ids::CTX_ADD_COMPONENT_LIBRARY),
        "",
        Message::Menu(crate::menu_bar::MenuMessage::AddComponentLibrary),
    ));
    // Altium parity: Symbol Library (= our `.snxsym`) is a top-level
    // project document. Opens a Save-As dialog scoped to the project dir.
    v.push(dd_msg(
        Some(ic::icon_component(tid)),
        &text(i18n, menu_ids::CTX_ADD_SYMBOL_LIBRARY),
        "",
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            P::AddProjectSymbolLibrary(target.clone()),
        )),
    ));
    // The project's one board, opened unsaved (#1312).
    v.push(dd_msg(
        Some(ic::icon_dd_part_actions(tid)),
        &text(i18n, menu_ids::CTX_ADD_PCB),
        "",
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(P::AddNewPcb(
            target.clone(),
        ))),
    ));
    // Footprint editor gated off for release; hide the "PCB Library" create
    // entry on the project-root menu.
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        v.push(dd_msg(
            Some(ic::icon_component(tid)),
            &text(i18n, menu_ids::CTX_ADD_PCB_LIBRARY),
            "",
            Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                P::AddProjectFootprintLibrary(target.clone()),
            )),
        ));
    }
    v.push(DropdownEntry::Separator);
    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_text_string(tid)),
        &text(i18n, menu_ids::CTX_ADD_OUTPUT_JOB),
        None,
    ));
    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_text_frame(tid)),
        &text(i18n, menu_ids::CTX_ADD_DESIGN_NOTEBOOK),
        None,
    ));
    v.push(DropdownEntry::Separator);
    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_text_string(tid)),
        &text(i18n, menu_ids::CTX_ADD_CONSTRAINT_FILE),
        None,
    ));
    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_text_string(tid)),
        &text(i18n, menu_ids::CTX_ADD_VHDL_FILE),
        None,
    ));

    v
}

impl Signex {
    /// Build the secondary submenu (Place / Align / Add New to Project)
    /// shown to the right of the parent context menu. Resolves the
    /// `selected` is the shown sheet's selection count; the target project is
    /// resolved from `self`. The pure entry builders do the rest and the
    /// shared widget renders it.
    pub(in crate::app::view) fn view_context_submenu(
        &self,
        kind: ContextSubmenu,
        selected: usize,
    ) -> Element<'_, Message> {
        let tid = self.ui_state.effective_theme_id();
        let tokens = &self.document_state.panel_ctx.tokens;
        let entries = match kind {
            ContextSubmenu::Place => place_entries(&self.ui_state.i18n, tid, &|id| {
                self.keymap_shortcut_label(id)
            }),
            ContextSubmenu::Align => align_entries(&self.ui_state.i18n, tid, selected, &|id| {
                self.keymap_shortcut_label(id)
            }),
            ContextSubmenu::Arrange => {
                arrange_entries(&self.ui_state.i18n, &|id| self.keymap_shortcut_label(id))
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
                add_new_entries(&self.ui_state.i18n, tid, target)
            }
        };
        super::placement::menu_card(entries, tokens, &self.document_state.panel_ctx.ui_font_name)
            .element
    }
}
