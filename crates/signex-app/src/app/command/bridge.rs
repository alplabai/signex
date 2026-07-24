//! The app's single id→[`Message`] bridge.
//!
//! [`core_to_message`] turns a stable `AppCommandId` (the id a profile
//! TOML binds a key to) into the app's namespaced [`Message`] tree
//! (ADR-0001 D3). It is not keymap-specific — menus, the command
//! palette, and a future CLI all route a command id through here on
//! their way to `update`.

use crate::keymap::AppCommandId;

use super::super::*;

/// Map a stable command id onto the app's namespaced [`Message`] tree.
///
/// Commands without a live dispatch arm return `None` (they resolve in
/// the keymap but no-op) — that gap is the deferred catalog/dispatch
/// triplication follow-up, not a regression here.
pub(crate) fn core_to_message(command: &AppCommandId) -> Option<Message> {
    use crate::library::editor::footprint::state::EditorMode;

    let message = match command.as_str() {
        "annotate_schematic" => Message::Annotate(AnnotateMsg::OpenDialog),
        "annotate_schematic_quietly" => {
            Message::Annotate(AnnotateMsg::Run(signex_engine::AnnotateMode::Incremental))
        }
        "cancel_current_tool" => Message::EscapePressed,
        "center_view_at_cursor" | "show_all_design_objects" | "zoom_to_fit" => {
            Message::CanvasEvent(CanvasEvent::FitAll)
        }
        "copy" => Message::Edit(EditMsg::Copy),
        "cycle_selection_mode" => Message::CycleSelectionMode,
        "cycle_snap_grid_forward" | "open_grid_picker" => Message::Ui(UiMsg::GridPickerOpen),
        "cycle_unit" => Message::Ui(UiMsg::UnitCycled),
        "cycle_wire_bus_graphic_mode" => Message::Tool(ToolMessage::CycleDrawMode),
        "cut" => Message::Edit(EditMsg::Cut),
        "delete_selection" | "remove_last_vertex" => Message::Edit(EditMsg::DeleteSelected),
        "duplicate" => Message::Edit(EditMsg::Duplicate),
        "find" | "find_text" => Message::Overlay(OverlayMsg::OpenFind),
        "find_and_replace" => Message::Overlay(OverlayMsg::OpenReplace),
        "footprint_mode_pads" => Message::FootprintModeShortcut(EditorMode::Normal),
        "footprint_mode_sketch" => Message::FootprintModeShortcut(EditorMode::Sketch),
        "footprint_mode_view_3d" => Message::FootprintModeShortcut(EditorMode::View3d),
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
        "open_preferences" => Message::Preferences(PreferencesMsg::Open),
        "paste" => Message::Edit(EditMsg::Paste),
        "paste_special" | "smart_paste" => Message::Edit(EditMsg::SmartPaste),
        "place_bus" => Message::Tool(ToolMessage::SelectTool(Tool::Bus)),
        "place_local_net_label" | "place_net_label" => {
            Message::Tool(ToolMessage::SelectTool(Tool::Label))
        }
        "place_text" => Message::Tool(ToolMessage::SelectTool(Tool::Text)),
        "place_wire" => Message::Tool(ToolMessage::SelectTool(Tool::Wire)),
        "placement_accept" => Message::LassoCommit,
        "placement_properties" => Message::Tool(ToolMessage::PrePlacementTab),
        "print" => Message::PrintPreview(PrintPreviewMsg::Requested),
        "redo" => Message::Edit(EditMsg::Redo),
        "reset_schematic_designators" => Message::Annotate(AnnotateMsg::OpenResetConfirm),
        "rotate_clockwise" | "rotate_counterclockwise" => Message::Edit(EditMsg::RotateSelected),
        "run_erc" | "update_pcb_from_schematic" => Message::Erc(ErcMsg::Run),
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
        _ => return None,
    };
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::metadata_for;

    /// `core_to_message`'s own source, embedded at compile time so the
    /// guard below scans the exact arms it checks instead of a
    /// hand-kept duplicate list ("kept in step" copies are banned).
    const BRIDGE_SRC: &str = include_str!("bridge.rs");

    /// Pull every quoted command-id literal out of `src`'s match-arm
    /// lines (each starts, once trimmed, with `"`). Deliberately tiny,
    /// no regex dependency — mirrors `keymap::menu_command_tests`'s
    /// `ids_from_call`.
    fn bridged_command_ids(src: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for line in src.lines() {
            if !line.trim_start().starts_with('"') {
                continue;
            }
            let mut rest = line;
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
}
