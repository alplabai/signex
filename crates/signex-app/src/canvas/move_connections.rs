//! Whether one move gesture keeps wires attached (#1266).
//!
//! The Preferences checkbox "Keep wires connected when moving" is the
//! default. A gesture can override it: holding Alt through a drag flips it,
//! and the `drag_keep_connections` command (G) always keeps the wires.

use super::*;

/// Modifier that flips the wire behaviour for the drag it is held through.
/// Ctrl and Shift already toggle and extend the selection on a press, so they
/// are not free for a drag.
pub const FLIP_WIRES_MODIFIER: &str = "Alt";

/// How one move gesture treats connected wires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoveConnections {
    /// The `move_keeps_wires` preference decides.
    #[default]
    Preference,
    /// The preference, inverted for this gesture (the flip modifier was held).
    Flipped,
    /// Wires stay attached whatever the preference says.
    Keep,
}

impl MoveConnections {
    /// Whether wires follow the move, given the preference.
    pub fn keeps_wires(self, preference: bool) -> bool {
        match self {
            Self::Preference => preference,
            Self::Flipped => !preference,
            Self::Keep => true,
        }
    }
}

impl CanvasState {
    /// How the gesture in flight treats wires: a drag started by the keep
    /// command always keeps them, otherwise the flip modifier decides.
    pub(in crate::canvas) fn move_connections(&self) -> MoveConnections {
        let started_as = self.move_float.as_ref().map(|float| float.connections);
        match started_as {
            Some(MoveConnections::Keep) => MoveConnections::Keep,
            _ if self.alt_held => MoveConnections::Flipped,
            _ => MoveConnections::Preference,
        }
    }
}

impl SchematicCanvas<'_> {
    /// Whether the gesture in flight rubber-bands wires.
    pub(in crate::canvas) fn keeps_wires(&self, state: &CanvasState) -> bool {
        state
            .move_connections()
            .keeps_wires(self.prefs.move_keeps_wires)
    }

    /// Mirror the live behaviour into the slot so the status bar can show it.
    /// Called after every canvas event, so it cannot go stale.
    pub(in crate::canvas) fn publish_move_behaviour(&self, state: &CanvasState) {
        let behaviour = state.move_dragging.then(|| self.keeps_wires(state));
        self.move_behaviour.set(behaviour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_always_keeps_and_flipped_inverts_the_preference() {
        for preference in [true, false] {
            assert_eq!(
                MoveConnections::Preference.keeps_wires(preference),
                preference
            );
            assert_eq!(
                MoveConnections::Flipped.keeps_wires(preference),
                !preference
            );
            assert!(MoveConnections::Keep.keeps_wires(preference));
        }
    }

    #[test]
    fn holding_the_modifier_flips_the_state_connections() {
        let mut state = CanvasState::default();
        assert_eq!(state.move_connections(), MoveConnections::Preference);
        state.alt_held = true;
        assert_eq!(state.move_connections(), MoveConnections::Flipped);
    }
}
