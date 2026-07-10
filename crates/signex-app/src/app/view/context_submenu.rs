//! Secondary context submenu body (Place / Align / Add New to Project)
//! shown to the right of the canvas and project-tree context menus.
//!
//! Extracted verbatim from `view/context_menus.rs` (ADR-0001, issue
//! #164) as pure code motion — no behaviour change. These are methods
//! of the same `Signex` view impl, split across sibling files.

use super::*;

impl Signex {
    /// Build the secondary submenu (Place / Align) shown to the right
    /// of the parent context menu. Each row dispatches an Active Bar
    /// action via `ContextAction::ActiveBar(...)` so the placement /
    /// transform pipelines stay shared with the toolbar.
    pub(super) fn view_context_submenu(&self, kind: ContextSubmenu) -> Element<'_, Message> {
        use crate::active_bar::ActiveBarAction as A;
        use crate::icons as ic;
        let tid = self.ui_state.theme_id;
        let panel_ctx = &self.document_state.panel_ctx;
        let mut items: Vec<Element<'_, Message>> = Vec::new();
        let mk = |icon: iced::widget::svg::Handle,
                  label: &'static str,
                  action: A|
         -> Element<'_, Message> {
            self.ctx_menu_item_kb(Some(icon), label, "", ContextAction::ActiveBar(action))
        };
        match kind {
            ContextSubmenu::Place => {
                // Wires + buses + entries
                items.push(mk(ic::icon_dd_wire(tid), "Wire", A::DrawWire));
                items.push(mk(ic::icon_dd_bus(tid), "Bus", A::DrawBus));
                items.push(mk(
                    ic::icon_dd_bus_entry(tid),
                    "Bus Entry",
                    A::PlaceBusEntry,
                ));
                items.push(mk(
                    ic::icon_dd_net_label(tid),
                    "Net Label",
                    A::PlaceNetLabel,
                ));
                items.push(self.ctx_menu_sep());
                // Ports
                items.push(mk(ic::icon_dd_port(tid), "Port", A::PlacePort));
                items.push(mk(
                    ic::icon_dd_off_sheet(tid),
                    "Off Sheet Connector",
                    A::PlaceOffSheetConnector,
                ));
                items.push(self.ctx_menu_sep());
                // Power ports (the four most common)
                items.push(mk(ic::icon_dd_gnd(tid), "GND Power Port", A::PlacePowerGND));
                items.push(mk(ic::icon_dd_vcc(tid), "VCC Power Port", A::PlacePowerVCC));
                items.push(mk(
                    ic::icon_dd_pwr_plus5(tid),
                    "+5 Power Port",
                    A::PlacePowerPlus5,
                ));
                items.push(mk(
                    ic::icon_dd_pwr_plus12(tid),
                    "+12 Power Port",
                    A::PlacePowerPlus12,
                ));
                items.push(self.ctx_menu_sep());
                // Directives
                items.push(mk(
                    ic::icon_dd_param_set(tid),
                    "Parameter Set",
                    A::PlaceParameterSet,
                ));
                items.push(mk(ic::icon_dd_no_erc(tid), "Generic No ERC", A::PlaceNoERC));
                items.push(mk(
                    ic::icon_dd_diff_pair(tid),
                    "Differential Pair",
                    A::PlaceDiffPair,
                ));
                items.push(mk(ic::icon_dd_blanket(tid), "Blanket", A::PlaceBlanket));
                items.push(self.ctx_menu_sep());
                // Harness
                items.push(mk(
                    ic::icon_dd_harness(tid),
                    "Signal Harness",
                    A::PlaceSignalHarness,
                ));
                items.push(mk(
                    ic::icon_dd_harness_conn(tid),
                    "Harness Connector",
                    A::PlaceHarnessConnector,
                ));
                items.push(mk(
                    ic::icon_dd_harness_entry(tid),
                    "Harness Entry",
                    A::PlaceHarnessEntry,
                ));
                items.push(self.ctx_menu_sep());
                // Sheet symbols
                items.push(mk(
                    ic::icon_dd_sheet_symbol(tid),
                    "Sheet Symbol",
                    A::PlaceSheetSymbol,
                ));
                items.push(mk(
                    ic::icon_dd_sheet_entry(tid),
                    "Sheet Entry",
                    A::PlaceSheetEntry,
                ));
                items.push(mk(
                    ic::icon_dd_device_sheet(tid),
                    "Device Sheet Symbol",
                    A::PlaceDeviceSheetSymbol,
                ));
                items.push(self.ctx_menu_sep());
                // Component
                items.push(mk(ic::icon_component(tid), "Part", A::PlaceComponent));
                items.push(self.ctx_menu_sep());
                // Text
                items.push(mk(
                    ic::icon_dd_text_string(tid),
                    "Text String",
                    A::PlaceTextString,
                ));
                items.push(mk(
                    ic::icon_dd_text_frame(tid),
                    "Text Frame",
                    A::PlaceTextFrame,
                ));
                items.push(mk(ic::icon_dd_note(tid), "Note", A::PlaceNote));
            }
            ContextSubmenu::Align => {
                // Altium gating: pairwise aligns (Left/Right/Top/Bottom/H/V
                // Centers) need ≥2 items to make sense; Distribute needs
                // ≥3 (two endpoints + at least one item to space between
                // them); Align To Grid works on a single item too. The
                // submenu is only opened when something is selected, so
                // grid is always enabled here.
                let n = self.interaction_state.canvas.selected.len();
                let pair = n >= 2;
                let dist = n >= 3;
                let mk_or_disabled = |icon: iced::widget::svg::Handle,
                                      label: &'static str,
                                      action: A,
                                      enabled: bool|
                 -> Element<'_, Message> {
                    if enabled {
                        mk(icon, label, action)
                    } else {
                        self.ctx_menu_item_disabled(Some(icon), label, None)
                    }
                };
                items.push(mk_or_disabled(
                    ic::icon_dd_align_left(tid),
                    "Align Left",
                    A::AlignLeft,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_right(tid),
                    "Align Right",
                    A::AlignRight,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_hcenter(tid),
                    "Align Horizontal Centers",
                    A::AlignHorizontalCenters,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_dist_horiz(tid),
                    "Distribute Horizontally",
                    A::DistributeHorizontally,
                    dist,
                ));
                items.push(self.ctx_menu_sep());
                items.push(mk_or_disabled(
                    ic::icon_dd_align_top(tid),
                    "Align Top",
                    A::AlignTop,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_bottom(tid),
                    "Align Bottom",
                    A::AlignBottom,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_vcenter(tid),
                    "Align Vertical Centers",
                    A::AlignVerticalCenters,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_dist_vert(tid),
                    "Distribute Vertically",
                    A::DistributeVertically,
                    dist,
                ));
                items.push(self.ctx_menu_sep());
                items.push(mk(
                    ic::icon_dd_align_grid(tid),
                    "Align To Grid",
                    A::AlignToGrid,
                ));
            }
            ContextSubmenu::AddNewToProject => {
                // Altium parity: this is the master "Add New" picker
                // for the active project. The Component Library row
                // wires through to `commands::create_library`; the
                // other rows stay version-badged stubs until their
                // respective editors land.
                // The right-clicked project lives at `tree_path[0]`
                // for any `AddNewToProject` submenu item — resolve it
                // off the captured menu state so the action targets
                // the right project even when another is active.
                let target_path = self
                    .interaction_state
                    .project_tree_context_menu
                    .as_ref()
                    .and_then(|m| m.path.clone())
                    .unwrap_or_default();
                items.push(self.ctx_menu_item_msg(
                    Some(ic::icon_dd_wire(tid)),
                    "Schematic",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        crate::app::ProjectTreeAction::AddNewSchematic(target_path.clone()),
                    )),
                ));
                // Component Library is the Altium-style replacement
                // for the legacy "Schematic Library" row. Wired
                // through the menu bridge so the existing menu
                // dispatcher resolves the active project and emits
                // `LibraryMessage::CreateLibraryAt(...)`.
                items.push(self.ctx_menu_item_msg(
                    Some(ic::icon_component(tid)),
                    "Component Library",
                    "",
                    Message::Menu(crate::menu_bar::MenuMessage::AddComponentLibrary),
                ));
                // Altium parity: Schematic Library (= our `.snxsym`) is
                // a top-level project document, not nested inside a
                // Component Library. Same for PCB Library (= our
                // `.snxfpt`). Both open a Save-As dialog scoped to the
                // project dir; the picked file is written empty and
                // opened as a primitive editor tab.
                items.push(self.ctx_menu_item_msg(
                    Some(ic::icon_component(tid)),
                    "Symbol Library",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        crate::app::ProjectTreeAction::AddProjectSymbolLibrary(target_path.clone()),
                    )),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_part_actions(tid)),
                    "PCB",
                    Some("v2.0"),
                ));
                // v0.13.0 — footprint editor gated off; hide the
                // "PCB Library" create entry on the project-root menu.
                if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
                    items.push(self.ctx_menu_item_msg(
                        Some(ic::icon_component(tid)),
                        "PCB Library",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            crate::app::ProjectTreeAction::AddProjectFootprintLibrary(
                                target_path.clone(),
                            ),
                        )),
                    ));
                }
                items.push(self.ctx_menu_sep());
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_string(tid)),
                    "Output Job",
                    Some("v1.3"),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_frame(tid)),
                    "Design Notebook",
                    Some("v1.4"),
                ));
                items.push(self.ctx_menu_sep());
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_string(tid)),
                    "Constraint File",
                    Some("v3.x"),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_string(tid)),
                    "VHDL File",
                    Some("v3.x"),
                ));
            }
        }
        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&panel_ctx.tokens))
            .into()
    }
}
