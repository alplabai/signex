//! How a schematic terminal finds its board pad.

use std::collections::HashMap;
use std::hash::Hash;

use signex_types::net::Terminal;
use signex_types::part::PinPadMap;
use signex_types::pcb::{Footprint, Pad, PcbBoard};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::parts::PartAnchor;

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
    /// (an unnumbered mechanical pad, a footprint with no link).
    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<Self::Key>;

    /// Whether the footprint has no stored link to a schematic part. The
    /// sync reports it and leaves the footprint completely untouched.
    /// Defaults to `false` for keys that need no link.
    fn footprint_unlinked(&self, _footprint: &Footprint) -> bool {
        false
    }
}

/// One pin of one placed part: sheet occurrence, the part's anchor unit and
/// the pad number.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolPad {
    pub sheet: InstancePath,
    /// The part's anchor: the smallest UUID among its units.
    pub part: Uuid,
    pub pad: String,
}

/// Keys pads by (sheet path, part, pad number). The part is named by the
/// footprint's stored link, which lists every unit of a multi-unit part, so a
/// pin of any unit lands on its pad. Neither the sheet path nor a unit UUID
/// changes when the schematic is re-annotated, unlike a designator, and no
/// designator is read here.
///
/// A footprint with no link has no key: the sync reports it and leaves it be.
/// `link_footprints_by_designator` is the one place a designator links one.
#[derive(Debug, Clone, Default)]
pub struct SymbolPadKey {
    /// Unit -> its part's anchor, for every unit some footprint links to.
    /// A unit named by two different links belongs to the first.
    anchor_of: HashMap<(InstancePath, Uuid), Uuid>,
    /// Which pad each pin of a part lands on; a part with no map is 1:1.
    pin_maps: HashMap<PartAnchor, PinPadMap>,
}

impl SymbolPadKey {
    /// The key for syncing `board`: its footprints' links say which units
    /// make up each part.
    pub fn for_board(board: &PcbBoard) -> Self {
        let mut anchor_of = HashMap::new();
        for link in board
            .footprints
            .iter()
            .filter_map(|f| f.schematic_link.as_ref())
        {
            let Some(anchor) = link.units.iter().min().copied() else {
                continue;
            };
            for unit in &link.units {
                anchor_of
                    .entry((link.sheet.clone(), *unit))
                    .or_insert(anchor);
            }
        }
        Self {
            anchor_of,
            pin_maps: HashMap::new(),
        }
    }

    /// Land each pin of a part on the pad its map names (a library part's
    /// pin-to-pad overrides). Parts not in `pin_maps` stay 1:1.
    pub fn with_pin_maps(mut self, pin_maps: HashMap<PartAnchor, PinPadMap>) -> Self {
        self.pin_maps = pin_maps;
        self
    }
}

impl PadLinkKey for SymbolPadKey {
    type Key = SymbolPad;

    fn terminal_key(&self, terminal: &Terminal) -> Option<SymbolPad> {
        let pad = clean(&terminal.pin)?;
        if terminal.symbol.is_nil() {
            return None;
        }
        let part = self
            .anchor_of
            .get(&(terminal.occurrence.clone(), terminal.symbol))
            .copied()
            .unwrap_or(terminal.symbol);
        let anchor = PartAnchor {
            sheet: terminal.occurrence.clone(),
            part,
        };
        let pad = self
            .pin_maps
            .get(&anchor)
            .map_or(pad, |map| map.pad_for(pad))
            .to_string();
        Some(SymbolPad {
            sheet: anchor.sheet,
            part,
            pad,
        })
    }

    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<SymbolPad> {
        let number = clean(&pad.number)?;
        let link = footprint.schematic_link.as_ref()?;
        Some(SymbolPad {
            sheet: link.sheet.clone(),
            part: link.units.iter().min().copied()?,
            pad: number.to_string(),
        })
    }

    fn footprint_unlinked(&self, footprint: &Footprint) -> bool {
        footprint.schematic_link.is_none()
    }
}

/// `text` trimmed, or `None` when blank.
fn clean(text: &str) -> Option<&str> {
    let text = text.trim();
    (!text.is_empty()).then_some(text)
}
