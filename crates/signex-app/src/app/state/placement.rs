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

/// What is on the cursor.
pub enum PlacementKind {
    /// A library part: placed again and again, each copy with the next
    /// designator counted for `prefix`, until Esc or a right-click.
    Part { prefix: String },
    /// Pasted content: keeps its own designators and is placed once.
    Paste,
}

impl PlacementKind {
    /// Designator letters, for a part.
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        match self {
            Self::Part { prefix } => Some(prefix),
            Self::Paste => None,
        }
    }

    #[must_use]
    pub fn is_paste(&self) -> bool {
        matches!(self, Self::Paste)
    }
}

pub struct PlacementSession {
    /// What a click places, with its current orientation.
    pub payload: PlacementPayload,
    /// The sheet the session was armed for.
    pub target_sheet: PathBuf,
    /// A designator typed in the Tab form: used by the next click, then
    /// stepped. `None` means the next free number for the prefix.
    pub reference_override: Option<String>,
    /// A part (repeats, steps designators) or a paste (once, keeps its own).
    pub kind: PlacementKind,
    /// The payload as drawn at the cursor; rebuilt whenever the payload
    /// changes.
    pub ghost: PlacementGhost,
}

impl PlacementSession {
    /// A session for `payload` on `target_sheet`.
    #[must_use]
    pub fn new(payload: PlacementPayload, target_sheet: PathBuf, kind: PlacementKind) -> Self {
        let ghost = PlacementGhost::new(&payload);
        Self {
            payload,
            target_sheet,
            reference_override: None,
            kind,
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

/// What "Repeat last item" arms again: the text-bearing tool that placed
/// something last, with the text its next copy would carry (already stepped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LastPlacement {
    /// A net label, port or hierarchical port.
    Label {
        port: Option<(signex_types::schematic::LabelType, String)>,
        text: String,
    },
    /// A text note.
    Text { text: String },
}
