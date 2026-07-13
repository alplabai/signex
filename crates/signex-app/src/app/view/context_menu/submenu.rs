//! Secondary context submenu body (Place / Align / Add New to Project)
//! shown to the right of the canvas and project-tree context menus.
//!
//! Data-to-view (#269): each submenu is a pure `Vec<DropdownEntry<Message>>`
//! rendered by the shared `signex_widgets::active_bar_dropdown` widget, so
//! the flyout shares its row chrome with the parent menus and the active
//! bars (ADR-0003). Every Place / Align row dispatches an Active Bar action
//! via `ContextAction::ActiveBar(...)` so placement / transform pipelines
//! stay shared with the toolbar.

use super::*;

use super::items::{dd_disabled, dd_kb, dd_msg};
use crate::icons as ic;
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Place submenu — wires, buses, ports, power, directives, harness, sheet
/// symbols, component, and text. Every row is always enabled.
pub(super) fn place_entries(tid: ThemeId) -> Vec<DropdownEntry<Message>> {
    use crate::active_bar::ActiveBarAction as A;
    let ab = |icon, label: &str, action: A| {
        dd_kb(Some(icon), label, "", ContextAction::ActiveBar(action))
    };
    vec![
        // Wires + buses + entries
        ab(ic::icon_dd_wire(tid), "Wire", A::DrawWire),
        ab(ic::icon_dd_bus(tid), "Bus", A::DrawBus),
        ab(ic::icon_dd_bus_entry(tid), "Bus Entry", A::PlaceBusEntry),
        ab(ic::icon_dd_net_label(tid), "Net Label", A::PlaceNetLabel),
        DropdownEntry::Separator,
        // Ports
        ab(ic::icon_dd_port(tid), "Port", A::PlacePort),
        ab(
            ic::icon_dd_off_sheet(tid),
            "Off Sheet Connector",
            A::PlaceOffSheetConnector,
        ),
        DropdownEntry::Separator,
        // Power ports (the four most common)
        ab(ic::icon_dd_gnd(tid), "GND Power Port", A::PlacePowerGND),
        ab(ic::icon_dd_vcc(tid), "VCC Power Port", A::PlacePowerVCC),
        ab(ic::icon_dd_pwr_plus5(tid), "+5 Power Port", A::PlacePowerPlus5),
        ab(
            ic::icon_dd_pwr_plus12(tid),
            "+12 Power Port",
            A::PlacePowerPlus12,
        ),
        DropdownEntry::Separator,
        // Directives
        ab(
            ic::icon_dd_param_set(tid),
            "Parameter Set",
            A::PlaceParameterSet,
        ),
        ab(ic::icon_dd_no_erc(tid), "Generic No ERC", A::PlaceNoERC),
        ab(
            ic::icon_dd_diff_pair(tid),
            "Differential Pair",
            A::PlaceDiffPair,
        ),
        ab(ic::icon_dd_blanket(tid), "Blanket", A::PlaceBlanket),
        DropdownEntry::Separator,
        // Harness
        ab(
            ic::icon_dd_harness(tid),
            "Signal Harness",
            A::PlaceSignalHarness,
        ),
        ab(
            ic::icon_dd_harness_conn(tid),
            "Harness Connector",
            A::PlaceHarnessConnector,
        ),
        ab(
            ic::icon_dd_harness_entry(tid),
            "Harness Entry",
            A::PlaceHarnessEntry,
        ),
        DropdownEntry::Separator,
        // Sheet symbols
        ab(
            ic::icon_dd_sheet_symbol(tid),
            "Sheet Symbol",
            A::PlaceSheetSymbol,
        ),
        ab(
            ic::icon_dd_sheet_entry(tid),
            "Sheet Entry",
            A::PlaceSheetEntry,
        ),
        ab(
            ic::icon_dd_device_sheet(tid),
            "Device Sheet Symbol",
            A::PlaceDeviceSheetSymbol,
        ),
        DropdownEntry::Separator,
        // Component
        ab(ic::icon_component(tid), "Part", A::PlaceComponent),
        DropdownEntry::Separator,
        // Text
        ab(
            ic::icon_dd_text_string(tid),
            "Text String",
            A::PlaceTextString,
        ),
        ab(ic::icon_dd_text_frame(tid), "Text Frame", A::PlaceTextFrame),
        ab(ic::icon_dd_note(tid), "Note", A::PlaceNote),
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
pub(super) fn align_entries(tid: ThemeId, selected: usize) -> Vec<DropdownEntry<Message>> {
    use crate::active_bar::ActiveBarAction as A;
    let (pair, dist) = align_gate(selected);
    let row = |icon, label: &str, action: A, enabled: bool| {
        if enabled {
            dd_kb(Some(icon), label, "", ContextAction::ActiveBar(action))
        } else {
            dd_disabled(Some(icon), label, None)
        }
    };
    vec![
        row(ic::icon_dd_align_left(tid), "Align Left", A::AlignLeft, pair),
        row(
            ic::icon_dd_align_right(tid),
            "Align Right",
            A::AlignRight,
            pair,
        ),
        row(
            ic::icon_dd_align_hcenter(tid),
            "Align Horizontal Centers",
            A::AlignHorizontalCenters,
            pair,
        ),
        row(
            ic::icon_dd_dist_horiz(tid),
            "Distribute Horizontally",
            A::DistributeHorizontally,
            dist,
        ),
        DropdownEntry::Separator,
        row(ic::icon_dd_align_top(tid), "Align Top", A::AlignTop, pair),
        row(
            ic::icon_dd_align_bottom(tid),
            "Align Bottom",
            A::AlignBottom,
            pair,
        ),
        row(
            ic::icon_dd_align_vcenter(tid),
            "Align Vertical Centers",
            A::AlignVerticalCenters,
            pair,
        ),
        row(
            ic::icon_dd_dist_vert(tid),
            "Distribute Vertically",
            A::DistributeVertically,
            dist,
        ),
        DropdownEntry::Separator,
        dd_kb(
            Some(ic::icon_dd_align_grid(tid)),
            "Align To Grid",
            "",
            ContextAction::ActiveBar(A::AlignToGrid),
        ),
    ]
}

/// Add New to Project submenu — the master "Add New" picker for the
/// right-clicked project (resolved as `target`). Schematic / Component
/// Library / Symbol Library are wired; the rest stay version-badged stubs
/// until their editors land.
pub(super) fn add_new_entries(tid: ThemeId, target: Vec<usize>) -> Vec<DropdownEntry<Message>> {
    use crate::app::ProjectTreeAction as P;
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(12);

    v.push(dd_msg(
        Some(ic::icon_dd_wire(tid)),
        "Schematic",
        "",
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(P::AddNewSchematic(
            target.clone(),
        ))),
    ));
    // Component Library is the Altium-style replacement for the legacy
    // "Schematic Library" row, wired through the menu bridge.
    v.push(dd_msg(
        Some(ic::icon_component(tid)),
        "Component Library",
        "",
        Message::Menu(crate::menu_bar::MenuMessage::AddComponentLibrary),
    ));
    // Altium parity: Symbol Library (= our `.snxsym`) is a top-level
    // project document. Opens a Save-As dialog scoped to the project dir.
    v.push(dd_msg(
        Some(ic::icon_component(tid)),
        "Symbol Library",
        "",
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(P::AddProjectSymbolLibrary(
            target.clone(),
        ))),
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_part_actions(tid)),
        "PCB",
        Some("v2.0"),
    ));
    // v0.13.0 — footprint editor gated off; hide the "PCB Library" create
    // entry on the project-root menu.
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        v.push(dd_msg(
            Some(ic::icon_component(tid)),
            "PCB Library",
            "",
            Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                P::AddProjectFootprintLibrary(target.clone()),
            )),
        ));
    }
    v.push(DropdownEntry::Separator);
    v.push(dd_disabled(
        Some(ic::icon_dd_text_string(tid)),
        "Output Job",
        Some("v1.3"),
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_text_frame(tid)),
        "Design Notebook",
        Some("v1.4"),
    ));
    v.push(DropdownEntry::Separator);
    v.push(dd_disabled(
        Some(ic::icon_dd_text_string(tid)),
        "Constraint File",
        Some("v3.x"),
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_text_string(tid)),
        "VHDL File",
        Some("v3.x"),
    ));

    v
}

impl Signex {
    /// Build the secondary submenu (Place / Align / Add New to Project)
    /// shown to the right of the parent context menu. Resolves the
    /// selection count / target project from `self`, then delegates to the
    /// pure entry builders and renders via the shared widget.
    pub(in crate::app::view) fn view_context_submenu(
        &self,
        kind: ContextSubmenu,
    ) -> Element<'_, Message> {
        let tid = self.ui_state.theme_id;
        let tokens = &self.document_state.panel_ctx.tokens;
        let entries = match kind {
            ContextSubmenu::Place => place_entries(tid),
            ContextSubmenu::Align => {
                align_entries(tid, self.interaction_state.canvas.selected.len())
            }
            ContextSubmenu::AddNewToProject => {
                // The right-clicked project lives at `tree_path[0]` —
                // resolve it off the captured menu state so the action
                // targets the right project even when another is active.
                let target = self
                    .interaction_state
                    .project_tree_context_menu
                    .as_ref()
                    .and_then(|m| m.path.clone())
                    .unwrap_or_default();
                add_new_entries(tid, target)
            }
        };
        signex_widgets::active_bar_dropdown::view(entries, tokens, Some(Self::CONTEXT_MENU_WIDTH))
    }
}
