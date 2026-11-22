//! The commands Replay may run (Caner, 2026-11-22, #2840). Default deny: a
//! command not listed here is refused, a new one included. Each is named by
//! stable id, writes an absolute value in stored coordinates, and has every
//! input covered by its target check. The proof of each row:
//! `docs/COMMAND_LOG.md`, "Replay".

/// How a listed command proves its target unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::handlers) enum ReplayProof {
    /// A recorded edit that stores whole objects: a sheet's drawing,
    /// directive or harness connector against its pre-image, or board
    /// commands against their guard.
    RecordedObjects,
    /// Operands set one board value; the guard covers it. On its own result
    /// it changes nothing.
    BoardValue,
    /// As [`Self::BoardValue`], on the items `ids` names, every one of which
    /// the guard must hold: an item already at the value wrote nothing.
    BoardItems,
    /// On its own result it would do something else, so it runs only on the
    /// board as it found it.
    BoardAsFound,
}

/// Every command Replay may run.
pub(in crate::app::handlers) const SAFE_COMMANDS: &[(&str, ReplayProof)] = &[
    ("edit_object_properties", ReplayProof::RecordedObjects),
    ("lock_board_items", ReplayProof::BoardItems),
    ("unlock_board_items", ReplayProof::BoardItems),
    ("set_pad_shape", ReplayProof::BoardItems),
    ("panelize_board", ReplayProof::BoardValue),
    ("remove_board_panel", ReplayProof::BoardValue),
    ("set_board_panel", ReplayProof::BoardValue),
    ("set_layer_color", ReplayProof::BoardValue),
    ("reset_layer_colors", ReplayProof::BoardValue),
    ("set_layer_stack", ReplayProof::BoardAsFound),
];

/// The proof of the command `id`; `None` refuses it.
pub(in crate::app::handlers) fn replay_proof(id: &str) -> Option<ReplayProof> {
    SAFE_COMMANDS
        .iter()
        .find(|(listed, _)| *listed == id)
        .map(|(_, proof)| *proof)
}
