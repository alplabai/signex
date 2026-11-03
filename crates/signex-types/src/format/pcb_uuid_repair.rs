//! Repair of repeated object uuids on a freshly parsed board (#1886).
//!
//! The same rule as a sheet (see `uuid_repair`): every uuid-carrying object
//! of a board shares ONE id space, the board's own uuid included. A later
//! object that repeats an identical earlier one of its kind is dropped;
//! otherwise it keeps its content and gets a deterministic new uuid.
//!
//! Two footprints sharing a uuid are refused at parse (their pads are keyed
//! by it), so only a footprint carrying the board's own id is re-minted. A pad copied onto another footprint with the same id
//! is never dropped (the other footprint is a different owner), only
//! re-minted; footprint graphics carry no id.

use super::load_repair::{SheetLoadRepair, kind};
use super::uuid_repair::{IdSpace, by_field, no_name};
use crate::pcb::PcbBoard;

/// Make every object uuid on `board` unique, returning what was repaired
/// (empty when nothing repeated).
pub(super) fn repair_duplicate_uuids(board: &mut PcbBoard) -> Vec<SheetLoadRepair> {
    let mut ids = IdSpace::new(board.uuid);
    // A footprint's pads are attached to it and `footprint_ref` is written
    // from its uuid on save, so a re-minted id stays consistent.
    ids.check(
        &mut board.footprints,
        kind::FOOTPRINT,
        None,
        by_field!(|f| f.reference.clone()),
    );
    for footprint in &mut board.footprints {
        let owner = footprint.reference.clone();
        ids.check(
            &mut footprint.pads,
            kind::PAD,
            Some(&owner),
            by_field!(|p| p.number.clone()),
        );
    }
    ids.check(&mut board.segments, kind::TRACK, None, by_field!(no_name));
    ids.check(&mut board.vias, kind::VIA, None, by_field!(no_name));
    ids.check(
        &mut board.zones,
        kind::ZONE,
        None,
        by_field!(|z| z.net_name.clone()),
    );
    ids.check(
        &mut board.texts,
        kind::BOARD_TEXT,
        None,
        by_field!(|t| t.text.clone()),
    );
    ids.into_repairs()
}
