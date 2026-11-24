//! Notices an edit raises for the Messages panel, as data (ADR-0006: the core
//! returns what happened; the host words it through its catalog).

use signex_types::schematic::Point;

/// One notice of an edit.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineNotice {
    /// A dragged wire could not keep what joined it at these points: each
    /// entry names what was there ("R7 pin 1", "label VCC").
    DraggedWireLeft { held: Vec<String> },
    /// A grip drag could not keep the wires attached at the end it moved,
    /// which sat at `at` before the drag.
    GripEndLeft { at: Point },
    /// Pins "keep wires" held on their own net that were not joined to the
    /// wire of another net their new end lies in.
    HeldPinsNotJoined { pins: Vec<String> },
    /// Pin ends left with no junction dot because one would merge two nets.
    PinDotRefused { pins: Vec<RefusedPinDot> },
    /// Connected pins a "keep wires" move left as a plain move, because
    /// keeping them connected would cross another net ("R1 pin 1").
    PinsNotKept { pins: Vec<String> },
}

#[cfg(test)]
impl EngineNotice {
    /// The pins of a [`Self::PinsNotKept`]; none for any other notice.
    pub(crate) fn pins_not_kept(&self) -> &[String] {
        match self {
            Self::PinsNotKept { pins } => pins,
            _ => &[],
        }
    }
}

/// A pin end left without a dot, and the two nets a dot would have merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusedPinDot {
    /// The pin, "R1 pin 2".
    pub pin: String,
    /// The net of the wire through the pin end, as the netlist names it;
    /// `None` for a net with no name and no part on it.
    pub through: Option<String>,
    /// The other net there: a wire's (as the netlist names it), else a
    /// label's text; `None` when nothing names it.
    pub other: Option<String>,
}
