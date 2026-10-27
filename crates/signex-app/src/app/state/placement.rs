//! Content attached to the cursor: the app-wide placement session.
//!
//! There is exactly ONE session app-wide, like `current_tool` and the other
//! placement buffers (`actions.rs`, `clear_transient_schematic_tool_state`):
//! a part picked from an undocked window's panel arms the same field. The
//! session names the sheet it was armed for; a click on any other sheet
//! never places into it, and only the window showing that sheet draws the
//! ghost.

use std::path::PathBuf;

use signex_engine::PlacementPayload;

use crate::canvas::PlacementGhost;

pub struct PlacementSession {
    /// What a click places, with its current orientation.
    pub payload: PlacementPayload,
    /// The sheet the session was armed for.
    pub target_sheet: PathBuf,
    /// A click leaves the session armed, so the next click places another
    /// copy (until Esc or a right-click).
    pub repeat: bool,
    /// A designator typed in the Tab form: used by the next click, then
    /// stepped. `None` means the next free number for the prefix.
    pub reference_override: Option<String>,
    /// `InteractionState::placement_epoch` when this session was armed.
    pub arm_epoch: u64,
    /// Designator letters (`R`, `U`, `D`) the next number is counted for.
    pub prefix: String,
    /// The payload as drawn at the cursor; rebuilt whenever the payload
    /// changes.
    pub ghost: PlacementGhost,
}

impl PlacementSession {
    /// A repeating session for `payload` on `target_sheet`.
    #[must_use]
    pub fn new(
        payload: PlacementPayload,
        target_sheet: PathBuf,
        prefix: String,
        arm_epoch: u64,
    ) -> Self {
        let ghost = PlacementGhost::new(payload.fragment().clone());
        Self {
            payload,
            target_sheet,
            repeat: true,
            reference_override: None,
            arm_epoch,
            prefix,
            ghost,
        }
    }

    /// Replace the payload (a turn, a Tab edit, the next designator) and
    /// redraw the ghost from it.
    pub fn set_payload(&mut self, payload: PlacementPayload) {
        self.ghost = PlacementGhost::new(payload.fragment().clone());
        self.payload = payload;
    }
}
