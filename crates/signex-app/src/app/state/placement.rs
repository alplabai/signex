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
    /// A designator typed in the Tab form: used by the next click, then
    /// stepped. `None` means the next free number for the prefix.
    pub reference_override: Option<String>,
    /// Designator letters (`R`, `U`, `D`) the next number is counted for.
    /// `None` for pasted content, whose designators are its own.
    pub prefix: Option<String>,
    /// A click ends the session (paste). Otherwise it stays armed, so the
    /// next click places another copy, until Esc or a right-click.
    pub one_shot: bool,
    /// The payload as drawn at the cursor; rebuilt whenever the payload
    /// changes.
    pub ghost: PlacementGhost,
}

impl PlacementSession {
    /// A session for `payload` on `target_sheet`.
    #[must_use]
    pub fn new(
        payload: PlacementPayload,
        target_sheet: PathBuf,
        prefix: Option<String>,
        one_shot: bool,
    ) -> Self {
        let ghost = PlacementGhost::new(&payload);
        Self {
            payload,
            target_sheet,
            reference_override: None,
            prefix,
            one_shot,
            ghost,
        }
    }

    /// Replace the payload (a turn, a Tab edit, the next designator) and
    /// redraw the ghost from it.
    pub fn set_payload(&mut self, payload: PlacementPayload) {
        self.ghost = PlacementGhost::new(&payload);
        self.payload = payload;
    }
}
