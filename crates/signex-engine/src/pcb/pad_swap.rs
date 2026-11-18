//! The board half of a pin or gate swap (#2410): pads of one footprint trade
//! nets. Pure: [`pad_swap`] reads the board and returns the one
//! [`BoardCommand::ReplacePads`] the caller runs as the board's step of the
//! swap; the schematic half is `crate::part_swap`. Which pins may trade is
//! the part's rule (`signex_types::part_swap`), checked by the caller; this
//! checks what the board itself holds back: a locked footprint, a locked pad
//! net and the footprint's own Pin Swapping / Part Swapping switches.

use std::collections::BTreeSet;

use signex_types::pcb::{Footprint, PadNet, PcbBoard};
use uuid::Uuid;

use super::command::BoardCommand;
use super::placement;
use super::replace::FootprintPad;

/// What is traded: two pins, or every pin of two gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapKind {
    Pins,
    Gates,
}

/// Why a swap changes nothing on the board.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PadSwapRefusal {
    #[error("the footprint {0} is not on the board")]
    NoFootprint(Uuid),
    #[error("{0} is locked")]
    Locked(String),
    #[error("Pin Swapping is off for {0}; turn it on in its Component view")]
    PinSwappingOff(String),
    #[error("Part Swapping is off for {0}; turn it on in its Component view")]
    PartSwappingOff(String),
    #[error("{designator} has no pad {pad}")]
    NoPad { designator: String, pad: String },
    #[error("the net of pad {pad} of {designator} is locked")]
    NetLocked { designator: String, pad: String },
    #[error("pad {pad} of {designator} is named twice")]
    NamedTwice { designator: String, pad: String },
}

/// A pad whose net the swap changes, as the result reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadNetChange {
    pub pad: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// The board step of a swap and the pads it changes. `commands` is empty
/// when every pair already shares its net.
#[derive(Debug, Clone, PartialEq)]
pub struct PadSwap {
    pub commands: Vec<BoardCommand>,
    pub changes: Vec<PadNetChange>,
}

/// Trade the nets of each `(a, b)` pad-number pair of `footprint`: every pad
/// numbered `a` takes the net of the first pad numbered `b`, and the other
/// way round. Refused, with nothing built, for a footprint that is gone or
/// locked, a footprint whose swapping switch for `kind` is off, a pad number
/// it lacks or names twice, and a pad whose net is locked.
pub fn pad_swap(
    board: &PcbBoard,
    footprint: Uuid,
    pairs: &[(String, String)],
    kind: SwapKind,
) -> Result<PadSwap, PadSwapRefusal> {
    let fp = board
        .footprints
        .iter()
        .find(|fp| fp.uuid == footprint)
        .ok_or(PadSwapRefusal::NoFootprint(footprint))?;
    let designator = placement::name(fp);
    if fp.locked {
        return Err(PadSwapRefusal::Locked(designator));
    }
    match kind {
        SwapKind::Pins if !fp.component.pin_swapping => {
            return Err(PadSwapRefusal::PinSwappingOff(designator));
        }
        SwapKind::Gates if !fp.component.part_swapping => {
            return Err(PadSwapRefusal::PartSwappingOff(designator));
        }
        _ => {}
    }
    let mut named: BTreeSet<&str> = BTreeSet::new();
    for number in pairs.iter().flat_map(|(a, b)| [a.as_str(), b.as_str()]) {
        if !named.insert(number) {
            return Err(PadSwapRefusal::NamedTwice {
                designator,
                pad: number.to_string(),
            });
        }
    }
    let mut pads = Vec::new();
    let mut changes = Vec::new();
    for (a, b) in pairs {
        let net_a = first_net(fp, a, &designator)?;
        let net_b = first_net(fp, b, &designator)?;
        for (number, net) in [(a, &net_b), (b, &net_a)] {
            for pad in fp.pads.iter().filter(|p| p.number == *number) {
                if pad.net == *net {
                    continue;
                }
                if pad.net_locked {
                    return Err(PadSwapRefusal::NetLocked {
                        designator,
                        pad: number.clone(),
                    });
                }
                changes.push(PadNetChange {
                    pad: number.clone(),
                    from: pad.net.as_ref().map(|n| n.name.clone()),
                    to: net.as_ref().map(|n| n.name.clone()),
                });
                let mut swapped = pad.clone();
                swapped.net = net.clone();
                pads.push(FootprintPad {
                    footprint,
                    pad: swapped,
                });
            }
        }
    }
    let commands = if pads.is_empty() {
        Vec::new()
    } else {
        vec![BoardCommand::ReplacePads { pads }]
    };
    Ok(PadSwap { commands, changes })
}

/// The net of the first pad numbered `number`; refused when there is none.
fn first_net(
    fp: &Footprint,
    number: &str,
    designator: &str,
) -> Result<Option<PadNet>, PadSwapRefusal> {
    fp.pads
        .iter()
        .find(|p| p.number == number)
        .map(|p| p.net.clone())
        .ok_or_else(|| PadSwapRefusal::NoPad {
            designator: designator.to_string(),
            pad: number.to_string(),
        })
}

#[cfg(test)]
#[path = "pad_swap_tests.rs"]
mod tests;
