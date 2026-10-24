//! Footprint/pad reassembly and uniqueness checks for `.snxpcb`.
//!
//! `pads.footprint_ref` holds the owning footprint's uuid (1.1) or its
//! reference designator (1.0). A file in which two footprints share a uuid is
//! refused on load, so pads are never merged onto the wrong footprint.

use std::collections::BTreeMap;

use uuid::Uuid;

use super::extras::{FootprintExtras, PadExtras};
use super::pcb_rows::{PcbFootprintRow, PcbPadRow, row_to_footprint, row_to_pad};
use super::{FormatError, UnknownColumns, UnknownFields};
use crate::pcb::{Footprint, Point as PcbPoint};

/// Refuse a board in which two footprints share a uuid: pads are keyed by it.
pub(super) fn check_unique_footprint_uuids(footprints: &[Footprint]) -> Result<(), FormatError> {
    let mut seen: BTreeMap<Uuid, &str> = BTreeMap::new();
    for fp in footprints {
        if let Some(first) = seen.insert(fp.uuid, &fp.reference) {
            return Err(FormatError::DuplicateFootprintUuid {
                uuid: fp.uuid.to_string(),
                first: first.to_string(),
                second: fp.reference.clone(),
            });
        }
    }
    Ok(())
}

/// Footprint uuid a pad's `footprint_ref` addresses: the ref itself when it
/// is a uuid, else a deterministic key derived from the designator.
fn orphan_uuid(footprint_ref: &str) -> Uuid {
    footprint_ref.parse::<Uuid>().unwrap_or_else(|_| {
        // FNV-1a/128: stable across builds, so a saved placeholder
        // re-keys to the same uuid on every load.
        let hash = footprint_ref
            .bytes()
            .fold(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d_u128, |h, b| {
                (h ^ u128::from(b)).wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b)
            });
        Uuid::from_u128(hash)
    })
}

pub(super) fn assemble_footprints(
    footprint_rows: Vec<PcbFootprintRow>,
    pad_rows: Vec<PcbPadRow>,
    footprint_extras: &BTreeMap<String, FootprintExtras>,
    pad_extras: &BTreeMap<String, PadExtras>,
) -> Result<Vec<Footprint>, FormatError> {
    let mut footprints: Vec<Footprint> = footprint_rows
        .into_iter()
        .map(|row| {
            let extra = footprint_extras
                .get(&row.uuid.to_string())
                .cloned()
                .unwrap_or_default();
            row_to_footprint(row, extra)
        })
        .collect();

    check_unique_footprint_uuids(&footprints)?;

    for prow in pad_rows {
        let extra = pad_extras
            .get(&prow.uuid.to_string())
            .cloned()
            .unwrap_or_default();
        let pad = row_to_pad(prow.clone(), extra);
        let by_uuid = prow
            .footprint_ref
            .parse::<Uuid>()
            .ok()
            .and_then(|u| footprints.iter().position(|f| f.uuid == u));
        let orphan_key = orphan_uuid(&prow.footprint_ref);
        let target = by_uuid
            .or_else(|| {
                footprints
                    .iter()
                    .position(|f| f.reference == prow.footprint_ref)
            })
            .or_else(|| footprints.iter().position(|f| f.uuid == orphan_key));
        if let Some(i) = target {
            footprints[i].pads.push(pad);
        } else {
            // Orphan pad: keep it on a placeholder footprint, one per
            // distinct key, with a stable uuid so a save round-trips them.
            footprints.push(Footprint {
                uuid: orphan_key,
                reference: prow.footprint_ref.clone(),
                value: String::new(),
                footprint_id: String::new(),
                position: PcbPoint { x: 0.0, y: 0.0 },
                rotation: 0.0,
                layer: String::new(),
                locked: false,
                pads: vec![pad],
                graphics: Vec::new(),
                properties: Vec::new(),
                // The placeholder is not in the file: it carries no
                // foreign keys and must not inherit the orphan pad's.
                unknown: UnknownFields::default(),
                unknown_columns: UnknownColumns::default(),
            });
        }
    }
    Ok(footprints)
}
