//! Keymap chord resolution.
//!
//! The keyboard subscription forwards each raw keystroke as
//! [`UiMsg::KeymapStroke`]. Resolution happens here, in `update`, where
//! `&mut self` is available — so the multi-stroke chord buffer lives in
//! [`UiState::keymap_pending_sequence`] instead of a process-global
//! static (sound across multiple windows, MVU-clean).
//!
//! A resolved command id is turned into a [`Message`] by
//! [`crate::app::command::core_to_message`] — the app's single
//! id→`Message` bridge, no longer owned by this module.

use iced::Task;

use super::super::*;
use crate::keymap::{KeyStroke, ShortcutContext};

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
            return Some(match crate::app::command::core_to_message(command) {
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
