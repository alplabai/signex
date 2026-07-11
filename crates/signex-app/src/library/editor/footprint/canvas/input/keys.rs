//! Keyboard input — modifier tracking, clipboard shortcuts, Space /
//! X (rotate / flip) on the selected pad, and the Sketch-mode live
//! numeric placement input.
//!
//! The catch-all char forwarding reads the first codepoint of the
//! event's platform `text`; the dispatcher extracts it so this module
//! never has to name iced's `SmolStr` type.

use iced::keyboard;
use iced::widget::canvas;

use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};

use super::super::{FootprintCanvas, FootprintCanvasState};

impl FootprintCanvas<'_> {
    /// v0.27 — mirror `ModifiersChanged` into `cstate` so the mouse
    /// press handlers can branch on Ctrl/Cmd + Shift (iced 0.14 mouse
    /// events don't carry modifiers). Returns None so the rest of the
    /// app still receives the event.
    pub(in crate::library::editor::footprint::canvas) fn on_modifiers_changed(
        &self,
        cstate: &mut FootprintCanvasState,
        mods: &keyboard::Modifiers,
    ) -> Option<canvas::Action<LibraryMessage>> {
        cstate.current_modifiers = *mods;
        None
    }

    /// v0.24 Track D — key handling: clipboard shortcuts, rotate / flip,
    /// then the Sketch-mode numeric placement input. `typed_char` is
    /// the first codepoint of the event's platform text (or None).
    pub(in crate::library::editor::footprint::canvas) fn on_key_pressed(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
        typed_char: Option<char>,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if let Some(a) = self.key_clipboard(key, modifiers) {
            return Some(a);
        }
        if let Some(a) = self.key_rotate_flip(key, modifiers) {
            return Some(a);
        }
        self.key_sketch_input(key, modifiers, typed_char)
    }

    /// v0.26-F — Ctrl+X / Ctrl+C / Ctrl+V clipboard shortcuts.
    /// Mode-agnostic (works in Normal AND Sketch). Captures the event
    /// so iced's global key subscription doesn't fire a duplicate.
    fn key_clipboard(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if modifiers.command()
            && !modifiers.shift()
            && !modifiers.alt()
            && let keyboard::Key::Character(c) = key.as_ref()
        {
            let cb_msg = match c {
                "x" | "X" => Some(EditorMsg::Footprint(FootprintEditorMsg::CutPad)),
                "c" | "C" => Some(EditorMsg::Footprint(FootprintEditorMsg::CopyPad)),
                "v" | "V" => Some(EditorMsg::Footprint(FootprintEditorMsg::PastePad)),
                _ => None,
            };
            if let Some(msg) = cb_msg {
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg,
                    })
                    .and_capture(),
                );
            }
        }
        None
    }

    /// v0.26-G — Space (rotate 90°) / X (flip layer) on the selected
    /// pad. Altium parity; only fires when there's a pad to act on so
    /// the canvas doesn't swallow Space / X from sketch tools.
    fn key_rotate_flip(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if !modifiers.command() && !modifiers.alt() {
            if matches!(key, keyboard::Key::Named(keyboard::key::Named::Space))
                && self.state.selected_pad.is_some()
            {
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg: EditorMsg::Footprint(FootprintEditorMsg::ActiveBarRotateSelection),
                    })
                    .and_capture(),
                );
            }
            if let keyboard::Key::Character(c) = key.as_ref()
                && (c == "x" || c == "X")
                && self.state.selected_pad.is_some()
            {
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg: EditorMsg::Footprint(FootprintEditorMsg::ActiveBarFlipSelection),
                    })
                    .and_capture(),
                );
            }
        }
        None
    }

    /// v0.24 Track D — Sketch-mode live numeric placement input.
    /// Active only while a multi-click sketch tool has its first click
    /// pending or a buffer is open, so digit keys outside Sketch mode
    /// never get swallowed. Modifiers must be empty so global
    /// shortcuts still reach the app dispatcher.
    fn key_sketch_input(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
        typed_char: Option<char>,
    ) -> Option<canvas::Action<LibraryMessage>> {
        use crate::library::editor::footprint::state::{EditorMode, PlacementInputKind, ToolPending};

        if !matches!(self.state.mode, EditorMode::Sketch) {
            return None;
        }
        // Only intercept when there's either an open buffer or an
        // in-progress gesture that could accept one.
        let has_open_buffer = self.state.placement_input.is_some();
        let kind_for_active = PlacementInputKind::from_active_tool(
            self.state.active_tool,
            &self.state.tool_pending,
        );
        if !has_open_buffer && kind_for_active.is_none() {
            return None;
        }
        if matches!(self.state.tool_pending, ToolPending::Idle) && !has_open_buffer {
            return None;
        }
        if modifiers.command() || modifiers.alt() || modifiers.logo() {
            return None;
        }
        let publish = |msg: EditorMsg| -> Option<canvas::Action<LibraryMessage>> {
            Some(
                canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg,
                })
                .and_capture(),
            )
        };
        match key {
            keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                if has_open_buffer {
                    return publish(EditorMsg::Footprint(
                        FootprintEditorMsg::SketchPlacementInputBackspace,
                    ));
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) => {
                if has_open_buffer {
                    return publish(EditorMsg::Footprint(
                        FootprintEditorMsg::SketchPlacementInputEnter,
                    ));
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                // v0.27 — Lasso Select cancel via Esc. Pre-empts the
                // placement-input Esc which also fires here.
                if self.state.lasso_mode_active {
                    return publish(EditorMsg::Footprint(FootprintEditorMsg::LassoCancel));
                }
                // v0.27 — Touching Line cancel via Esc.
                if self.state.touching_line_active {
                    return publish(EditorMsg::Footprint(FootprintEditorMsg::TouchingLineCancel));
                }
                if has_open_buffer {
                    return publish(EditorMsg::Footprint(
                        FootprintEditorMsg::SketchPlacementInputEscape,
                    ));
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Tab) => {
                // v0.14-footprint — Tab cycles dimension fields ONLY
                // while a buffer is open on a multi-field tool;
                // otherwise Tab keeps its placement-pause role.
                let switch_fields = self
                    .state
                    .placement_input
                    .as_ref()
                    .map(|p| p.kind.is_tab_switchable())
                    .unwrap_or(false);
                if switch_fields {
                    return publish(EditorMsg::Footprint(
                        FootprintEditorMsg::SketchPlacementInputTab,
                    ));
                }
                // No active dimension buffer — let Tab reach the global
                // pre-placement-pause subscription.
                return None;
            }
            _ => {
                // Use the platform-supplied `text` so we get exactly
                // the codepoint the user typed. Only forward digits /
                // `.` / `-`; everything else falls through.
                if let Some(ch) = typed_char {
                    let useful = ch.is_ascii_digit()
                        || ch == '.'
                        || (ch == '-'
                            && kind_for_active
                                .or_else(|| self.state.placement_input.as_ref().map(|p| p.kind))
                                .map(|k| k.allows_negative())
                                .unwrap_or(false));
                    if useful {
                        return publish(EditorMsg::Footprint(
                            FootprintEditorMsg::SketchPlacementInputChar(ch),
                        ));
                    }
                }
            }
        }
        None
    }
}
