//! How a schematic terminal finds its board pad.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use signex_types::net::{Netlist, Terminal};
use signex_types::pcb::{Footprint, Pad, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

/// The link between a schematic terminal and a board pad: two sides that
/// produce the same key are the same electrical terminal.
///
/// Net assignment only ever compares keys, so the link rule can change
/// without touching the assignment itself.
pub trait PadLinkKey {
    type Key: Eq + Hash + Clone;

    /// The key of a schematic terminal, or `None` when it cannot be linked
    /// (no symbol identity, an empty pin id).
    fn terminal_key(&self, terminal: &Terminal) -> Option<Self::Key>;

    /// The key of a pad on a footprint, or `None` when it cannot be linked
    /// (an unnumbered mechanical pad, a footprint nothing identifies).
    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<Self::Key>;

    /// Whether the footprint has no stored link to a schematic symbol; the
    /// sync reports it. Defaults to `false` for keys that need no link.
    fn footprint_unlinked(&self, _footprint: &Footprint) -> bool {
        false
    }
}

/// One pin of one placed symbol: sheet occurrence, symbol UUID, pad number.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolPad {
    pub sheet: InstancePath,
    pub symbol: Uuid,
    pub pad: String,
}

/// What a pad is keyed by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PadIdentity {
    /// The pad's footprint is tied to a symbol (stored link, or the
    /// designator fallback resolved to exactly one).
    Linked(SymbolPad),
    /// A footprint with no stored link whose designator names no single
    /// symbol. No terminal has this key, so it matches nothing.
    Unresolved { reference: String, pad: String },
}

/// Keys pads by (sheet path, symbol UUID, pad number), the identity a
/// netlist terminal carries. It survives re-annotation, unlike a designator.
///
/// A footprint with no stored link (placed by hand, or on a board made
/// before the link existed) is tied to the symbol whose designator equals the
/// footprint reference, when exactly one does. That fallback is not stored.
#[derive(Debug, Clone, Default)]
pub struct SymbolPadKey {
    by_reference: HashMap<String, SymbolLink>,
    ambiguous: HashSet<String>,
}

impl SymbolPadKey {
    /// The key for syncing against `netlist`; its terminals supply the
    /// designator fallback.
    pub fn for_netlist(netlist: &Netlist) -> Self {
        let mut key = Self::default();
        for terminal in netlist.nets.iter().flat_map(|n| &n.terminals) {
            if terminal.symbol.is_nil() {
                continue;
            }
            let Some(reference) = clean(&terminal.reference).filter(|r| !r.contains('?')) else {
                continue;
            };
            let link = SymbolLink {
                sheet: terminal.occurrence.clone(),
                symbol: terminal.symbol,
            };
            match key.by_reference.get(reference) {
                Some(seen) if *seen != link => {
                    key.ambiguous.insert(reference.to_string());
                }
                Some(_) => {}
                None => {
                    key.by_reference.insert(reference.to_string(), link);
                }
            }
        }
        key
    }
}

impl PadLinkKey for SymbolPadKey {
    type Key = PadIdentity;

    fn terminal_key(&self, terminal: &Terminal) -> Option<PadIdentity> {
        let pad = clean(&terminal.pin)?;
        if terminal.symbol.is_nil() {
            return None;
        }
        Some(PadIdentity::Linked(SymbolPad {
            sheet: terminal.occurrence.clone(),
            symbol: terminal.symbol,
            pad: pad.to_string(),
        }))
    }

    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<PadIdentity> {
        let number = clean(&pad.number)?;
        if let Some(link) = &footprint.schematic_link {
            return Some(linked(link, number));
        }
        let reference = clean(&footprint.reference).filter(|r| !r.contains('?'))?;
        match self.by_reference.get(reference) {
            Some(link) if !self.ambiguous.contains(reference) => Some(linked(link, number)),
            _ => Some(PadIdentity::Unresolved {
                reference: reference.to_string(),
                pad: number.to_string(),
            }),
        }
    }

    fn footprint_unlinked(&self, footprint: &Footprint) -> bool {
        footprint.schematic_link.is_none()
    }
}

fn linked(link: &SymbolLink, pad: &str) -> PadIdentity {
    PadIdentity::Linked(SymbolPad {
        sheet: link.sheet.clone(),
        symbol: link.symbol,
        pad: pad.to_string(),
    })
}

/// `text` trimmed, or `None` when blank.
fn clean(text: &str) -> Option<&str> {
    let text = text.trim();
    (!text.is_empty()).then_some(text)
}
