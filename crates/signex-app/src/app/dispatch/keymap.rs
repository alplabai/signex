//! Keymap bridge + chord resolution.
//!
//! The keyboard subscription forwards each raw keystroke as
//! [`UiMsg::KeymapStroke`]. Resolution happens here, in `update`, where
//! `&mut self` is available — so the multi-stroke chord buffer lives in
//! [`UiState::keymap_pending_sequence`] instead of a process-global
//! static (sound across multiple windows, MVU-clean).
//!
//! [`message_for_keymap_command`] is the bridge from a stable
//! `AppCommandId` (the id a profile TOML binds a key to) onto the app's
//! namespaced [`Message`] tree (ADR-0001 D3). Only the commands with a
//! live dispatch arm are mapped; the remaining catalog entries resolve
//! to `None` and no-op (tracked separately as the catalog/dispatch
//! triplication follow-up).

use iced::Task;

use super::super::*;
use crate::keymap::{AppCommandId, KeyStroke, ShortcutContext};

impl Signex {
    /// Resolve one forwarded keystroke against the active keymap,
    /// accumulating multi-stroke chords in `keymap_pending_sequence`.
    ///
    /// A resolved command is dispatched through the normal
    /// [`Signex::dispatch_update`] path, so it behaves exactly as if the
    /// mapped message had been sent directly. A partial chord keeps the
    /// buffer and waits; a definite miss clears it (with a single-stroke
    /// restart retry so a stale prefix can't wedge later keys).
    pub(super) fn resolve_keymap_stroke(&mut self, stroke: KeyStroke) -> Task<Message> {
        let contexts = self.shortcut_contexts();
        self.ui_state.keymap_pending_sequence.push(stroke.clone());

        if let Some(task) = self.take_keymap_match(&contexts) {
            return task;
        }

        // Definite miss on the accumulated sequence. Restart from the
        // latest stroke alone so a stale prefix (e.g. an abandoned `P`)
        // can't swallow the next real shortcut.
        if self.ui_state.keymap_pending_sequence.len() > 1 {
            self.ui_state.keymap_pending_sequence.clear();
            self.ui_state.keymap_pending_sequence.push(stroke);
            if let Some(task) = self.take_keymap_match(&contexts) {
                return task;
            }
        }

        self.ui_state.keymap_pending_sequence.clear();
        Task::none()
    }

    /// Look the current pending sequence up in the active keymap.
    ///
    /// Returns `Some(task)` when the sequence is consumed — either it
    /// resolved to a command (dispatched), matched a binding with no
    /// dispatch arm yet (no-op), or is a live prefix of a longer chord
    /// (buffer kept, no-op). Returns `None` on a definite miss so the
    /// caller can apply its restart retry / fall through.
    fn take_keymap_match(&mut self, contexts: &[ShortcutContext]) -> Option<Task<Message>> {
        let lookup = self
            .ui_state
            .active_keymap
            .lookup(&self.ui_state.keymap_pending_sequence, contexts);

        if let Some(command) = lookup.command.as_ref() {
            self.ui_state.keymap_pending_sequence.clear();
            return Some(match message_for_keymap_command(command) {
                Some(message) => self.dispatch_update(message),
                None => Task::none(),
            });
        }
        if lookup.matched {
            self.ui_state.keymap_pending_sequence.clear();
            return Some(Task::none());
        }
        if lookup.pending {
            // Prefix of a longer chord — keep the buffer and wait.
            return Some(Task::none());
        }
        None
    }

    /// The active shortcut contexts, most-specific last. `Global` is
    /// always present; schematic / PCB / footprint / library layers are
    /// added based on the active tab so a preset can bind the same key
    /// differently per surface.
    fn shortcut_contexts(&self) -> Vec<ShortcutContext> {
        let mut contexts = vec![ShortcutContext::Global];
        if self.has_active_schematic() {
            contexts.push(ShortcutContext::Schematic);
        }
        if self.has_active_pcb() {
            contexts.push(ShortcutContext::Pcb);
        }
        match self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| &tab.kind)
        {
            Some(TabKind::FootprintEditor(_)) => {
                contexts.push(ShortcutContext::Footprint);
                contexts.push(ShortcutContext::Library);
            }
            Some(TabKind::SymbolEditor(_)) | Some(TabKind::LibraryBrowser(_)) => {
                contexts.push(ShortcutContext::Library);
            }
            _ => {}
        }
        contexts
    }
}

/// Map a stable keymap command id onto the app's namespaced message tree.
///
/// Commands without a live dispatch arm return `None` (they resolve in
/// the keymap but no-op) — that gap is the deferred catalog/dispatch
/// triplication, not a regression here.
fn message_for_keymap_command(command: &AppCommandId) -> Option<Message> {
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
        "force_annotate_all_schematics" => {
            Message::Annotate(AnnotateMsg::Run(signex_engine::AnnotateMode::ResetAndRenumber))
        }
        "mirror_x" => Message::Edit(EditMsg::MirrorSelectedX),
        "mirror_y" => Message::Edit(EditMsg::MirrorSelectedY),
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
