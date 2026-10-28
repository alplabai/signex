//! A complete shortcut that is also the prefix of a longer chord (`T A` /
//! `T A E`, #1378) cannot fire the moment it is typed, or the longer chord
//! could never be reached. It waits instead, and this module owns the ways
//! that wait ends without a further key:
//!
//! * [`CHORD_TIMEOUT`] elapses — the waiting command fires;
//! * Esc, a pointer press, a window focus change, a modal, or a key that
//!   is not a keymap stroke — the chord is dropped, nothing fires.
//!
//! The next-key resolution lives in `keymap.rs`.

use std::time::Duration;

use iced::Task;

use super::super::*;

/// How long a complete-but-ambiguous chord waits for the next key before
/// it fires. Long enough to type the second key of a two-key chord, short
/// enough that `T A` does not feel dead. A constant rather than a
/// preference: reading one would put IO on the key path.
pub(crate) const CHORD_TIMEOUT: Duration = Duration::from_millis(800);

/// A pending chord whose exact match is waiting for [`CHORD_TIMEOUT`].
///
/// `generation` is part of the timer subscription's identity, so each new
/// wait gets a fresh timer rather than inheriting a half-elapsed one, and
/// a late tick from an earlier wait is recognised and ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChordWait {
    pub generation: u64,
    pub window: Option<iced::window::Id>,
}

/// Whether a raw window event abandons a pending chord: any pointer press,
/// or the window gaining or losing focus.
pub(crate) fn chord_cancelling_event(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Window(iced::window::Event::Focused | iced::window::Event::Unfocused)
    )
}

impl Signex {
    /// Drop the pending chord and any wait on it, firing nothing.
    pub(crate) fn cancel_pending_chord(&mut self) {
        self.ui_state.keymap_pending_sequence.clear();
        self.ui_state.keymap_pending_target = None;
        self.ui_state.keymap_chord_wait = None;
    }

    /// Start waiting on the pending chord's exact match.
    pub(super) fn arm_chord_wait(&mut self, window: Option<iced::window::Id>) {
        self.ui_state.keymap_chord_generation += 1;
        self.ui_state.keymap_chord_wait = Some(ChordWait {
            generation: self.ui_state.keymap_chord_generation,
            window,
        });
    }

    /// [`CHORD_TIMEOUT`] elapsed for wait `generation`: fire the waiting
    /// command, unless the wait is stale or a blocking modal now owns the
    /// screen (a held command must never run behind one).
    pub(crate) fn resolve_chord_timeout(&mut self, generation: u64) -> Task<Message> {
        let Some(wait) = self.ui_state.keymap_chord_wait else {
            return Task::none();
        };
        if wait.generation != generation {
            return Task::none();
        }
        let held = std::mem::take(&mut self.ui_state.keymap_pending_sequence);
        self.cancel_pending_chord();
        if self.input_target(wait.window).paints_overlay_stack() && self.has_blocking_modal() {
            return Task::none();
        }
        let contexts = self.shortcut_contexts(wait.window);
        let Some(command) = self.ui_state.active_keymap.lookup(&held, &contexts).command else {
            return Task::none();
        };
        self.dispatch_command(
            &command,
            crate::app::command::CommandArgs::none(),
            wait.window,
        )
    }
}

#[cfg(test)]
mod tests;
