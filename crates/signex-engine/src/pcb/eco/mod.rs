//! "Update PCB from schematic" as a reviewable change list (#1312).
//!
//! [`diff_board`] compares the schematic's parts and netlist with a board
//! and lists every change as a row ([`EcoRow`]) the user can tick or untick.
//! [`lower`] turns the ticked rows into board commands in a fixed order;
//! [`check`] runs them on a copy (Validate); the caller executes them as one
//! labelled step at the list's revision, so Execute is all or nothing, one
//! undo step, and refused once the board has moved on. [`report_text`]
//! writes the list as plain text.
//!
//! Nothing here reads a file or a library: the caller resolves each part's
//! footprint (`signex_library::resolve_part_footprint`) and builds the
//! netlist, and hands both in through [`EcoInput`].
//!
//! The rules (plan and maintainer decisions on #1312):
//! - A part matches a footprint by its `SymbolLink` (same sheet, a shared
//!   unit). An unlinked footprint with the part's designator gets a Link
//!   row, so its placement survives.
//! - An unlinked footprint (a hole, a fiducial) is never removed.
//! - A part with `on_board == false` is removed, or not added. DNP parts are
//!   placed.
//! - Nothing linked under a sheet that could not be read is removed.
//! - A newer library version of the same footprint is listed, unticked.
//! - Remove and change rows of a locked footprint start unticked.
//! - A pad whose net is locked gets no pad net row.

mod change;
mod diff;
mod lower;
mod nets;
mod report;

pub use change::{
    ChangeReason, EcoBlock, EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoNote, EcoRow,
};
pub use diff::diff_board;
pub use lower::{EcoRefusal, check, lower};
pub use nets::refresh_nets;
pub use report::report_text;

#[cfg(test)]
mod fixture;
#[cfg(test)]
mod net_tests;
#[cfg(test)]
mod tests;

pub use pcb_geom::Staging;
use signex_net::DesignPart;
use signex_types::net::Netlist;
use signex_types::part::{FootprintResolveError, ResolvedFootprint};
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

/// One part of the design, as the change list needs it.
#[derive(Debug, Clone)]
pub struct EcoPart {
    pub part: DesignPart,
    /// The value of the part's first unit.
    pub value: String,
    /// `false` when the part's first unit is marked not on the board: the
    /// part is removed, or not added.
    pub on_board: bool,
    /// Its footprint, resolved by the caller from the part's library row.
    pub footprint: Result<ResolvedFootprint, FootprintResolveError>,
}

/// What [`diff_board`] compares.
#[derive(Debug, Clone, Copy)]
pub struct EcoInput<'a> {
    pub board: &'a PcbBoard,
    /// The board revision the list is computed at.
    pub revision: u64,
    /// The design's parts, in sheet then placement order.
    pub parts: &'a [EcoPart],
    pub netlist: &'a Netlist,
    /// Sheets that could not be read; nothing linked under one is removed.
    pub unreadable: &'a [InstancePath],
    pub staging: Staging,
}

/// The design's parts from its placed sheets, with each part's footprint
/// resolved by `resolve` (given the part and its first unit's symbol).
pub fn design_eco_parts(
    sheets: &[(InstancePath, &SchematicSheet)],
    mut resolve: impl FnMut(&DesignPart, &Symbol) -> Result<ResolvedFootprint, FootprintResolveError>,
) -> Vec<EcoPart> {
    signex_net::design_parts(sheets)
        .into_iter()
        .filter_map(|part| {
            let sheet = sheets.iter().find(|(path, _)| *path == part.sheet)?.1;
            let first = sheet
                .symbols
                .iter()
                .find(|s| part.units.first() == Some(&s.uuid))?;
            Some(EcoPart {
                value: first.value.clone(),
                on_board: first.on_board,
                footprint: resolve(&part, first),
                part,
            })
        })
        .collect()
}

/// A uuid that depends only on `parts`, so a recomputed list mints the same
/// footprint and pad uuids and the rows keep their keys. 128-bit FNV-1a,
/// stamped as a version 8 (custom) uuid. Not for security.
pub(crate) fn stable_uuid(parts: &[&[u8]]) -> Uuid {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013b;
    let mut hash = OFFSET;
    for part in parts {
        for &byte in part.iter().chain(&[0xff]) {
            hash ^= u128::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    let mut bytes = hash.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Whether `path` is `sheet` or a sheet below it.
pub(crate) fn is_under(path: &InstancePath, sheet: &InstancePath) -> bool {
    path.chain().starts_with(sheet.chain())
}
