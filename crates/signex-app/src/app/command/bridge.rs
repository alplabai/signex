//! The app's single id→[`Message`] bridge.
//!
//! [`core_to_message`] turns a stable `AppCommandId` (the id a profile
//! TOML binds a key to) into the app's namespaced [`Message`] tree
//! (ADR-0001 D3). It is not keymap-specific — menus, the command
//! palette, and a future CLI all route a command id through here on
//! their way to `update`.

use crate::keymap::AppCommandId;

use super::super::*;

/// Resolve a command for DISPATCH, reporting when it cannot be resolved.
///
/// Maps a stable command id onto the app's namespaced [`Message`] tree.
/// Commands without a live dispatch arm return `None`, and
/// [`log_unmapped`] says so — naming the id and which of the two
/// failure modes it hit — so an invocation can no longer vanish without
/// a trace. The catalog ids in `UNMAPPED_CATALOG_IDS` are in that state, every one of them bound to
/// a trigger in a shipped profile; the set is pinned by
/// `tests::UNMAPPED_CATALOG_IDS` and may only shrink.
///
/// Use [`is_dispatchable`] instead when you are merely *asking* whether a
/// command resolves. #619: the palette filtered its rows through this
/// function, so building the palette view emitted one warning per
/// unmapped id — 2432 records in a single session, against a 200-entry
/// ring buffer, which evicted every real diagnostic before anyone could
/// read it. The message was also false on that path: nothing had been
/// invoked.
pub(crate) fn core_to_message(command: &AppCommandId) -> Option<Message> {
    let resolved = resolve(command);
    if resolved.is_none() {
        log_unmapped(command);
    }
    resolved
}

/// Does this command resolve to a message? Silent — no diagnostics.
///
/// The counterpart to [`core_to_message`] for callers that are asking a
/// question rather than dispatching. A query must not narrate.
pub(crate) fn is_dispatchable(command: &AppCommandId) -> bool {
    resolve(command).is_some()
}

/// The resolution itself. Pure: no logging, no side effects, so both
/// entry points above are built from one table and cannot drift.
pub(super) fn resolve(command: &AppCommandId) -> Option<Message> {
    use crate::library::editor::footprint::state::EditorMode;

    let message = match command.as_str() {
        "annotate_schematic" => Message::Annotate(AnnotateMsg::OpenDialog),
        "annotate_schematic_quietly" => {
            Message::Annotate(AnnotateMsg::Run(signex_engine::AnnotateMode::Incremental))
        }
        // No originating window: this Esc was invoked as a command, not
        // typed. `None` resolves as a main-window Esc (#547).
        "cancel_current_tool" => Message::Input(InputMsg::EscapePressed { window: None }),
        // Recentres without zooming; it used to share the fit arm below (#525).
        "center_view_at_cursor" => Message::CanvasEvent(CanvasEvent::CenterAtCursor),
        // One fit primitive; it frames object extents, as all three labels promise.
        "show_all_design_objects" | "zoom_to_all_objects" | "zoom_to_fit" => {
            Message::CanvasEvent(CanvasEvent::FitAll)
        }
        "drag_keep_connections" => {
            Message::Selection(selection_request::SelectionRequest::DragKeepingConnections)
        }
        "find_similar_objects" => {
            Message::Selection(selection_request::SelectionRequest::FindSimilar(
                crate::find_similar::FindSimilarMsg::OpenRequested { at: None },
            ))
        }
        "zoom_to_selection_area" => Message::CanvasEvent(CanvasEvent::FitSelection),
        "measure_distance" => Message::Tool(ToolMessage::SelectTool(Tool::Measure)),
        "copy" => Message::Edit(EditMsg::Copy),
        "cycle_selection_mode" => {
            Message::Selection(selection_request::SelectionRequest::CycleMode)
        }
        "previous_grid" | "cycle_snap_grid_backward" => Message::Ui(UiMsg::GridCycleBackward),
        "toggle_properties_panel" => Message::Overlay(OverlayMsg::PanelToggled(
            crate::panels::PanelKind::Properties,
        )),
        "toggle_schematic_filter_panel" => Message::Overlay(OverlayMsg::PanelToggled(
            crate::panels::PanelKind::SchFilter,
        )),
        "toggle_schematic_list_panel" => {
            Message::Overlay(OverlayMsg::PanelToggled(crate::panels::PanelKind::SchList))
        }
        "next_document_tab" => Message::Menu(MenuMessage::NextDocumentTab),
        "previous_document_tab" => Message::Menu(MenuMessage::PreviousDocumentTab),
        "find_next" => Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::NextRequested),
        "find_previous" => {
            Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::PreviousRequested)
        }
        "next_grid" => Message::Ui(UiMsg::GridCycle),
        "cycle_snap_grid_forward" | "open_grid_picker" => Message::Ui(UiMsg::GridPickerOpen),
        "cycle_unit" => Message::Ui(UiMsg::UnitCycled),
        "cycle_wire_bus_graphic_mode" => Message::Tool(ToolMessage::CycleDrawMode),
        "cut" => Message::Edit(EditMsg::Cut),
        "delete_selection" => Message::Edit(EditMsg::DeleteSelected),
        "remove_last_vertex" => Message::Tool(ToolMessage::RemoveLastVertex),
        "draw_graphic_line" => Message::Tool(ToolMessage::SelectTool(Tool::Line)),
        "draw_hierarchical_sheet" => Message::Tool(ToolMessage::SelectTool(Tool::Sheet)),
        "duplicate" => Message::Edit(EditMsg::Duplicate),
        "bring_drawings_to_front" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::ToFront,
        )),
        "send_drawings_to_back" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::ToBack,
        )),
        "bring_drawings_forward" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::Forward,
        )),
        "send_drawings_backward" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::Backward,
        )),
        // Docking the Properties panel IS how this app shows the
        // selection's properties: the status bar's
        // `OpenPropertiesForSelection` and the canvas context menu's
        // "Properties..." row both send this same message.
        "edit_object_properties" | "edit_selected_object_properties" => {
            Message::Menu(MenuMessage::OpenPropertiesPanel)
        }
        "find" | "find_text" => Message::Overlay(OverlayMsg::OpenFind),
        "find_and_replace" => Message::Overlay(OverlayMsg::OpenReplace),
        "footprint_mode_pads" => {
            Message::Input(InputMsg::FootprintModeShortcut(EditorMode::Normal))
        }
        "footprint_mode_sketch" => {
            Message::Input(InputMsg::FootprintModeShortcut(EditorMode::Sketch))
        }
        "footprint_mode_view_3d" => {
            Message::Input(InputMsg::FootprintModeShortcut(EditorMode::View3d))
        }
        "force_annotate_all_schematics" => Message::Annotate(AnnotateMsg::Run(
            signex_engine::AnnotateMode::ResetAndRenumber,
        )),
        // Visual-flip semantics (preserved from the pre-keymap hardcoded map):
        // the `X` key = a horizontal (left-right) flip = internal MirrorSelectedY,
        // and `Y` = vertical (top-bottom) flip = MirrorSelectedX. The presets
        // bind physical `X`->mirror_x / `Y`->mirror_y, so the command id names
        // the KEY, and the arm names the AXIS it flips — hence the cross.
        "mirror_x" => Message::Edit(EditMsg::MirrorSelectedY),
        "mirror_y" => Message::Edit(EditMsg::MirrorSelectedX),
        "open_components_panel" | "place_symbol" => {
            Message::Tool(ToolMessage::SelectTool(Tool::Component))
        }
        "new_document" => Message::Menu(MenuMessage::NewProject),
        "open_command_palette" => Message::CommandPalette(CommandPaletteMsg::Open),
        "open_document" => Message::Menu(MenuMessage::OpenProject),
        "open_grid_properties" => Message::GridProperties(GridPropertiesMsg::Open),
        "open_net_color_palette" => Message::NetColor(NetColorMsg::Open),
        // One dialog, two ids. `PrefNav` has no schematic page —
        // Appearance / Keyboard / Erc / LibraryDistributors /
        // ComponentClasses — and the open handler picks no page, so
        // both ids land on whichever page was last visited. The label
        // "Open schematic preferences" therefore over-promises; the
        // dialog it opens is the only preferences dialog there is.
        "open_preferences" | "open_schematic_preferences" => {
            Message::Preferences(PreferencesMsg::Open)
        }
        "repeat_last_item" => Message::Edit(EditMsg::RepeatLastItem),
        "paste" => Message::Edit(EditMsg::Paste),
        "paste_special" | "smart_paste" => Message::Edit(EditMsg::SmartPaste),
        "place_bus" => Message::Tool(ToolMessage::SelectTool(Tool::Bus)),
        "place_local_net_label" | "place_net_label" => {
            Message::Tool(ToolMessage::SelectTool(Tool::Label))
        }
        "place_no_connect" => Message::Tool(ToolMessage::SelectTool(Tool::NoConnect)),
        "place_text" => Message::Tool(ToolMessage::SelectTool(Tool::Text)),
        "place_wire" => Message::Tool(ToolMessage::SelectTool(Tool::Wire)),
        "place_wire_to_bus_entry" => Message::Tool(ToolMessage::SelectTool(Tool::BusEntry)),
        "placement_accept" => Message::Selection(selection_request::SelectionRequest::LassoCommit),
        "placement_properties" => Message::Tool(ToolMessage::PrePlacementTab),
        "print" => Message::PrintPreview(PrintPreviewMsg::Requested),
        "redo" => Message::Edit(EditMsg::Redo),
        // `ExportBom` reaches `handle_bom_preview_open`, the BOM dialog;
        // Design > Generate BOM opens the same one.
        "report_manager_bom" => Message::Menu(MenuMessage::ExportBom),
        "reset_schematic_designators" => Message::Annotate(AnnotateMsg::OpenResetConfirm),
        "rotate_clockwise" => Message::Edit(EditMsg::RotateSelectedClockwise),
        "rotate_counterclockwise" => Message::Edit(EditMsg::RotateSelectedCounterclockwise),
        "run_erc" => Message::Erc(ErcMsg::Run),
        "save_document" => Message::File(FileMsg::Save),
        "save_document_as" => Message::Menu(MenuMessage::SaveAs),
        "select_all" => Message::Selection(selection_request::SelectionRequest::SelectAll),
        "show_current_command_hotkeys" | "show_current_command_shortcuts" => {
            Message::Menu(MenuMessage::OpenKeyboardShortcuts)
        }
        "toggle_visible_grid" => Message::Ui(UiMsg::GridToggle),
        "toggle_auto_focus" => Message::Overlay(OverlayMsg::ToggleAutoFocus),
        "toggle_electrical_grid" => Message::Ui(UiMsg::ToggleSnapHotspots),
        "undo" => Message::Edit(EditMsg::Undo),
        "unselect_all" => Message::Selection(selection_request::SelectionRequest::ClearAll),
        // Tools > Up Hierarchy: the sheet that references the focused one.
        "leave_sheet" | "navigate_up_hierarchy" => Message::Menu(MenuMessage::AscendToParentSheet),
        "next_sheet" => Message::Menu(MenuMessage::NextSheet),
        "previous_sheet" => Message::Menu(MenuMessage::PreviousSheet),
        // Zooms about the view centre, as View > Zoom In/Out does.
        "zoom_in_at_cursor" => Message::Menu(MenuMessage::ZoomIn),
        "zoom_out_at_cursor" => Message::Menu(MenuMessage::ZoomOut),
        // BRIDGE_ARMS_END — `tests::match_block` scans for command-id
        // literals up to this marker and no further. Everything below is
        // fallthrough machinery whose own string literals must not be
        // mistaken for bridge arms. Move the marker, not the anchor.
        //
        // The schematic Active Bar's 59 actions are a table, not 59 arms
        // here — see `super::active_bar`. Consulted before giving up, so
        // adding an Active Bar command means adding one table row.
        id => Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            super::active_bar::action_for_id(id)?,
        )),
    };
    Some(message)
}

/// Report an id that reached the bridge and found no arm.
///
/// Lives here, not at a call site, so every consumer of the registry
/// inherits it — the keyboard today, and the menu bar, command palette
/// and CLI as they are rewired onto `dispatch_command` (#367, #366).
///
/// The two failure modes want different words because they need
/// different fixes:
/// - a real catalog id with no arm — the binding is advertised in the
///   Keyboard Shortcuts pane and does nothing. the ids in `UNMAPPED_CATALOG_IDS` are in this state;
///   see `tests::UNMAPPED_CATALOG_IDS`.
/// - an id that is in no catalog at all — almost always a typo in a
///   user-edited keymap TOML, which `keymap::profile` accepts without
///   ever validating against the catalog.
///
/// Deliberately `log_warning`, not `log_error`: neither case loses data
/// or leaves the app wrong, and neither is actionable mid-edit.
fn log_unmapped(command: &AppCommandId) {
    let id = command.as_str();
    if let Some(metadata) = crate::keymap::metadata_for(command) {
        crate::diagnostics::log_warning(format!(
            "command '{id}': {} is not available yet",
            metadata.label
        ));
    } else {
        crate::diagnostics::log_warning(format!(
            "command '{id}' is not in the command catalog — check the id in \
             your keyboard-shortcuts profile"
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::keymap::metadata_for;

    /// `core_to_message`'s own source, embedded at compile time so the
    /// guard below scans the exact arms it checks instead of a
    /// hand-kept duplicate list ("kept in step" copies are banned).
    const BRIDGE_SRC: &str = include_str!("bridge.rs");

    /// The `core_to_message` match block alone, cut out of `src`.
    ///
    /// Load-bearing: `BRIDGE_SRC` is the WHOLE file, tests included, and
    /// this module now holds `UNMAPPED_CATALOG_IDS` — the string literals
    /// on lines that start with `"`. Scanning the whole file would read
    /// every one of them as a bridge arm and quietly invert the ratchet.
    fn match_block(src: &str) -> &str {
        let start = src
            .find("let message = match command.as_str() {")
            .expect("core_to_message's match block moved — update match_block");
        let rest = &src[start..];
        // Ends at an explicit marker rather than at the fallthrough arm's
        // text, which has already changed shape once. Everything after it
        // — `log_unmapped`'s message literals, the pinned id list — must
        // stay out of the scan.
        let end = rest
            .find("BRIDGE_ARMS_END")
            .expect("the BRIDGE_ARMS_END marker is gone — restore it in core_to_message");
        &rest[..end]
    }

    /// Pull every quoted command-id literal out of `src`'s match-arm
    /// lines (each starts, once trimmed, with `"` or — on a wrapped
    /// multi-id arm — with `|`). Deliberately tiny, no regex dependency
    /// — mirrors `keymap::menu_command_tests`'s `ids_from_call`.
    /// Every id `core_to_message` can resolve: the match arms it names
    /// literally, plus the Active Bar table its fallthrough consults.
    /// A source scan alone would miss the table and report all 59 Active
    /// Bar commands as dead.
    fn resolvable_command_ids(src: &str) -> HashSet<String> {
        let mut ids: HashSet<String> = bridged_command_ids(src).into_iter().collect();
        ids.extend(crate::app::command::active_bar::command_ids().map(str::to_string));
        ids
    }

    fn bridged_command_ids(src: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for line in match_block(src).lines() {
            // Once an arm lists enough ids to pass the width limit,
            // rustfmt puts each one on its own line starting with `|`.
            // Reading only `"`-lines drops those ids, and the ratchet
            // then reports live commands as dead — it fails loudly
            // rather than silently, but it still fails.
            let trimmed = line.trim_start();
            if !trimmed.starts_with('"') && !trimmed.starts_with('|') {
                continue;
            }
            // Only the pattern side of an arm can name a command id.
            // Everything past `=>` is the body — a trailing comment or a
            // `&str` argument there is not an id, and scanning it would
            // mark a command bridged that has no arm. That failure is
            // SILENT: the ratchet would pass while the shortcut stays
            // dead, which is the one thing it exists to prevent.
            let mut rest = trimmed.split_once("=>").map_or(trimmed, |(head, _)| head);
            while let Some(open) = rest.find('"') {
                let after = &rest[open + 1..];
                match after.find('"') {
                    Some(close) => {
                        ids.push(after[..close].to_string());
                        rest = &after[close + 1..];
                    }
                    None => break,
                }
            }
        }
        ids
    }

    /// Drift guard: every id `core_to_message` matches must resolve in
    /// the command catalog via [`metadata_for`]. `menu_command_tests`
    /// guards the menu *views*; this is the analogous guard for the
    /// bridge itself, which nothing scanned before this slice.
    #[test]
    fn every_bridged_command_id_resolves_in_the_catalog() {
        let ids = bridged_command_ids(BRIDGE_SRC);
        assert!(
            ids.len() >= 59,
            "bridge scan found only {} command ids — core_to_message's \
             match arms may have drifted from this source scan",
            ids.len()
        );

        let orphans: Vec<String> = ids
            .into_iter()
            .filter(|id| {
                AppCommandId::new(id.as_str())
                    .ok()
                    .and_then(|command| metadata_for(&command))
                    .is_none()
            })
            .collect();

        assert!(
            orphans.is_empty(),
            "core_to_message maps command ids with no CommandMetadata \
             entry (add them to keymap/catalog): {orphans:?}"
        );
    }

    /// Catalog ids that `core_to_message` has no arm for, as measured on
    /// `trunk` @ `f211e7d1` (2026-07-25). Every one of them is bound to a
    /// trigger in the shipped `assets/keyboard-shortcuts/{altium,classic}.toml`
    /// (a key sequence or a pointer gesture), so the user presses
    /// the key, `take_keymap_match` consumes the stroke, and nothing happens.
    ///
    /// This list may only SHRINK. Adding a catalog entry without a bridge
    /// arm, or deleting an arm, fails `unmapped_command_ids_only_shrink`.
    /// Wiring one up means deleting its line here, in the same commit.
    ///
    /// The ids listed there are a mix, and the distinction decides the fix:
    /// - **unimplemented** — the catalog entry and the binding are the only
    ///   occurrences in the tree (`break_wire`,
    ///   `place_global_label`, `place_hierarchical_label`,
    ///   `repeat_last_item`, `rubber_stamp_copy`). A feature, not an arm.
    ///   This is most of what is left. It also covers ids whose obvious
    ///   target turns out to be a STUB, where an arm would trade a logged
    ///   warning for a silent no-op. Read the handler body, not just its
    ///   existence.
    /// - **needs an argument** — the target `Message` carries data this
    ///   entry point cannot supply. `select_expand_connection` wants
    ///   `SelectionRequest::SelectConnected { world_x, world_y }`, and a
    ///   bare id has no cursor. Unblocked by `dispatch(id, args)` (#367),
    ///   not by a match arm.
    /// - **not addressable from a bare id** — the operation exists but
    ///   only in a form this entry point cannot name. `DockMessage` does
    ///   have `ClosePanel` and `ToggleCollapse`, but both are addressed
    ///   by `(PanelPosition, index)`; the panel toggles go through the
    ///   kind-addressed `OverlayMsg::PanelToggled`. `unselect_all` is
    ///   stranded for the opposite reason: every `selected.clear()` in the tree is a step inside
    ///   some larger operation, never a message of its own.
    ///
    /// Full analysis:
    /// `docs/audit/command-registry-action-surface-2026-07-25.md` §5.
    #[rustfmt::skip]
    const UNMAPPED_CATALOG_IDS: &[&str] = &[
        "autoplace_fields", "break_wire", "center_on_cursor", "clear_net_highlighting",
        "close_active_document", "copy_attributes_or_add_vertex", "cycle_fast_grid",
        "cycle_wiring_mode",
        "edit_footprint_field",
        "edit_library_symbol", "edit_reference_designator",
        "edit_selected_symbol_in_symbol_editor",
        "edit_text_in_place", "edit_value", "fast_grid_1", "fast_grid_2",
        "highlight_net_under_cursor",
        "highlight_related_net_objects", "import_graphics",
        "next_highlighted_net_item", "open_datasheet",
        "place_design_block",
        "place_global_label", "place_hierarchical_label", "place_junction",
        "place_power_symbol",
        "previous_highlighted_net_item",
        "refresh_view",
        "reset_local_coordinates", "rubber_stamp_copy", "select_expand_connection",
        "select_node_or_connection_item", "sheet_navigation_back", "sheet_navigation_forward",
        "switch_segment_posture", "toggle_cross_select_mode", "toggle_floating_panels",
        "toggle_search_panel",
        "undo_last_segment",
        // Forward annotation does not exist yet. Aliasing this to ERC made the
        // update-PCB shortcut run a rule check under the wrong name (#1247).
        "update_pcb_from_schematic",
    ];

    /// Coverage ratchet: the set of catalog ids with no dispatch arm may
    /// only shrink.
    ///
    /// The bridge's `_ => return None` fallthrough makes an unmapped id
    /// indistinguishable from a mapped one at the call site — the keymap
    /// resolves it, consumes the stroke, and no-ops. Nothing measured that
    /// gap before this test, so it grew unnoticed.
    ///
    /// Deliberately two assertions rather than one set equality: the
    /// "no longer unmapped" direction is a fix and gets its own message
    /// telling you to update the list, while the "newly unmapped"
    /// direction is the regression this exists to catch.
    #[test]
    fn unmapped_command_ids_only_shrink() {
        let bridged = resolvable_command_ids(BRIDGE_SRC);
        let pinned: HashSet<&str> = UNMAPPED_CATALOG_IDS.iter().copied().collect();

        let newly_unmapped: Vec<&'static str> = crate::keymap::all_command_ids()
            .filter(|id| !bridged.contains(*id) && !pinned.contains(id))
            .collect();
        assert!(
            newly_unmapped.is_empty(),
            "these command ids are in the catalog with no core_to_message \
             arm, and are not pinned: {newly_unmapped:?}. A catalog entry \
             whose key binding silently no-ops is a dead shortcut — add the \
             dispatch arm, or add the id to UNMAPPED_CATALOG_IDS with a note \
             saying why it cannot be wired yet."
        );

        let now_mapped: Vec<&str> = UNMAPPED_CATALOG_IDS
            .iter()
            .copied()
            .filter(|id| bridged.contains(*id))
            .collect();
        assert!(
            now_mapped.is_empty(),
            "these ids now have a dispatch arm but are still pinned as \
             unmapped: {now_mapped:?}. Delete them from \
             UNMAPPED_CATALOG_IDS in the same commit that wires them up — \
             the ratchet only means anything if it tightens."
        );
    }

    /// The ratchet proves the seven ids this slice drained now *resolve*;
    /// it cannot tell a right arm from a wrong one. A typo that sent
    /// `place_no_connect` to `Tool::Wire` would leave the ratchet green
    /// and the shortcut quietly drawing wires, so pin each target here.
    #[test]
    fn the_newly_wired_ids_reach_their_named_messages() {
        fn resolve(id: &str) -> Message {
            let command = AppCommandId::new(id).expect("a catalog id is a valid AppCommandId");
            core_to_message(&command).unwrap_or_else(|| panic!("'{id}' no longer resolves"))
        }

        assert!(matches!(
            resolve("draw_graphic_line"),
            Message::Tool(ToolMessage::SelectTool(Tool::Line))
        ));
        assert!(matches!(
            resolve("edit_selected_object_properties"),
            Message::Menu(MenuMessage::OpenPropertiesPanel)
        ));
        assert!(matches!(
            resolve("open_schematic_preferences"),
            Message::Preferences(PreferencesMsg::Open)
        ));
        assert!(matches!(
            resolve("place_no_connect"),
            Message::Tool(ToolMessage::SelectTool(Tool::NoConnect))
        ));
        assert!(matches!(
            resolve("place_wire_to_bus_entry"),
            Message::Tool(ToolMessage::SelectTool(Tool::BusEntry))
        ));
        // The BOM command opens the BOM dialog (`ExportBom`).
        assert!(matches!(
            resolve("report_manager_bom"),
            Message::Menu(MenuMessage::ExportBom)
        ));
        assert!(matches!(
            resolve("zoom_to_all_objects"),
            Message::CanvasEvent(CanvasEvent::FitAll)
        ));
        // #525: centring is not a fit.
        assert!(matches!(
            resolve("center_view_at_cursor"),
            Message::CanvasEvent(CanvasEvent::CenterAtCursor)
        ));
    }

    /// #103 - the cheap shortcuts that used to be dead keys now reach
    /// the handlers that already exist.
    #[test]
    fn the_103_shortcuts_reach_their_named_messages() {
        fn resolve(id: &str) -> Message {
            let command = AppCommandId::new(id).expect("a catalog id is a valid AppCommandId");
            core_to_message(&command).unwrap_or_else(|| panic!("'{id}' no longer resolves"))
        }

        assert!(matches!(
            resolve("zoom_in_at_cursor"),
            Message::Menu(MenuMessage::ZoomIn)
        ));
        assert!(matches!(
            resolve("zoom_out_at_cursor"),
            Message::Menu(MenuMessage::ZoomOut)
        ));
        assert!(matches!(
            resolve("next_sheet"),
            Message::Menu(MenuMessage::NextSheet)
        ));
        assert!(matches!(
            resolve("previous_sheet"),
            Message::Menu(MenuMessage::PreviousSheet)
        ));
        assert!(matches!(
            resolve("unselect_all"),
            Message::Selection(selection_request::SelectionRequest::ClearAll)
        ));
    }

    /// #271 — `move_object` (Altium's `M M`) resolves to the interactive
    /// Move, not to Move Selection or an unmapped no-op.
    #[test]
    fn move_object_resolves_to_the_interactive_move_action() {
        let command = AppCommandId::new("move_object").expect("a catalog id is valid");
        assert!(matches!(
            core_to_message(&command),
            Some(Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::Move
            )))
        ));
    }

    /// The pinned ids must still exist in the catalog, so the list cannot
    /// rot into a set of names that no longer mean anything.
    #[test]
    fn pinned_unmapped_ids_still_exist_in_the_catalog() {
        let catalog: HashSet<&str> = crate::keymap::all_command_ids().collect();
        let stale: Vec<&str> = UNMAPPED_CATALOG_IDS
            .iter()
            .copied()
            .filter(|id| !catalog.contains(id))
            .collect();
        assert!(
            stale.is_empty(),
            "UNMAPPED_CATALOG_IDS names ids that are no longer in the \
             catalog: {stale:?}. Remove them from the pinned list."
        );
    }

    /// The diagnostics ring is one process-wide `Mutex<VecDeque<_>>`
    /// (`diagnostics.rs`), and cargo runs the `signex-app --lib` tests in
    /// parallel threads. The two reporting tests below both read it around
    /// an action, so they have to take turns — otherwise one sees the
    /// other's record land between its `before` and its assert.
    ///
    /// Poison is recovered rather than propagated: a panic in one of these
    /// two is a real failure to report, not a reason to fail the other one
    /// for an unrelated reason.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Records currently in the ring that name this command id.
    ///
    /// Counting *total* entries instead would make these tests depend on
    /// whatever else the binary happened to log in a neighbouring thread —
    /// which is exactly how the first version of
    /// `asking_whether_a_command_resolves_reports_nothing` failed in CI
    /// (`left: 2, right: 1`). The claim under test is about records naming
    /// this id, so count those.
    fn records_naming(id: &str) -> usize {
        let needle = format!("command '{id}'");
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|entry| entry.message.contains(&needle))
            .count()
    }

    /// #619 — a query must not narrate. The palette filters its rows by
    /// asking whether each catalog id resolves; routing that through the
    /// dispatch entry point put one warning per unmapped id into the
    /// Messages panel on every rebuild — 2432 records in one observed
    /// session, against a 200-entry ring buffer, so every real
    /// diagnostic was evicted before it could be read.
    #[test]
    fn asking_whether_a_command_resolves_reports_nothing() {
        // Arrange — an id the catalog carries with no dispatch arm, i.e.
        // exactly what the palette filters out.
        let _guard = serial();
        let _ = crate::diagnostics::init_logging();
        let id: &str = UNMAPPED_CATALOG_IDS
            .first()
            .expect("the pinned list is non-empty while any id is unmapped");
        let command = AppCommandId::new(id).expect("a pinned id is a valid command id");
        let before = records_naming(id);

        // Act — the query, run as often as a few palette rebuilds would.
        for _ in 0..25 {
            assert!(
                !is_dispatchable(&command),
                "{id} is pinned as unmapped, so it must not resolve"
            );
        }

        // Assert
        assert_eq!(
            records_naming(id),
            before,
            "asking whether `{id}` resolves must leave the Messages panel untouched"
        );
    }

    /// #1350 - Backspace/Delete mid-wire pops the pending vertex; it must not
    /// be a plain delete-selection alias.
    #[test]
    fn remove_last_vertex_ends_a_wire_draw_instead_of_deleting() {
        let (mut app, _task) = Signex::new();
        let cmd = AppCommandId::new("remove_last_vertex").unwrap();
        let msg = core_to_message(&cmd).unwrap();
        assert!(matches!(msg, Message::Tool(ToolMessage::RemoveLastVertex)));
        assert!(app.interaction_state.active_canvas().selected.is_empty());
        app.interaction_state.wire_drawing = true;
        app.interaction_state
            .wire_points
            .push(signex_types::schematic::Point::new(0.0, 0.0));

        let _ = app.dispatch_command(&cmd, crate::app::command::args::CommandArgs::none(), None);

        assert!(!app.interaction_state.wire_drawing);
        assert!(app.interaction_state.wire_points.is_empty());
    }

    /// #1350 - Backspace mid-polyline pops the last buffered point and
    /// leaves the selection alone.
    #[test]
    fn remove_last_vertex_pops_a_polyline_point_and_keeps_selection() {
        let (mut app, _task) = Signex::new();
        let cmd = AppCommandId::new("remove_last_vertex").unwrap();
        let pts = vec![
            signex_types::schematic::Point::new(0.0, 0.0),
            signex_types::schematic::Point::new(5.0, 0.0),
        ];
        app.interaction_state.polyline_points = pts.clone();
        app.interaction_state.active_canvas_mut().polyline_points = pts;
        app.interaction_state.active_canvas_mut().selected.push(
            signex_types::schematic::SelectedItem::new(
                uuid::Uuid::new_v4(),
                signex_types::schematic::SelectedKind::Wire,
            ),
        );

        let _ = app.dispatch_command(&cmd, crate::app::command::args::CommandArgs::none(), None);

        assert_eq!(app.interaction_state.polyline_points.len(), 1);
        assert_eq!(
            app.interaction_state.active_canvas().polyline_points.len(),
            1
        );
        assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
    }

    /// #1247 - the update-PCB shortcut must not be an alias for ERC. Until
    /// forward annotation exists it has no arm, so it is unavailable like any
    /// other unmapped command, while Run ERC keeps its own arm.
    #[test]
    fn update_pcb_from_schematic_does_not_run_erc() {
        let update_pcb = AppCommandId::new("update_pcb_from_schematic").unwrap();
        let run_erc = AppCommandId::new("run_erc").unwrap();

        assert!(!is_dispatchable(&update_pcb));
        assert!(matches!(resolve(&run_erc), Some(Message::Erc(ErcMsg::Run))));
    }

    /// #103 - ids that gained an existing handler resolve; the rest of the
    /// pinned set stays unresolved.
    #[test]
    fn the_103_wired_ids_resolve_to_their_handlers() {
        for (id, expect_properties) in [
            ("edit_object_properties", true),
            ("leave_sheet", false),
            ("navigate_up_hierarchy", false),
        ] {
            let command = AppCommandId::new(id).unwrap();
            let message = resolve(&command).unwrap_or_else(|| panic!("'{id}' must resolve"));
            if expect_properties {
                assert!(matches!(
                    message,
                    Message::Menu(MenuMessage::OpenPropertiesPanel)
                ));
            } else {
                assert!(matches!(
                    message,
                    Message::Menu(MenuMessage::AscendToParentSheet)
                ));
            }
        }
        for id in UNMAPPED_CATALOG_IDS {
            assert!(!is_dispatchable(&AppCommandId::new(*id).unwrap()), "{id}");
        }
    }

    /// #1256 / #1257 / #1266 - the canvas commands resolve to their messages.
    #[test]
    fn the_canvas_commands_resolve() {
        let resolve_id = |id: &str| resolve(&AppCommandId::new(id).unwrap());
        assert!(matches!(
            resolve_id("measure_distance"),
            Some(Message::Tool(ToolMessage::SelectTool(Tool::Measure)))
        ));
        assert!(matches!(
            resolve_id("find_similar_objects"),
            Some(Message::Selection(
                selection_request::SelectionRequest::FindSimilar(
                    crate::find_similar::FindSimilarMsg::OpenRequested { at: None }
                )
            ))
        ));
        assert!(matches!(
            resolve_id("drag_keep_connections"),
            Some(Message::Selection(
                selection_request::SelectionRequest::DragKeepingConnections
            ))
        ));
        assert!(matches!(
            resolve_id("zoom_to_selection_area"),
            Some(Message::CanvasEvent(CanvasEvent::FitSelection))
        ));
    }

    /// #103 - pressing an unmapped command's key says "<label> is not
    /// available yet" in the Messages panel instead of doing nothing.
    #[test]
    fn an_unmapped_command_reports_that_it_is_not_available_yet() {
        let _guard = serial();
        let _ = crate::diagnostics::init_logging();
        let command = AppCommandId::new("break_wire").unwrap();

        assert!(core_to_message(&command).is_none());

        assert!(crate::diagnostics::recent_entries().iter().any(|entry| {
            entry.message.contains("command 'break_wire'")
                && entry.message.contains("Break wire is not available yet")
        }));
    }

    /// The other half: dispatching an unmapped command still reports, so
    /// the split silenced the query and not the failure.
    #[test]
    fn dispatching_an_unmapped_command_still_reports_it() {
        // Arrange
        let _guard = serial();
        let _ = crate::diagnostics::init_logging();
        let id: &str = UNMAPPED_CATALOG_IDS
            .first()
            .expect("the pinned list is non-empty while any id is unmapped");
        let command = AppCommandId::new(id).expect("a pinned id is a valid command id");
        let before = records_naming(id);

        // Act
        assert!(core_to_message(&command).is_none());

        // Assert — exactly one new record, so a future change that reports
        // the same failure twice is caught too.
        assert_eq!(
            records_naming(id),
            before + 1,
            "dispatching `{id}` must still say it did nothing"
        );
    }
}
