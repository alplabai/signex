//! Swap the footprint of components placed on a board for a library
//! footprint (#2366): the Component view's Footprint picker and the board
//! menu's Change Footprint.
//!
//! Pure: [`swap_commands`] reads the board and returns one
//! [`BoardCommand::ReplaceFootprint`] per footprint, which the caller runs
//! as one undo step, and the old pads no new pad takes over, for the
//! Messages panel. The replace itself keeps the position, rotation, side,
//! designator and value and carries pad nets by pad number.

use std::collections::BTreeSet;

use signex_types::part::FootprintSourceRef;
use signex_types::pcb::{Footprint, PcbBoard};
use uuid::Uuid;

use super::command::BoardCommand;
use super::component::ComponentRefusal;
use super::eco::remint_pads;
use super::footprint_content::{carry_would_lose, unmatched_pads};
use super::placement;

/// An old pad the new footprint has no pad of its number for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmatchedPad {
    /// The designator of the footprint it was on.
    pub reference: String,
    pub number: String,
    /// The net it was on, if any.
    pub net: Option<String>,
}

/// What a swap runs and what it leaves behind.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintSwap {
    pub commands: Vec<BoardCommand>,
    /// The old pads that go with no new pad of their number, in board order.
    pub unmatched: Vec<UnmatchedPad>,
}

/// Swap every footprint of `ids` for `template`, a library footprint in its
/// top-side board form. Each footprint gets its own pad uuids, which follow
/// from it and the library footprint. Refused when a footprint is gone or
/// locked, or when the new footprint has fewer pads of a number and an old
/// pad past that count holds a net or a lock. A template the board cannot
/// hold (not top-side, a pad not at a finite place) is refused when the
/// command runs.
pub fn swap_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    template: &Footprint,
) -> Result<FootprintSwap, ComponentRefusal> {
    let footprints = placement::editable_footprints(board, ids)?;
    let source = template
        .library_ref
        .clone()
        .unwrap_or_else(|| FootprintSourceRef {
            library: Uuid::nil(),
            footprint: template.uuid,
            version: String::new(),
        });
    let mut swap = FootprintSwap {
        commands: Vec::with_capacity(footprints.len()),
        unmatched: Vec::new(),
    };
    for old in footprints {
        let mut with = template.clone();
        remint_pads(&mut with, old.uuid, &source);
        if let Some(lost) = carry_would_lose(old, &with) {
            return Err(lost.into());
        }
        swap.unmatched
            .extend(unmatched_pads(old, &with).into_iter().map(|pad| UnmatchedPad {
                reference: old.reference.clone(),
                number: pad.number.clone(),
                net: pad.net.as_ref().map(|net| net.name.clone()),
            }));
        swap.commands.push(BoardCommand::ReplaceFootprint {
            footprint: old.uuid,
            with: Box::new(with),
        });
    }
    Ok(swap)
}

#[cfg(test)]
#[path = "footprint_swap_tests.rs"]
mod tests;
