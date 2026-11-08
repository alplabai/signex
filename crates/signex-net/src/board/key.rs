//! How a schematic terminal finds its board pad.

use std::hash::Hash;

use signex_types::net::Terminal;
use signex_types::pcb::{Footprint, Pad};

/// The link between a schematic terminal and a board pad: two sides that
/// produce the same key are the same electrical terminal.
///
/// Net assignment only ever compares keys, so the link rule can change (the
/// sheet path plus symbol UUID key replaces [`ReferencePadKey`] once the
/// footprint carries that link) without touching the assignment itself.
pub trait PadLinkKey {
    type Key: Eq + Hash + Clone;

    /// The key of a schematic terminal, or `None` when it cannot be linked
    /// (an unannotated designator, an empty pin id).
    fn terminal_key(&self, terminal: &Terminal) -> Option<Self::Key>;

    /// The key of a pad on a footprint, or `None` when it cannot be linked
    /// (an unnumbered mechanical pad, a footprint with no reference).
    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<Self::Key>;
}

/// A designator plus a pin or pad number, e.g. `R1` / `2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferencePad {
    pub reference: String,
    pub pad: String,
}

/// The interim link: the schematic designator equals the footprint
/// reference and the pin number equals the pad number.
///
/// A designator is not a stable identity (re-annotation renames it), which
/// is why this key is meant to be replaced, not extended.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReferencePadKey;

impl PadLinkKey for ReferencePadKey {
    type Key = ReferencePad;

    fn terminal_key(&self, terminal: &Terminal) -> Option<ReferencePad> {
        reference_pad(&terminal.reference, &terminal.pin)
    }

    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<ReferencePad> {
        reference_pad(&footprint.reference, &pad.number)
    }
}

fn reference_pad(reference: &str, pad: &str) -> Option<ReferencePad> {
    let reference = reference.trim();
    let pad = pad.trim();
    if reference.is_empty() || reference.contains('?') || pad.is_empty() {
        return None;
    }
    Some(ReferencePad {
        reference: reference.to_string(),
        pad: pad.to_string(),
    })
}
